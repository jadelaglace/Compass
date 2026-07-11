//! frontmatter 读写（T1.2）：解析 YAML → 改 score 块 → 原子写回 + 文件锁。
//!
//! 设计：**文本级替换 score 块**，不全量重序列化 frontmatter，以严格保留
//! 其他字段（id/title/tags/...）的原始文本与格式，只动 score 块。
//!
//! 注：文件锁为 advisory（独立 .lock 文件），只防 compass 自身并发写，
//! 不防 Obsidian 等外部编辑器直接改 .md（外部编辑 last-write-wins）。

use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use fs2::FileExt;
use serde_yaml::Value;

use crate::models::{Freshness, Score};

/// 一个笔记：frontmatter 原始文本 + 正文（closing `---` 之后）。
pub struct Note {
    pub frontmatter: String,
    pub body: String,
}

/// Metadata changes that Phase 4 accept operations are allowed to apply.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum MetadataPatch {
    AddTag(String),
    AddLink(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataPatchResult {
    pub changed: bool,
    pub content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetadataPatchError {
    Stale { expected: String, actual: String },
}

impl fmt::Display for MetadataPatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stale { expected, actual } => {
                write!(
                    f,
                    "content hash is stale: expected {expected}, actual {actual}"
                )
            }
        }
    }
}

impl std::error::Error for MetadataPatchError {}

/// 读取 .md 文件，拆分 frontmatter 与正文。
pub fn read_note(path: &Path) -> Result<Note> {
    let content =
        fs::read_to_string(path).with_context(|| format!("读取文件失败 {}", path.display()))?;
    let (frontmatter, body) = split_frontmatter(&content)?;
    Ok(Note { frontmatter, body })
}

/// Returns true when frontmatter still contains a Templater expression.
/// These notes are source templates rather than indexable entities.
pub fn has_unrendered_templater_marker(frontmatter: &str) -> bool {
    frontmatter.contains("<%") || frontmatter.contains("%>")
}

/// Apply the selected metadata changes under the same advisory lock used by score writes.
/// The expected hash is computed from the raw authoritative Markdown note, excluding only
/// the top-level score block as defined by `sha256-note-v1`.
pub fn patch_metadata(
    path: &Path,
    expected_hash: &str,
    patches: &[MetadataPatch],
) -> Result<MetadataPatchResult> {
    let lock_path = path.with_extension("md.lock");
    let lock_file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&lock_path)
        .with_context(|| format!("打开锁文件失败 {}", lock_path.display()))?;
    lock_file.lock_exclusive().context("获取文件锁失败")?;

    let result = (|| -> Result<MetadataPatchResult> {
        let original = fs::read_to_string(path)?;
        let actual_hash = crate::contracts::note_content_hash(&original)?;
        if actual_hash != expected_hash {
            return Err(MetadataPatchError::Stale {
                expected: expected_hash.to_string(),
                actual: actual_hash,
            }
            .into());
        }

        let newline = if original.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        let has_bom = original.starts_with('\u{feff}');
        let normalized = original
            .strip_prefix('\u{feff}')
            .unwrap_or(&original)
            .replace("\r\n", "\n")
            .replace('\r', "\n");
        let had_trailing_newline = normalized.ends_with('\n');
        let (frontmatter, body) = split_frontmatter(&normalized)?;
        let mut new_frontmatter = frontmatter.clone();
        let mut new_body = body.clone();
        let mut changed = false;

        for patch in patches {
            match patch {
                MetadataPatch::AddTag(tag) => {
                    let tag = crate::contracts::normalize_tag(tag)?;
                    let existing = extract_tags(&new_frontmatter);
                    if !existing
                        .iter()
                        .any(|value| value.eq_ignore_ascii_case(&tag))
                    {
                        new_frontmatter = add_tag(&new_frontmatter, &tag);
                        changed = true;
                    }
                }
                MetadataPatch::AddLink(target) => {
                    let target = target.trim();
                    if target.is_empty()
                        || target.contains('\n')
                        || target.contains('\r')
                        || target.contains(']')
                    {
                        return Err(anyhow!("link target must be a non-empty single line"));
                    }
                    if !extract_refs(&new_body).iter().any(|value| value == target) {
                        if !new_body.is_empty() {
                            new_body.push_str("\n\n");
                        }
                        new_body.push_str("[[");
                        new_body.push_str(target);
                        new_body.push_str("]]\n");
                        changed = true;
                    }
                }
            }
        }

        if !changed {
            return Ok(MetadataPatchResult {
                changed: false,
                content_hash: actual_hash,
            });
        }

        let mut new_content = format!("---\n{}\n---", new_frontmatter.trim_end_matches('\n'));
        if !new_body.is_empty() {
            new_content.push('\n');
            new_content.push_str(&new_body);
        }
        if had_trailing_newline && !new_content.ends_with('\n') {
            new_content.push('\n');
        }
        if has_bom {
            new_content.insert(0, '\u{feff}');
        }
        let new_content = if newline == "\r\n" {
            new_content.replace('\n', "\r\n")
        } else {
            new_content
        };
        atomic_write(path, &new_content)?;
        let content_hash = crate::contracts::note_content_hash(&new_content)?;
        Ok(MetadataPatchResult {
            changed: true,
            content_hash,
        })
    })();
    let _ = lock_file.unlock();
    result
}

