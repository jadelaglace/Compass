//! frontmatter 读写（T1.2）：解析 YAML → 改 score 块 → 原子写回 + 文件锁。
//!
//! 设计：**文本级替换 score 块**，不全量重序列化 frontmatter，以严格保留
//! 其他字段（id/title/tags/...）的原始文本与格式，只动 score 块。
//!
//! 注：文件锁为 advisory（独立 .lock 文件），只防 compass 自身并发写，
//! 不防 Obsidian 等外部编辑器直接改 .md（外部编辑 last-write-wins）。

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use fs2::FileExt;
use serde_yaml::Value;

use crate::models::Score;

/// 一个笔记：frontmatter 原始文本 + 正文（closing `---` 之后）。
pub struct Note {
    pub frontmatter: String,
    pub body: String,
}

/// 读取 .md 文件，拆分 frontmatter 与正文。
pub fn read_note(path: &Path) -> Result<Note> {
    let content = fs::read_to_string(path).with_context(|| format!("读取文件失败 {}", path.display()))?;
    let (frontmatter, body) = split_frontmatter(&content)?;
    Ok(Note { frontmatter, body })
}

/// 拆分 `---\n<yaml>\n---\n<body>`。
pub fn split_frontmatter(content: &str) -> Result<(String, String)> {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let lines: Vec<&str> = content.lines().collect();
    if lines.first().map(|l| l.trim() != "---").unwrap_or(true) {
        return Err(anyhow!("缺少开头的 ---"));
    }
    let mut close_idx = None;
    for (i, line) in lines.iter().enumerate().skip(1) {
        if line.trim() == "---" {
            close_idx = Some(i);
            break;
        }
    }
    let close_idx = close_idx.ok_or_else(|| anyhow!("缺少结尾的 ---"))?;
    let frontmatter: String = lines[1..close_idx].join("\n");
    let body: String = lines[close_idx + 1..].join("\n");
    Ok((frontmatter, body))
}

/// 从 frontmatter 文本解析 score 块。无 score 返回 None。
pub fn get_score(frontmatter: &str) -> Result<Option<Score>> {
    let fm: Value = serde_yaml::from_str(frontmatter).context("解析 frontmatter 失败")?;
    let Some(m) = fm.as_mapping() else {
        return Err(anyhow!("frontmatter 不是 mapping"));
    };
    match m.get(&Value::String("score".into())) {
        Some(v) => Ok(Some(
            serde_yaml::from_value(v.clone()).context("解析 score 块失败")?,
        )),
        None => Ok(None),
    }
}

/// 把 Score 序列化为 score 块文本（`score:\n  field: ...\n...`）。
fn score_to_block(score: &Score) -> Result<String> {
    let yaml = serde_yaml::to_string(score).context("序列化 score 失败")?;
    // yaml 形如 "interest: 85.0\nstrategy: ...\n"，每行缩进 2 空格作为 score 子字段
    let indented: String = yaml
        .lines()
        .map(|l| format!("  {l}"))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(format!("score:\n{indented}\n"))
}

/// 在 frontmatter 文本中替换（或追加）score 块，保留其他字段原文。
pub fn replace_score_block(frontmatter: &str, new_block: &str) -> String {
    let lines: Vec<&str> = frontmatter.lines().collect();
    // 找顶层 `score:` 行（无前导空白）
    let start = lines.iter().position(|l| l.starts_with("score:"));

    let mut out = String::new();
    match start {
        None => {
            // 不存在：追加到末尾
            out.push_str(frontmatter);
            if !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(new_block);
        }
        Some(start_idx) => {
            // score 块结束 = 下一个顶层 key（非空且无前导空白）或文件尾
            let mut end_idx = lines.len();
            for i in (start_idx + 1)..lines.len() {
                let l = lines[i];
                if !l.is_empty() && !l.starts_with(' ') && !l.starts_with('\t') {
                    end_idx = i;
                    break;
                }
            }
            // 保留 score 之前
            for l in &lines[..start_idx] {
                out.push_str(l);
                out.push('\n');
            }
            // 新 score 块（new_block 自带末尾 \n）
            out.push_str(new_block);
            // 保留 score 之后
            for l in &lines[end_idx..] {
                out.push_str(l);
                out.push('\n');
            }
        }
    }
    // 去掉末尾多余空行（保留最多一个）
    out.trim_end_matches('\n').to_string() + "\n"
}

/// 写回 score：读 → 文本替换 score 块 → 原子写 + 文件锁。
pub fn write_score(path: &Path, score: &Score) -> Result<()> {
    // 锁独立 .lock 文件（advisory），避免锁数据文件与 rename 冲突（Windows os error 33）
    let lock_path = path.with_extension("md.lock");
    let lock_file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&lock_path)
        .with_context(|| format!("打开锁文件失败 {}", lock_path.display()))?;
    lock_file.lock_exclusive().context("获取文件锁失败")?;
    let result = (|| -> Result<()> {
        let content = fs::read_to_string(path)?;
        let (frontmatter, body) = split_frontmatter(&content)?;
        let new_block = score_to_block(score)?;
        let new_frontmatter = replace_score_block(&frontmatter, &new_block);
        let new_content = format!("---\n{new_frontmatter}---\n{body}");
        atomic_write(path, &new_content)
    })();
    let _ = lock_file.unlock();
    result
}

