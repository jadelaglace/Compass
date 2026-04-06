//! Reference parser — extracts [[id]] bidirectional links from markdown content.

use crate::models::{ReferenceInput, ReferenceOutput};
use once_cell::sync::Lazy;
use regex::Regex;

static REF_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"\[\[([a-zA-Z0-9_/-]+)\]\]"#).unwrap()
});

/// Parser for [[id]] style references in markdown.
pub struct ReferenceParser;

impl ReferenceParser {
    /// Extract all unique [[id]] references from content.
    /// If `current_entity_id` is provided, filters out self-references.
    ///
    /// Normalization pipeline (mirrors Python's vault_path_to_entity_id):
    ///   1. lowercase
    ///   2. replace '/' with '-'
    ///   3. collapse multiple dashes
    ///
    /// This ensures [[Projects/compass-v2]] normalizes to "projects-compass-v2"
    /// and matches the entity_id derived from the file path.
    pub fn extract_ids(content: &str, current_entity_id: Option<&str>) -> Vec<String> {
        let re = &*REF_RE;

        let mut ids: Vec<String> = re
            .captures_iter(content)
            .map(|cap| {
                let raw = cap.get(1).unwrap().as_str();
                // Normalize: lowercase, replace '/' with '-', collapse dashes
                let normalized = raw
                    .to_lowercase()
                    .replace('/', "-")
                    .replace('\\', "-");
                // Collapse multiple dashes
                let mut result = String::new();
                let mut prev_dash = false;
                for ch in normalized.chars() {
                    if ch == '-' {
                        if !prev_dash {
                            result.push(ch);
                            prev_dash = true;
                        }
                    } else {
                        result.push(ch);
                        prev_dash = false;
                    }
                }
                result.trim_matches('-').to_string()
            })
            .collect();

        // Deduplicate
        ids.sort();
        ids.dedup();

        // Filter self-reference if specified (normalized already)
        if let Some(self_id) = current_entity_id {
            let normalized_self = self_id.to_lowercase();
            ids.retain(|id| id != &normalized_self);
        }

        ids
    }

    /// Parse input and return output struct.
    pub fn parse(input: ReferenceInput) -> ReferenceOutput {
        let refs = Self::extract_ids(&input.content, input.current_entity_id.as_deref());
        ReferenceOutput { refs }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reference_extraction() {
        let content = "这是 [[entity-001]] 和 [[Entity-002]] 的核心观点";
        let refs = ReferenceParser::extract_ids(content, None);
        assert_eq!(refs, vec!["entity-001", "entity-002"]);  // all lowercase
    }

    #[test]
    fn test_self_reference_filtered() {
        let content = "关于 [[entity-001]] 的讨论";
        let refs = ReferenceParser::extract_ids(content, Some("entity-001"));
        assert!(!refs.contains(&"entity-001".to_string()));
    }

    #[test]
    fn test_self_reference_case_insensitive() {
        // case mismatch: content has "Entity-001", entity_id is "entity-001"
        // should still be filtered because both are normalized to lowercase
        let content = "关于 [[Entity-001]] 的讨论";
        let refs = ReferenceParser::extract_ids(content, Some("entity-001"));
        assert!(refs.is_empty(), "case-insensitive self-ref should be filtered");
    }

    #[test]
    fn test_duplicate_refs_deduplicated() {
        let content = "[[id1]] 和 [[ID1]] 和 [[id2]]";
        let refs = ReferenceParser::extract_ids(content, None);
        // case-insensitive dedup: "id1" and "ID1" collapse to "id1"
        assert_eq!(refs, vec!["id1", "id2"]);
    }

    #[test]
    fn test_no_refs() {
        let content = "这是一段没有引用的普通文本";
        let refs = ReferenceParser::extract_ids(content, None);
        assert!(refs.is_empty());
    }

    #[test]
    fn test_complex_id_with_underscore() {
        let content = "参考 [[comp_v2-2026_04-01]] 这个实体";
        let refs = ReferenceParser::extract_ids(content, None);
        assert_eq!(refs, vec!["comp_v2-2026_04-01"]);
    }

    #[test]
    fn test_cross_folder_wiki_link() {
        let content = "参考 [[Projects/compass-v2]] 这个实体";
        let refs = ReferenceParser::extract_ids(content, None);
        assert_eq!(refs, vec!["projects-compass-v2"]);  // / → - normalized
    }

    #[test]
    fn test_self_reference_cross_folder() {
        // entity_id = "projects-compass-v2", content refs [[Projects/compass-v2]]
        // both normalize to "projects-compass-v2" → should be filtered
        let content = "关于 [[Projects/compass-v2]] 的讨论";
        let refs = ReferenceParser::extract_ids(content, Some("projects-compass-v2"));
        assert!(refs.is_empty(), "cross-folder self-ref should be filtered");
    }
}