fn add_tag(frontmatter: &str, tag: &str) -> String {
    let lines: Vec<&str> = frontmatter.lines().collect();
    let mut tags = extract_tags(frontmatter);
    tags.push(tag.to_string());
    let mut block = vec!["tags:".to_string()];
    block.extend(tags.into_iter().map(|value| {
        let serialized = serde_yaml::to_string(&value)
            .unwrap_or_else(|_| format!("'{}'", value.replace('\'', "''")))
            .trim()
            .to_string();
        format!("  - {serialized}")
    }));
    let start = lines.iter().position(|line| line.starts_with("tags:"));
    let insert_at = start.or_else(|| lines.iter().position(|line| line.starts_with("score:")));

    let mut output = Vec::new();
    match insert_at {
        Some(start_idx) if start.is_some() => {
            let mut end_idx = lines.len();
            for (idx, line) in lines.iter().enumerate().skip(start_idx + 1) {
                if !line.is_empty() && !line.starts_with(' ') && !line.starts_with('\t') {
                    end_idx = idx;
                    break;
                }
            }
            output.extend(lines[..start_idx].iter().map(|line| (*line).to_string()));
            output.extend(block);
            output.extend(lines[end_idx..].iter().map(|line| (*line).to_string()));
        }
        Some(score_idx) => {
            output.extend(lines[..score_idx].iter().map(|line| (*line).to_string()));
            output.extend(block);
            output.extend(lines[score_idx..].iter().map(|line| (*line).to_string()));
        }
        None => {
            output.extend(lines.iter().map(|line| (*line).to_string()));
            output.extend(block);
        }
    }
    output.join("\n")
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
    match m.get(Value::String("score".into())) {
        Some(v) => Ok(Some(
            serde_yaml::from_value(v.clone()).context("解析 score 块失败")?,
        )),
        None => Ok(None),
    }
}

pub fn get_freshness(frontmatter: &str) -> Result<Freshness> {
    let fm: Value = serde_yaml::from_str(frontmatter).context("解析 frontmatter 失败")?;
    let Some(mapping) = fm.as_mapping() else {
        return Err(anyhow!("frontmatter 不是 mapping"));
    };
    match mapping.get(Value::String("freshness".into())) {
        Some(value) => serde_yaml::from_value(value.clone()).context("解析 freshness 失败"),
        None => Ok(Freshness::default()),
    }
}

