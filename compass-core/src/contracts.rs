//! Phase 4 contracts shared by suggestion, write-back, and report endpoints.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[allow(dead_code)]
pub const CONTENT_HASH_ALGORITHM: &str = "sha256-note-v1";
pub const TAG_ALGORITHM_VERSION: &str = "tags-v1";
pub const RELATED_ALGORITHM_VERSION: &str = "related-v1";
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
pub struct RelatedSuggestion {
    pub suggestion_id: String,
    pub entity_id: String,
    pub id: String,
    pub title: Option<String>,
    pub composite: Option<f64>,
    pub score: f64,
    pub reasons: Vec<String>,
    #[serde(default)]
    pub signals: RelatedSignals,
    pub content_hash: String,
    pub source: String,
    pub algorithm_version: String,
    pub status: SuggestionStatus,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RelatedSignals {
    pub term_overlap: usize,
    pub tag_overlap: usize,
    pub shared_neighbors: usize,
    pub term_signal: f64,
    pub tag_signal: f64,
    pub graph_signal: f64,
    pub composite_signal: f64,
}

impl TagSuggestion {
    pub fn validate(&self) -> Result<()> {
        validate_suggestion_id(&self.suggestion_id)?;
        validate_content_hash(&self.content_hash)?;
        normalize_tag(&self.tag)?;
        if !(0.0..=1.0).contains(&self.confidence) {
            return Err(anyhow!("confidence must be between 0 and 1"));
        }
        validate_stable_id(
            &self.suggestion_id,
            stable_suggestion_id(
                SuggestionKind::Tag,
                &self.entity_id,
                &self.tag,
                &self.content_hash,
                &self.algorithm_version,
                &self.source,
            ),
        )?;
        Ok(())
    }
}

impl RelatedSuggestion {
    pub fn validate(&self) -> Result<()> {
        validate_suggestion_id(&self.suggestion_id)?;
        validate_content_hash(&self.content_hash)?;
        validate_stable_id(
            &self.suggestion_id,
            stable_suggestion_id(
                SuggestionKind::Related,
                &self.entity_id,
                &self.id,
                &self.content_hash,
                &self.algorithm_version,
                &self.source,
            ),
        )
    }
}

fn validate_stable_id(actual: &str, expected: String) -> Result<()> {
    if actual != expected {
        return Err(anyhow!(
            "suggestion_id does not match stable identity; expected {expected}"
        ));
    }
    Ok(())
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
    let fields = [
        kind.as_str().to_string(),
        entity_id.to_string(),
        canonical_candidate(kind, candidate),
        content_hash.to_ascii_lowercase(),
        algorithm_version.to_string(),
        source.to_string(),
    ];
    let mut key = Vec::new();
    for field in fields {
        key.extend_from_slice(field.len().to_string().as_bytes());
        key.push(b':');
        key.extend_from_slice(field.as_bytes());
    }
    format!("sug-{}", hex_digest(&key))
}

pub fn normalize_tag(tag: &str) -> Result<String> {
    let tag = tag.trim();
    if tag.is_empty() {
        return Err(anyhow!("tag cannot be empty"));
    }
    if tag.contains('#') {
        return Err(anyhow!("tag must not contain #"));
    }
    if tag
        .chars()
        .any(|character| character == '\n' || character == '\r')
    {
        return Err(anyhow!("tag must be a single line"));
    }
    Ok(tag.to_string())
}

pub fn candidate_key(kind: SuggestionKind, candidate: &str) -> String {
    canonical_candidate(kind, candidate)
}

fn canonical_candidate(kind: SuggestionKind, candidate: &str) -> String {
    match kind {
        SuggestionKind::Tag => candidate.trim().to_lowercase(),
        SuggestionKind::Related => candidate.to_string(),
    }
}

pub fn validate_suggestion_id(value: &str) -> Result<()> {
    let digest = value
        .strip_prefix("sug-")
        .ok_or_else(|| anyhow!("suggestion_id must start with sug-"))?;
    validate_hex_digest(digest, "suggestion_id")
}

pub fn validate_content_hash(value: &str) -> Result<()> {
    validate_hex_digest(value, "content_hash")
}

fn validate_hex_digest(value: &str, field: &str) -> Result<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(anyhow!(
            "{field} must contain 64 lowercase hexadecimal characters"
        ));
    }
    Ok(())
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
    let mut pending_blank_lines = Vec::new();
    for line in frontmatter.split('\n') {
        if !skipping_score && line.starts_with("score:") {
            skipping_score = true;
            continue;
        }
        if skipping_score {
            if line.is_empty() {
                pending_blank_lines.push(line);
                continue;
            }
            if line.chars().next().is_some_and(char::is_whitespace) {
                pending_blank_lines.clear();
                continue;
            }
            skipping_score = false;
            output.append(&mut pending_blank_lines);
        }
        output.push(line);
    }
    if skipping_score {
        output.append(&mut pending_blank_lines);
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
        assert_eq!(
            first,
            stable_suggestion_id(
                SuggestionKind::Tag,
                "know-1",
                "Decision",
                "hash",
                TAG_ALGORITHM_VERSION,
                "rust_lexical",
            )
        );
        assert_eq!(candidate_key(SuggestionKind::Tag, " Decision "), "decision");
        assert_ne!(
            stable_suggestion_id(
                SuggestionKind::Related,
                "know-1",
                "know-2",
                "hash",
                "related-v1",
                "rust_lexical",
            ),
            stable_suggestion_id(
                SuggestionKind::Related,
                "know-3",
                "know-2",
                "hash",
                "related-v1",
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

        let with_gap = "---\nid: know-1\ntags:\n  - rust\nscore:\n  interest: 99\n\nstatus: active\n---\nbody\n";
        let with_gap_changed_score = with_gap.replace("interest: 99", "interest: 1");
        assert_eq!(
            note_content_hash(with_gap).unwrap(),
            note_content_hash(&with_gap_changed_score).unwrap()
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
        suggestion.validate().unwrap();
        let related: RelatedSuggestion =
            serde_json::from_str(include_str!("../fixtures/phase4/related-suggestion.json"))
                .unwrap();
        related.validate().unwrap();
        let error: ApiError =
            serde_json::from_str(include_str!("../fixtures/phase4/error.json")).unwrap();
        assert_eq!(error.code, "suggestion_expired");
        let report: WeeklyReportFixture =
            serde_json::from_str(include_str!("../fixtures/phase4/weekly-report.json")).unwrap();
        assert_eq!(report.tz, "Asia/Shanghai");
    }
}
