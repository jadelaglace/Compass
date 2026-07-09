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
use crate::db::{Db, EntityRow};
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

// ============ T2.5 Phase 2 验收 ============

/// 30天衰减曲线：验证衰减随天数递增而下降，且方向一致
#[tokio::test]
async fn test_p2_decay_curve_30_days() {
    use crate::config::DecayConfig;
    use crate::scheduler::DecayScheduler;

    let dir = tempdir().unwrap();
    let vault = dir.path().join("vault");
    fs::create_dir_all(&vault).unwrap();
    let db = Arc::new(Mutex::new(Db::open_in_memory().unwrap()));
    let weights = Weights::default();
    let decay = DecayConfig {
        daily_rate: 0.98,
        floor: 0.5,
        boost_protection_days: 3,
        direction_layer_factor: 0.5,
    };
    let scheduler = DecayScheduler::new(db.clone(), vault.clone(), weights, decay);

    // 创建 5 个实体，last_boosted 分别 10/20/30/60/100 天前
    let now = chrono::Utc::now();
    for (i, days_ago) in [10i64, 20, 30, 60, 100].iter().enumerate() {
        let last = now - chrono::Duration::days(*days_ago);
        let id = format!("know-{i}");
        let md = format!(
            "---\nid: {id}\ntitle: T\nlayer: knowledge\nstatus: active\nscore:\n  interest: 90.0\n  strategy: 50.0\n  consensus: 50.0\n  composite: 70.0\n  updated_at: '{}'\n  last_boosted_at: '{}'\n  access_count: 0\n---\nbody\n",
            last.to_rfc3339(), last.to_rfc3339()
        );
        fs::write(vault.join(format!("{id}.md")), md).unwrap();
        let entity = EntityRow {
            id: id.clone(),
            file_path: format!("{id}.md"),
            title: Some("T".to_string()),
            layer: Some("knowledge".to_string()),
            status: Some("active".to_string()),
            interest: Some(90.0),
            strategy: Some(50.0),
            consensus: Some(50.0),
            composite: Some(70.0),
            access_count: 0,
            last_boosted_at: Some(last.to_rfc3339()),
            content_hash: Some("abc".to_string()),
            updated_at: Some(last.to_rfc3339()),
        };
        db.lock().await.upsert_entity(&entity, "body").unwrap();
    }

    let result = scheduler.run_once().await.unwrap();
    assert!(result.decayed >= 4, "至少4个应衰减（100天的可能到地板）");

    // 验证衰减曲线：天数越大，interest 下降越多
    let db_guard = db.lock().await;
    let mut prev_interest = 100.0;
    for i in 0..5 {
        let entity = db_guard.get_entity(&format!("know-{i}")).unwrap().unwrap();
        let interest = entity.interest.unwrap();
        // 衰减天数越大，interest 越低（单调递减）
        assert!(
            interest <= prev_interest + 1e-6,
            "know-{i} interest={interest} 应 <= prev={prev_interest}（衰减曲线单调递减）"
        );
        prev_interest = interest;
    }

    // 100天的应到地板附近（90*0.5=45）
    let entity_100 = db_guard.get_entity("know-4").unwrap().unwrap();
    let interest_100 = entity_100.interest.unwrap();
    assert!(
        interest_100 <= 46.0,
        "100天衰减应接近地板45，实际 {interest_100}"
    );
}

