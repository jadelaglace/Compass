//! T1.7 端到端验收测试：完整闭环验证。
//!
//! 验收闭环：新建笔记 -> 引擎算分写回 frontmatter -> 验证 composite 排序。
//! 覆盖：API create -> get -> score 调分 -> access boost -> feed 排序 + watcher 自动算分。

#![cfg(test)]

use std::fs;
use std::sync::Arc;

use tempfile::tempdir;
use tokio::sync::Mutex;

use crate::api::{
    self, AccessRequest, AppState, CreateEntityRequest, FeedQuery, ScoreUpdateRequest,
};
use crate::db::Db;
use crate::frontmatter;
use crate::models::Weights;
use crate::watcher;

fn setup(vault: &std::path::Path) -> AppState {
    AppState {
        db: Arc::new(Mutex::new(Db::open_in_memory().unwrap())),
        vault: vault.to_path_buf(),
        weights: Weights::default(),
    }
}

/// 端到端：API 完整闭环
/// create -> get -> score 调分 -> access boost -> feed 排序 -> frontmatter 可读
#[tokio::test]
async fn test_e2e_api_full_loop() {
    let dir = tempdir().unwrap();
    let vault = dir.path().join("vault");
    fs::create_dir_all(&vault).unwrap();
    let state = setup(&vault);

    // 1. create：新建笔记 -> 算分 -> 写 .md -> 索引
    let (status, json) = api::create_entity(
        axum::extract::State(state.clone()),
        axum::Json(CreateEntityRequest {
            title: "博弈论基础".to_string(),
            layer: "knowledge".to_string(),
            content: Some("纳什均衡是核心概念 [[know-000002]]".to_string()),
            interest: Some(85.0),
            strategy: Some(92.0),
            consensus: Some(78.0),
        }),
    )
    .await
    .unwrap();
    assert_eq!(status, axum::http::StatusCode::CREATED);
    let id1 = json["id"].as_str().unwrap().to_string();
    let composite1 = json["composite"].as_f64().unwrap();
    // 85*0.4 + 92*0.35 + 78*0.25 = 34 + 32.2 + 19.5 = 85.7
    assert!(
        (composite1 - 85.7).abs() < 1e-6,
        "composite 应为 85.7，实际 {composite1}"
    );

    // 2. 验证 .md 文件已创建且 frontmatter 含 score.composite（Dataview 可读）
    let file1 = vault.join("Knowledge").join(format!("{id1}.md"));
    assert!(file1.exists(), "文件应存在");
    let content = fs::read_to_string(&file1).unwrap();
    let (fm, body) = frontmatter::split_frontmatter(&content).unwrap();
    assert!(fm.contains("composite:"), "frontmatter 应含 composite");
    assert!(fm.contains("score:"), "frontmatter 应含 score 块");
    assert!(body.contains("纳什均衡"), "正文应保留");
    let score = frontmatter::get_score(&fm).unwrap().unwrap();
    assert!((score.composite - 85.7).abs() < 1e-6);

    // 3. get：详情含 refs
    let detail = api::get_entity(
        axum::extract::State(state.clone()),
        axum::extract::Path(id1.clone()),
    )
    .await
    .unwrap()
    .0;
    assert_eq!(detail.title.as_deref(), Some("博弈论基础"));
    assert!(detail.refs.contains(&"know-000002".to_string()));

    // 4. score 调分：interest 85 -> 95
    let score_json = api::update_score(
        axum::extract::State(state.clone()),
        axum::extract::Path(id1.clone()),
        axum::Json(ScoreUpdateRequest {
            interest: Some(95.0),
            strategy: None,
            consensus: None,
        }),
    )
    .await
    .unwrap()
    .0;
    let new_composite = score_json["score"]["composite"].as_f64().unwrap();
    // 95*0.4 + 92*0.35 + 78*0.25 = 38 + 32.2 + 19.5 = 89.7
    assert!(
        (new_composite - 89.7).abs() < 1e-6,
        "调分后 composite 应为 89.7"
    );

    // 验证写回 frontmatter
    let content = fs::read_to_string(&file1).unwrap();
    let (fm, _) = frontmatter::split_frontmatter(&content).unwrap();
    let score = frontmatter::get_score(&fm).unwrap().unwrap();
    assert!(
        (score.interest - 95.0).abs() < 1e-9,
        "frontmatter interest 应为 95"
    );

    // 5. access boost：study -> interest +3
    let access_json = api::record_access(
        axum::extract::State(state.clone()),
        axum::extract::Path(id1.clone()),
        axum::Json(AccessRequest {
            depth: "study".to_string(),
        }),
    )
    .await
    .unwrap()
    .0;
    let interest = access_json["score"]["interest"].as_f64().unwrap();
    assert!((interest - 98.0).abs() < 1e-9, "95+3=98");
    let access_count = access_json["score"]["access_count"].as_f64().unwrap();
    assert_eq!(access_count, 1.0, "access_count 0+1=1");

    // 6. 创建第二个低分实体
    let (_, json2) = api::create_entity(
        axum::extract::State(state.clone()),
        axum::Json(CreateEntityRequest {
            title: "低分笔记".to_string(),
            layer: "knowledge".to_string(),
            content: Some("内容".to_string()),
            interest: Some(30.0),
            strategy: Some(30.0),
            consensus: Some(30.0),
        }),
    )
    .await
    .unwrap();
    let id2 = json2["id"].as_str().unwrap().to_string();

    // 7. feed 排序：高分在前
    let feed = api::feed(
        axum::extract::State(state.clone()),
        axum::extract::Query(FeedQuery {
            mode: "explore".to_string(),
            limit: 10,
        }),
    )
    .await
    .unwrap()
    .0;
    assert_eq!(feed.len(), 2);
    assert_eq!(feed[0].id, id1, "高分应排第一");
    assert_eq!(feed[1].id, id2);

    // 8. entities/top 也排序正确
    let top = api::entities_top(
        axum::extract::State(state.clone()),
        axum::extract::Query(api::TopQuery {
            layer: None,
            limit: 10,
        }),
    )
    .await
    .unwrap()
    .0;
    assert_eq!(top[0].id, id1);
}

