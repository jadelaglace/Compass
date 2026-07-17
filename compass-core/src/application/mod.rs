//! Use-case boundary.
//!
//! Concrete query, entity, suggestion, and index services are extracted in P5.3-P5.5.
//! Keeping this module free of transport and persistence imports makes that migration
//! incremental without changing the existing public contracts.

pub(crate) mod entity_service;
pub(crate) mod error;
pub(crate) mod index_service;
pub(crate) mod ports;
pub(crate) mod query_service;
pub(crate) mod suggestion_service;
