//! # helix-neurosleep — ADR-051 verified NeuroSleep research rail
//!
//! Accepts only authoritative signed rUv Neural V1 bundles, independently
//! verifies origin and canonical payload integrity, applies local trust and
//! research-consent policy, and atomically seals typed derived records in an
//! opaque-keyed study partition.
//!
//! This crate intentionally has no dependency on Helix's generic pipeline,
//! Focus Areas, score, escalation, LLM, neural-session adapter, or retrieval.

mod config;
mod longitudinal;
mod view;

pub use config::NeuroSleepResearchFlags;
pub use longitudinal::*;
pub use view::*;

#[cfg(feature = "native-ingest")]
mod contract;
#[cfg(feature = "native-ingest")]
mod ingest;
#[cfg(feature = "native-ingest")]
mod storage;

#[cfg(feature = "native-ingest")]
pub use contract::*;
#[cfg(feature = "native-ingest")]
pub use ingest::verify_and_store;
#[cfg(feature = "native-ingest")]
pub use storage::SealedStudyPartition;

#[cfg(all(test, feature = "native-ingest"))]
mod policy_tests;
#[cfg(all(test, feature = "native-ingest"))]
mod tests;
