//! Pure values exchanged with the Vault port.

use std::fmt;

use serde::{Deserialize, Serialize};

use super::entity::Score;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VaultNote {
    pub(crate) frontmatter: String,
    pub(crate) body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MetadataPatch {
    AddTag(String),
    AddLink(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct MetadataPatchResult {
    pub(crate) changed: bool,
    pub(crate) content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MetadataPatchError {
    Stale { expected: String, actual: String },
}

impl fmt::Display for MetadataPatchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stale { expected, actual } => write!(
                formatter,
                "content hash mismatch: expected {expected}, actual {actual}"
            ),
        }
    }
}

impl std::error::Error for MetadataPatchError {}

pub(crate) type VaultScore = Score;