/// 原子写：写到 .tmp → rename 覆盖（Windows MOVEFILE_REPLACE_EXISTING）。
fn atomic_write(path: &Path, content: &str) -> Result<()> {
    let tmp: PathBuf = path.with_extension("md.tmp");
    {
        let mut f = fs::File::create(&tmp).context("创建 tmp 文件失败")?;
        f.write_all(content.as_bytes()).context("写 tmp 失败")?;
        f.sync_all().ok();
    }
    fs::rename(&tmp, path).context("rename 覆盖失败")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Weights;
    use std::fs;
    use tempfile::tempdir;

    fn sample_score() -> Score {
        Score {
            interest: 8.5,
            strategy: 9.0,
            consensus: 7.0,
            composite: 8.2,
            weights: None,
            updated_at: "2026-07-05T10:00:00+08:00".into(),
            last_boosted_at: "2026-07-05T10:00:00+08:00".into(),
            access_count: 3,
        }
    }

    fn sample_md() -> String {
        "---\n\
         id: know-000001\n\
         title: Game Theory\n\
         category: Knowledge\n\
         tags:\n  - math\n  - strategy\n\
         score:\n  interest: 5.0\n  strategy: 5.0\n  consensus: 0.0\n  composite: 3.2\n  updated_at: '2026-01-01T00:00:00+08:00'\n  last_boosted_at: '2026-01-01T00:00:00+08:00'\n  access_count: 0\n\
         status: active\n\
         ---\n\
         # Game Theory\n\n\
         ```mermaid\ngraph TD\nA-->B\n```\n\n\
         正文涉及 [[know-000002]]。\n"
            .to_string()
    }

    #[test]
    fn test_split_frontmatter() {
        let (fm, body) = split_frontmatter(&sample_md()).unwrap();
        assert!(fm.contains("id: know-000001"));
        assert!(fm.contains("score:"));
        assert!(body.contains("# Game Theory"));
        assert!(body.contains("```mermaid"));
    }

    #[test]
    fn test_get_score() {
        let (fm, _) = split_frontmatter(&sample_md()).unwrap();
        let s = get_score(&fm).unwrap().unwrap();
        assert!((s.interest - 5.0).abs() < 1e-9);
        assert_eq!(s.access_count, 0);
    }

    #[test]
    fn test_replace_score_preserves_other_fields() {
        let (fm, _) = split_frontmatter(&sample_md()).unwrap();
        let new_block = score_to_block(&sample_score()).unwrap();
        let new_fm = replace_score_block(&fm, &new_block);
        // 其他字段原文保留
        assert!(new_fm.contains("id: know-000001"));
        assert!(new_fm.contains("title: Game Theory"));
        assert!(new_fm.contains("category: Knowledge"));
        assert!(new_fm.contains("- math"));
        assert!(new_fm.contains("status: active"));
        // score 已更新
        let s = get_score(&new_fm).unwrap().unwrap();
        assert!((s.interest - 8.5).abs() < 1e-9);
        assert!((s.composite - 8.2).abs() < 1e-9);
        assert_eq!(s.access_count, 3);
    }

    #[test]
    fn test_replace_score_when_absent() {
        let fm = "id: x\ntitle: y\n".to_string();
        let new_block = score_to_block(&sample_score()).unwrap();
        let new_fm = replace_score_block(&fm, &new_block);
        assert!(new_fm.contains("id: x"));
        assert!(new_fm.contains("score:"));
        let s = get_score(&new_fm).unwrap().unwrap();
        assert!((s.interest - 8.5).abs() < 1e-9);
    }

    #[test]
    fn test_write_score_end_to_end_preserves_body() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("note.md");
        fs::write(&path, sample_md()).unwrap();

        write_score(&path, &sample_score()).unwrap();

        let after = fs::read_to_string(&path).unwrap();
        // 正文/Mermaid 完全不变
        assert!(after.contains("# Game Theory"));
        assert!(after.contains("```mermaid\ngraph TD\nA-->B\n```"));
        assert!(after.contains("[[know-000002]]"));
        // 其他 frontmatter 字段保留
        assert!(after.contains("id: know-000001"));
        assert!(after.contains("status: active"));
        // score 更新
        let (fm, _) = split_frontmatter(&after).unwrap();
        let s = get_score(&fm).unwrap().unwrap();
        assert!((s.interest - 8.5).abs() < 1e-9);
        assert_eq!(s.access_count, 3);
    }

    #[test]
    fn test_write_score_with_weights() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("note.md");
        fs::write(&path, sample_md()).unwrap();

        let mut s = sample_score();
        s.weights = Some(Weights { interest: 0.5, strategy: 0.3, consensus: 0.2 });
        write_score(&path, &s).unwrap();

        let after = fs::read_to_string(&path).unwrap();
        let (fm, _) = split_frontmatter(&after).unwrap();
        let s2 = get_score(&fm).unwrap().unwrap();
        let w = s2.weights.unwrap();
        assert!((w.interest - 0.5).abs() < 1e-9);
    }

    #[test]
    fn test_rejects_missing_frontmatter() {
        let r = split_frontmatter("no frontmatter here");
        assert!(r.is_err());
    }

    #[test]
    fn test_write_score_round_trip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("note.md");
        fs::write(&path, sample_md()).unwrap();
        write_score(&path, &sample_score()).unwrap();
        // round-trip: read_note 能重新解析
        let note = read_note(&path).unwrap();
        let s = get_score(&note.frontmatter).unwrap().unwrap();
        assert!((s.interest - 8.5).abs() < 1e-9);
        assert_eq!(s.access_count, 3);
        assert!(note.body.contains("# Game Theory"));
    }

    #[test]
    fn test_split_frontmatter_handles_bom() {
        let md = format!("\u{feff}{}", sample_md());
        let (fm, body) = split_frontmatter(&md).unwrap();
        assert!(fm.contains("id: know-000001"));
        assert!(body.contains("# Game Theory"));
    }
}