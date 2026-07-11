//! Phase 4 contracts shared by suggestion, write-back, and report endpoints.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[allow(dead_code)]
pub const CONTENT_HASH_ALGORITHM: &str = "sha256-note-v1";
pub const TAG_ALGORITHM_VERSION: &str = "tags-v1";
#[allow(dead_code)]
pub const MAX_SUGGESTIONS_PER_REQUEST: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuggestionKind {
    Tag,
    Related,
}

impl SuggestionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Tag => "tag",
            Self::Related => "related",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuggestionStatus {
    Pending,
    Accepted,
    Rejected,
    Expired,
}

pub fn can_transition(from: SuggestionStatus, to: SuggestionStatus) -> bool {
    from == SuggestionStatus::Pending
        && matches!(
            to,
            SuggestionStatus::Accepted | SuggestionStatus::Rejected | SuggestionStatus::Expired
        )
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TagSuggestion {
    pub suggestion_id: String,
    pub entity_id: String,
    pub tag: String,
    pub confidence: f64,
    pub reason: String,
    pub source: String,
    pub algorithm_version: String,
    pub content_hash: String,
    pub status: SuggestionStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    pub details: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeeklyReportFixture {
    pub from: String,
    pub to: String,
    pub tz: String,
    pub generated_at: String,
    pub data_quality: DataQuality,
    pub score_changes: Vec<ScoreChangeFixture>,
    pub access_count: u64,
    pub new_entities: Vec<String>,
    pub suggestion_stats: SuggestionStats,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataQuality {
    pub history_unavailable: bool,
    pub missing: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScoreChangeFixture {
    pub entity_id: String,
    pub delta: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuggestionStats {
    pub accepted: u64,
    pub rejected: u64,
    pub expired: u64,
}

pub fn stable_suggestion_id(
    kind: SuggestionKind,
    entity_id: &str,
    candidate: &str,
    content_hash: &str,
    algorithm_version: &str,
    source: &str,
) -> String {
    let key = [
        kind.as_str(),
        entity_id,
        candidate,
        content_hash,
        algorithm_version,
        source,
    ]
    .join("\n");
    format!("sug-{}", hex_digest(key.as_bytes()))
}

pub fn normalize_tag(tag: &str) -> Result<String> {
    let tag = tag.trim();
    if tag.is_empty() {
        return Err(anyhow!("tag cannot be empty"));
    }
    if tag.contains('#') {
        return Err(anyhow!("tag must not contain #"));
    }
    Ok(tag.to_string())
}

/// Hash note metadata and body while excluding the mutable top-level `score` block.
pub fn note_content_hash(note: &str) -> Result<String> {
    let normalized = note
        .strip_prefix('\u{feff}')
        .unwrap_or(note)
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    let rest = normalized
        .strip_prefix("---\n")
        .ok_or_else(|| anyhow!("missing frontmatter opening delimiter"))?;
    let (frontmatter, body) = rest
        .split_once("\n---\n")
        .ok_or_else(|| anyhow!("missing frontmatter closing delimiter"))?;
    let canonical_frontmatter = without_top_level_score(frontmatter);
    let canonical = format!("{canonical_frontmatter}\n---\n{body}");
    Ok(hex_digest(canonical.as_bytes()))
}

fn without_top_level_score(frontmatter: &str) -> String {
    let mut output = Vec::new();
    let mut skipping_score = false;
    for line in frontmatter.split('\n') {
        if !skipping_score && line.starts_with("score:") {
            skipping_score = true;
            continue;
        }
        if skipping_score {
            if line.is_empty() || line.starts_with(char::is_whitespace) {
                continue;
            }
            skipping_score = false;
        }
        output.push(line);
    }
    output.join("\n")
}

fn hex_digest(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_id_is_deterministic_and_input_sensitive() {
        let first = stable_suggestion_id(
            SuggestionKind::Tag,
            "know-1",
            "decision",
            "hash",
            TAG_ALGORITHM_VERSION,
            "rust_lexical",
        );
        assert_eq!(first.len(), 68);
        assert_eq!(
            first,
            stable_suggestion_id(
                SuggestionKind::Tag,
                "know-1",
                "decision",
                "hash",
                TAG_ALGORITHM_VERSION,
                "rust_lexical",
            )
        );
        assert_ne!(
            first,
            stable_suggestion_id(
                SuggestionKind::Tag,
                "know-1",
                "other",
                "hash",
                TAG_ALGORITHM_VERSION,
                "rust_lexical",
            )
        );
    }

    #[test]
    fn status_transitions_only_leave_pending_once() {
        assert!(can_transition(
            SuggestionStatus::Pending,
            SuggestionStatus::Accepted
        ));
        assert!(can_transition(
            SuggestionStatus::Pending,
            SuggestionStatus::Rejected
        ));
        assert!(!can_transition(
            SuggestionStatus::Accepted,
            SuggestionStatus::Rejected
        ));
    }

    #[test]
    fn note_hash_ignores_score_and_line_endings_but_not_metadata_or_body() {
        let first = "\u{feff}---\r\nid: know-1\r\ntags:\r\n  - rust\r\nscore:\r\n  interest: 1\r\n  composite: 2\r\nstatus: active\r\n---\r\nbody\r\n";
        let second = "---\nid: know-1\ntags:\n  - rust\nscore:\n  interest: 99\n  composite: 88\nstatus: active\n---\nbody\n";
        assert_eq!(
            note_content_hash(first).unwrap(),
            note_content_hash(second).unwrap()
        );

        let changed = second.replace("- rust", "- rust\n  - sqlite");
        assert_ne!(
            note_content_hash(second).unwrap(),
            note_content_hash(&changed).unwrap()
        );
    }

    #[test]
    fn tags_are_normalized_without_hash_prefixes() {
        assert_eq!(
            normalize_tag("  decision science ").unwrap(),
            "decision science"
        );
        assert!(normalize_tag("#decision").is_err());
        assert!(normalize_tag(" ").is_err());
    }

    #[test]
    fn phase4_fixtures_match_the_frozen_contracts() {
        let suggestion: TagSuggestion =
            serde_json::from_str(include_str!("../fixtures/phase4/tag-suggestion.json")).unwrap();
        assert_eq!(suggestion.status, SuggestionStatus::Pending);
        let error: ApiError =
            serde_json::from_str(include_str!("../fixtures/phase4/error.json")).unwrap();
        assert_eq!(error.code, "suggestion_expired");
        let report: WeeklyReportFixture =
            serde_json::from_str(include_str!("../fixtures/phase4/weekly-report.json")).unwrap();
        assert_eq!(report.tz, "Asia/Shanghai");
    }
}
