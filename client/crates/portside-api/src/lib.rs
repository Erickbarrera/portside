//! Typed client for the static Portside catalog, used by the desktop app's Rust engine.
//!
//! Contract: `docs/specs/static-catalog.md` (ADR-0015). `tests/contract.rs` checks the models
//! against `docs/specs/static-catalog/v1.json`, generated from the catalog builder's schemas.

mod client;
mod error;
pub mod models;

pub use client::{Client, SearchParams, SnapshotResult};
pub use error::{ApiError, Problem};