/// Feed 三模式端到端：explore/consolidate/strategic 排序正确
#[tokio::test]
async fn test_p2_feed_three_modes_e2e() {
    use crate::api;

    let dir = tempdir().unwrap();
    let vault = dir.path().join("vault");
    fs::create_dir_all(&vault).unwrap();
    let state = api::AppState {
        db: Arc::new(Mutex::new(Db::open_in_memory().unwrap())),
        vault: vault.clone(),
        weights: Weights::default(),
    };

    // know-a: composite高 strategy低 last_boosted久
    // know-b: composite中 strategy高 last_boosted最近
    // know-c: composite低 strategy中 last_boosted最久
    let entities = vec![
        ("know-a", 80.0, 30.0, "2026-01-01T00:00:00Z"),
        ("know-b", 60.0, 90.0, "2026-07-08T00:00:00Z"),
        ("know-c", 40.0, 50.0, "2025-12-01T00:00:00Z"),
    ];

    for (id, comp, strat, boosted) in &entities {
        let entity = EntityRow {
            id: id.to_string(),
            file_path: format!("{id}.md"),
            title: Some(id.to_string()),
            layer: Some("knowledge".to_string()),
            status: Some("active".to_string()),
            interest: Some(50.0),
            strategy: Some(*strat),
            consensus: Some(50.0),
            composite: Some(*comp),
            access_count: 0,
            last_boosted_at: Some(boosted.to_string()),
            content_hash: Some("abc".to_string()),
            updated_at: Some("2026-07-09T00:00:00Z".to_string()),
        };
        state
            .db
            .lock()
            .await
            .upsert_entity(&entity, "body")
            .unwrap();
    }

    // explore: composite 降序 -> a(80) b(60) c(40)
    let q = api::FeedQuery {
        mode: "explore".to_string(),
        limit: 10,
    };
    let r = api::feed(axum::extract::State(state.clone()), axum::extract::Query(q))
        .await
        .unwrap()
        .0;
    assert_eq!(r[0].id, "know-a");
    assert_eq!(r[1].id, "know-b");
    assert_eq!(r[2].id, "know-c");

    // strategic: strategy 降序 -> b(90) c(50) a(30)
    let q = api::FeedQuery {
        mode: "strategic".to_string(),
        limit: 10,
    };
    let r = api::feed(axum::extract::State(state.clone()), axum::extract::Query(q))
        .await
        .unwrap()
        .0;
    assert_eq!(r[0].id, "know-b");
    assert_eq!(r[1].id, "know-c");
    assert_eq!(r[2].id, "know-a");

    // consolidate: last_boosted 升序 -> c(2025-12) a(2026-01) b(2026-07)
    let q = api::FeedQuery {
        mode: "consolidate".to_string(),
        limit: 10,
    };
    let r = api::feed(axum::extract::State(state.clone()), axum::extract::Query(q))
        .await
        .unwrap()
        .0;
    assert_eq!(r[0].id, "know-c");
    assert_eq!(r[1].id, "know-a");
    assert_eq!(r[2].id, "know-b");
}

/// Graph 节点大小=composite：验证 /graph 返回的节点含 composite 字段
#[tokio::test]
async fn test_p2_graph_node_size_equals_score() {
    use crate::api;

    let dir = tempdir().unwrap();
    let vault = dir.path().join("vault");
    fs::create_dir_all(&vault).unwrap();
    let state = api::AppState {
        db: Arc::new(Mutex::new(Db::open_in_memory().unwrap())),
        vault: vault.clone(),
        weights: Weights::default(),
    };

    // 两个实体：高分 + 低分
    let high = EntityRow {
        id: "know-high".to_string(),
        file_path: "high.md".to_string(),
        title: Some("High".to_string()),
        layer: Some("knowledge".to_string()),
        status: Some("active".to_string()),
        interest: Some(90.0),
        strategy: Some(90.0),
        consensus: Some(90.0),
        composite: Some(90.0),
        access_count: 0,
        last_boosted_at: Some("2026-07-09T00:00:00Z".to_string()),
        content_hash: Some("h".to_string()),
        updated_at: Some("2026-07-09T00:00:00Z".to_string()),
    };
    let low = EntityRow {
        id: "know-low".to_string(),
        file_path: "low.md".to_string(),
        title: Some("Low".to_string()),
        layer: Some("knowledge".to_string()),
        status: Some("active".to_string()),
        interest: Some(10.0),
        strategy: Some(10.0),
        consensus: Some(10.0),
        composite: Some(10.0),
        access_count: 0,
        last_boosted_at: Some("2026-07-09T00:00:00Z".to_string()),
        content_hash: Some("l".to_string()),
        updated_at: Some("2026-07-09T00:00:00Z".to_string()),
    };
    state.db.lock().await.upsert_entity(&high, "body").unwrap();
    state.db.lock().await.upsert_entity(&low, "body").unwrap();

    let result = api::graph(axum::extract::State(state)).await.unwrap().0;
    assert_eq!(result.nodes.len(), 2);

    // 验证节点含 composite 字段（Web D3 用它定节点大小）
    let high_node = result.nodes.iter().find(|n| n.id == "know-high").unwrap();
    let low_node = result.nodes.iter().find(|n| n.id == "know-low").unwrap();
    assert!((high_node.composite.unwrap() - 90.0).abs() < 1e-9);
    assert!((low_node.composite.unwrap() - 10.0).abs() < 1e-9);
    // 高分节点 composite > 低分节点（D3 会渲染更大）
    assert!(high_node.composite.unwrap() > low_node.composite.unwrap());
}
