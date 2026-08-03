//! # helix-neurosleep — ADR-051 verified NeuroSleep research rail
//!
//! Accepts only authoritative signed rUv Neural V1 bundles, independently
//! verifies origin and canonical payload integrity, applies local trust and
//! research-consent policy, and atomically seals typed derived records in an
//! opaque-keyed study partition.
//!
//! This crate intentionally has no dependency on Helix's generic pipeline,
//! Focus Areas, score, escalation, LLM, neural-session adapter, or retrieval.

mod contract;
mod ingest;
mod longitudinal;
mod storage;

pub use contract::*;
pub use ingest::verify_and_store;
pub use longitudinal::*;
pub use storage::SealedStudyPartition;

#[cfg(test)]
mod policy_tests;
#[cfg(test)]
mod tests;
