//! Pure Compass concepts and rules.
//!
//! This layer intentionally has no HTTP, filesystem, SQLite, or Tokio dependency.

#[allow(dead_code)]
pub(crate) mod contracts;
pub(crate) mod entity;
pub(crate) mod scoring;
pub(crate) mod vault;