pub fn content_updated_at(frontmatter: &str) -> Result<Option<String>> {
    let fm: Value = serde_yaml::from_str(frontmatter).context("解析 frontmatter 失败")?;
    let Some(mapping) = fm.as_mapping() else {
        return Err(anyhow!("frontmatter 不是 mapping"));
    };
    Ok(mapping
        .get(Value::String("content_updated_at".into()))
        .or_else(|| mapping.get(Value::String("updated_at".into())))
        .and_then(Value::as_str)
        .map(str::to_string))
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
            for (i, l) in lines.iter().enumerate().skip(start_idx + 1) {
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

/// ????? `[[id]]` wiki-link ?????? watcher ???? /graph ????
pub fn extract_refs(body: &str) -> Vec<String> {
    let re = regex::Regex::new(r"\[\[([^\]]+)\]\]").unwrap();
    re.captures_iter(body)
        .filter_map(|c| {
            let raw = c.get(1).unwrap().as_str();
            let target = raw.split('|').next().unwrap_or(raw);
            let target = target.split('#').next().unwrap_or(target).trim();
            (!target.is_empty()).then(|| target.to_string())
        })
        .collect()
}

/// Extract frontmatter tags as canonical values without a leading `#`.
pub fn extract_tags(frontmatter: &str) -> Vec<String> {
    let Ok(value) = serde_yaml::from_str::<Value>(frontmatter) else {
        return Vec::new();
    };
    let Some(tags) = value
        .as_mapping()
        .and_then(|mapping| mapping.get(Value::String("tags".into())))
    else {
        return Vec::new();
    };
    let values = match tags {
        Value::Sequence(values) => values.iter().filter_map(Value::as_str).collect(),
        Value::String(value) => vec![value.as_str()],
        _ => Vec::new(),
    };
    let mut result: Vec<String> = Vec::new();
    for value in values {
        let value = value.trim().trim_start_matches('#').trim();
        if !value.is_empty()
            && !result
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(value))
        {
            result.push(value.to_string());
        }
    }
    result
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
        s.weights = Some(Weights {
            interest: 0.5,
            strategy: 0.3,
            consensus: 0.2,
        });
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

    #[test]
    fn test_extract_tags_normalizes_and_deduplicates() {
        let tags = extract_tags("tags:\n  - Rust\n  - '#rust'\n  - sqlite\n");
        assert_eq!(tags, vec!["Rust", "sqlite"]);
        assert_eq!(extract_tags("tags: knowledge\n"), vec!["knowledge"]);
    }

    #[test]
    fn test_extract_refs_normalizes_aliases_and_headings() {
        assert_eq!(
            extract_refs("[[know-1|Display]] [[know-2#Heading]] [[know-3]]"),
            vec!["know-1", "know-2", "know-3"]
        );
    }

    #[test]
    fn test_patch_metadata_preserves_bom_crlf_score_and_body() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("note.md");
        let original = format!("\u{feff}{}", sample_md().replace('\n', "\r\n"));
        fs::write(&path, &original).unwrap();
        let expected = crate::contracts::note_content_hash(&original).unwrap();

        let result = patch_metadata(
            &path,
            &expected,
            &[
                MetadataPatch::AddTag("Rust".to_string()),
                MetadataPatch::AddLink("know-000003".to_string()),
            ],
        )
        .unwrap();
        assert!(result.changed);

        let after = fs::read_to_string(&path).unwrap();
        assert!(after.starts_with('\u{feff}'));
        assert!(after.contains("\r\n"));
        assert!(after.contains("  - math\r\n  - strategy\r\n  - Rust"));
        assert!(after.contains("access_count: 0"));
        assert!(after.contains("# Game Theory\r\n"));
        assert!(after.ends_with("[[know-000003]]\r\n"));
        assert_eq!(
            crate::contracts::note_content_hash(&after).unwrap(),
            result.content_hash
        );
    }

    #[test]
    fn test_patch_metadata_stale_does_not_modify_file_and_repeat_is_idempotent() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("note.md");
        fs::write(&path, sample_md()).unwrap();
        let original = fs::read_to_string(&path).unwrap();
        let stale = "0".repeat(64);
        let error = patch_metadata(&path, &stale, &[MetadataPatch::AddTag("Rust".to_string())])
            .unwrap_err();
        assert!(error.downcast_ref::<MetadataPatchError>().is_some());
        assert_eq!(fs::read_to_string(&path).unwrap(), original);

        let expected = crate::contracts::note_content_hash(&original).unwrap();
        let first = patch_metadata(
            &path,
            &expected,
            &[MetadataPatch::AddTag("Rust".to_string())],
        )
        .unwrap();
        let after_first = fs::read_to_string(&path).unwrap();
        let second = patch_metadata(
            &path,
            &first.content_hash,
            &[MetadataPatch::AddTag("rust".to_string())],
        )
        .unwrap();
        assert!(!second.changed);
        assert_eq!(second.content_hash, first.content_hash);
        assert_eq!(fs::read_to_string(&path).unwrap(), after_first);
    }
}