/// 端到端：watcher 自动为无 score 笔记算分写回
#[tokio::test]
async fn test_e2e_watcher_assigns_default_score() {
    let dir = tempdir().unwrap();
    let vault = dir.path().join("vault");
    fs::create_dir_all(&vault).unwrap();
    let state = setup(&vault);

    // 手动写一个无 score 的笔记
    let md = "---\nid: know-000001\ntitle: 无评分笔记\nlayer: knowledge\n---\n这是正文内容。\n";
    let path = vault.join("know-000001.md");
    fs::write(&path, md).unwrap();

    // watcher 处理：应为无 score 笔记计算默认评分并写回
    watcher::process_single_file(&vault, &state.db, &state.weights, &path)
        .await
        .unwrap();

    // 验证 score 已写回 frontmatter
    let content = fs::read_to_string(&path).unwrap();
    let (fm, _) = frontmatter::split_frontmatter(&content).unwrap();
    assert!(fm.contains("score:"), "应写入 score 块");
    assert!(fm.contains("composite:"), "应含 composite");
    let score = frontmatter::get_score(&fm).unwrap().unwrap();
    // 默认 5/5/5 -> composite = 5.0
    assert!((score.composite - 5.0).abs() < 1e-6);
    assert_eq!(score.access_count, 0);

    // 验证已索引到 db
    let entity = state.db.lock().await.get_entity("know-000001").unwrap();
    assert!(entity.is_some());
    let entity = entity.unwrap();
    assert_eq!(entity.title.as_deref(), Some("无评分笔记"));
    assert!((entity.composite.unwrap() - 5.0).abs() < 1e-6);

    // 验证 frontmatter 其他字段保留（id/title/layer）
    assert!(fm.contains("id: know-000001"));
    assert!(fm.contains("title: 无评分笔记"));
    assert!(fm.contains("layer: knowledge"));
}

/// 端到端：watcher 处理已有 score 笔记（重算 composite 保持一致）
#[tokio::test]
async fn test_e2e_watcher_recalculates_existing_score() {
    let dir = tempdir().unwrap();
    let vault = dir.path().join("vault");
    fs::create_dir_all(&vault).unwrap();
    let state = setup(&vault);

    // 写一个有 score 但 composite 不一致的笔记
    let md = "---\nid: know-000001\ntitle: 测试\nlayer: knowledge\nscore:\n  interest: 80.0\n  strategy: 90.0\n  consensus: 70.0\n  composite: 999.0\n  updated_at: '2026-07-09T00:00:00Z'\n  last_boosted_at: '2026-07-09T00:00:00Z'\n  access_count: 3\n---\n正文\n";
    let path = vault.join("know-000001.md");
    fs::write(&path, md).unwrap();

    watcher::process_single_file(&vault, &state.db, &state.weights, &path)
        .await
        .unwrap();

    // composite 应被重算（80*0.4+90*0.35+70*0.25 = 32+31.5+17.5 = 81）
    let entity = state
        .db
        .lock()
        .await
        .get_entity("know-000001")
        .unwrap()
        .unwrap();
    assert!(
        (entity.composite.unwrap() - 81.0).abs() < 1e-6,
        "composite 应重算为 81"
    );
}

/// 端到端：search 可查到已索引内容
#[tokio::test]
async fn test_e2e_search_after_create() {
    let dir = tempdir().unwrap();
    let vault = dir.path().join("vault");
    fs::create_dir_all(&vault).unwrap();
    let state = setup(&vault);

    api::create_entity(
        axum::extract::State(state.clone()),
        axum::Json(CreateEntityRequest {
            title: "纳什均衡".to_string(),
            layer: "knowledge".to_string(),
            content: Some("game theory nash equilibrium".to_string()),
            interest: Some(80.0),
            strategy: Some(80.0),
            consensus: Some(80.0),
        }),
    )
    .await
    .unwrap();

    let hits = api::search(
        axum::extract::State(state.clone()),
        axum::extract::Query(api::SearchQuery {
            q: "nash".to_string(),
            limit: 10,
        }),
    )
    .await
    .unwrap()
    .0;
    assert_eq!(hits.len(), 1);
    assert!(hits[0].snippet.as_deref().unwrap_or("").contains("nash"));
}
