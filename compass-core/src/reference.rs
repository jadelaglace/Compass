//! Reference parser — extracts [[id]] bidirectional links from markdown content.

use crate::models::{ReferenceInput, ReferenceOutput};
use once_cell::sync::Lazy;
use regex::Regex;

static REF_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"\[\[([a-zA-Z0-9_-]+)\]\]"#).unwrap()
});

/// Parser for [[id]] style references in markdown.
pub struct ReferenceParser;

impl ReferenceParser {
    /// Extract all unique [[id]] references from content.
    /// If `current_entity_id` is provided, filters out self-references.
    pub fn extract_ids(content: &str, current_entity_id: Option<&str>) -> Vec<String> {
        let re = &*REF_RE;

        let mut ids: Vec<String> = re
            .captures_iter(content)
            .map(|cap| cap.get(1).unwrap().as_str().to_string())
            .collect();

        // Deduplicate
        ids.sort();
        ids.dedup();

        // Filter self-reference if specified
        if let Some(self_id) = current_entity_id {
            ids.retain(|id| id != self_id);
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
        let content = "这是 [[entity-001]] 和 [[entity-002]] 的核心观点";
        let refs = ReferenceParser::extract_ids(content, None);
        assert_eq!(refs, vec!["entity-001", "entity-002"]);
    }

    #[test]
    fn test_self_reference_filtered() {
        let content = "关于 [[entity-001]] 的讨论";
        let refs = ReferenceParser::extract_ids(content, Some("entity-001"));
        assert!(!refs.contains(&"entity-001".to_string()));
    }

    #[test]
    fn test_duplicate_refs_deduplicated() {
        let content = "[[id1]] 和 [[id1]] 和 [[id2]]";
        let refs = ReferenceParser::extract_ids(content, None);
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
}
