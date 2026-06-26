//! # helix-retrieval — ADR-023: semantic retrieval over the health graph
//!
//! The analyst may only compose from what the **Retrieve** step pulled (ADR-005).
//! Exact concept-code matching misses the connective reasoning the product is for:
//! "why am I tired?" should surface ferritin **and** deep-sleep **and** vitamin D,
//! which share no code — only a clinical relationship.
//!
//! This crate is the **retrieval contract** Helix builds on RuVector's HNSW /
//! GraphRAG. The hard line it draws: **retrieval is recall, not grounding.** It
//! decides what the analyst is *allowed to look at*; every returned record still
//! passes the ADR-005 grounding gate before it can back a claim. So retrieval can
//! be permissive — assertion stays strict.
//!
//! Each result carries its similarity score **and the reason it was retrieved**
//! (direct match / vector neighbour / graph-linked), so the Retrieve step is
//! auditable and the Verifier (ADR-008) can see why a record is in scope.
//!
//! The vector index + embedder are injected (`Embedder` + `Index` traits); this
//! crate owns the ranking/explanation policy and is pure + fully testable.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use helix_provenance::{EpochMillis, ProvRecord};

/// Why a record was pulled into the candidate set — surfaced for audit (ADR-005)
/// and for the Verifier (ADR-008).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalReason {
    /// Exact concept-code match with the query.
    DirectMatch,
    /// Vector-space nearest neighbour of the query.
    VectorNeighbour,
    /// Reached by traversing a graph edge from a matched record.
    GraphLinked,
}

/// A retrieved candidate: the record, its fused score, and why it was retrieved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Retrieved {
    pub record: ProvRecord,
    /// Fused score in `[0,1]` (similarity blended with a recency bonus).
    pub score: f64,
    pub reason: RetrievalReason,
}

/// Computes a query/record embedding. RuVector's on-device encoder implements
/// this; tests use a deterministic stub.
pub trait Embedder {
    fn embed_query(&self, query: &str) -> Vec<f32>;
    fn embed_record(&self, record: &ProvRecord) -> Vec<f32>;
}

/// A vector index over the dossier. RuVector HNSW implements this.
pub trait Index {
    /// Cosine-or-similar similarity neighbours of `query_vec`, as (record, sim in
    /// `[0,1]`), already capped to a reasonable candidate pool by the engine.
    fn neighbours(&self, query_vec: &[f32]) -> Vec<(ProvRecord, f32)>;
    /// Records reachable by one graph hop from any of `seed` (clinical edges).
    fn graph_links(&self, seed: &[ProvRecord]) -> Vec<ProvRecord>;
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum RetrievalError {
    #[error("top_k must be >= 1")]
    BadTopK,
    #[error("recency_weight must be in 0.0..=1.0, got {0}")]
    BadRecencyWeight(f64),
    #[error("a similarity score was not finite or out of [0,1]")]
    BadSimilarity,
}

/// Retrieval query parameters.
#[derive(Debug, Clone)]
pub struct Query<'a> {
    pub text: &'a str,
    /// The concept code(s) the user is directly asking about (direct matches).
    pub concept_codes: &'a [String],
    /// "Now" for recency scoring (clock injected, ADR-007 discipline).
    pub now: EpochMillis,
    pub top_k: usize,
    /// 0 = pure similarity; 1 = recency dominates. Blends the two.
    pub recency_weight: f64,
}

const HALF_LIFE_DAYS: f64 = 180.0;
const MS_PER_DAY: f64 = 86_400_000.0;

/// Recency bonus in `[0,1]`: 1.0 for "now", decaying with a 180-day half-life.
fn recency_bonus(now: EpochMillis, measured_at: EpochMillis) -> f64 {
    let age_days = ((now - measured_at).max(0) as f64) / MS_PER_DAY;
    0.5f64.powf(age_days / HALF_LIFE_DAYS)
}

/// Run retrieval: direct matches + vector neighbours + one graph hop, fused,
/// deduplicated by record id, recency-blended, and truncated to `top_k`.
///
/// Returns a ranked candidate set. **These are candidates only** — each must
/// still pass the ADR-005 grounding gate before backing a claim.
pub fn retrieve(
    query: &Query<'_>,
    all_records: &[ProvRecord],
    embedder: &dyn Embedder,
    index: &dyn Index,
) -> Result<Vec<Retrieved>, RetrievalError> {
    if query.top_k == 0 {
        return Err(RetrievalError::BadTopK);
    }
    if !(0.0..=1.0).contains(&query.recency_weight) {
        return Err(RetrievalError::BadRecencyWeight(query.recency_weight));
    }

    let mut by_id: std::collections::BTreeMap<String, Retrieved> = Default::default();

    let consider = |by_id: &mut std::collections::BTreeMap<String, Retrieved>,
                    record: ProvRecord,
                    sim: f64,
                    reason: RetrievalReason|
     -> Result<(), RetrievalError> {
        if !(0.0..=1.0).contains(&sim) {
            return Err(RetrievalError::BadSimilarity);
        }
        let rec_bonus = recency_bonus(query.now, record.measured_at);
        let score = (1.0 - query.recency_weight) * sim + query.recency_weight * rec_bonus;
        let id = record.id.0.clone();
        // Keep the strongest reason/score per record; DirectMatch wins ties.
        by_id
            .entry(id)
            .and_modify(|cur| {
                if reason == RetrievalReason::DirectMatch || score > cur.score {
                    cur.score = cur.score.max(score);
                    if reason == RetrievalReason::DirectMatch {
                        cur.reason = RetrievalReason::DirectMatch;
                    }
                }
            })
            .or_insert(Retrieved {
                record,
                score,
                reason,
            });
        Ok(())
    };

    // 1. Direct concept-code matches (similarity 1.0).
    for r in all_records {
        if let Some(code) = &r.code {
            if query.concept_codes.iter().any(|c| c == code) {
                consider(&mut by_id, r.clone(), 1.0, RetrievalReason::DirectMatch)?;
            }
        }
    }

    // 2. Vector neighbours.
    let qv = embedder.embed_query(query.text);
    for (rec, sim) in index.neighbours(&qv) {
        consider(
            &mut by_id,
            rec,
            sim as f64,
            RetrievalReason::VectorNeighbour,
        )?;
    }

    // 3. One graph hop from the direct matches.
    let seeds: Vec<ProvRecord> = by_id
        .values()
        .filter(|r| r.reason == RetrievalReason::DirectMatch)
        .map(|r| r.record.clone())
        .collect();
    for rec in index.graph_links(&seeds) {
        // graph links get a fixed moderate similarity; the edge is the evidence.
        consider(&mut by_id, rec, 0.6, RetrievalReason::GraphLinked)?;
    }

    let mut out: Vec<Retrieved> = by_id.into_values().collect();
    out.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
    out.truncate(query.top_k);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use helix_provenance::{Confidence, MeasurementMethod, RecordId, ReferenceRange};

    const DAY: i64 = 86_400_000;

    fn rec(id: &str, code: &str, concept: &str, days_ago: i64) -> ProvRecord {
        ProvRecord {
            id: RecordId::from(id),
            source: "Quest".into(),
            measured_at: 1000 * DAY - days_ago * DAY,
            method: MeasurementMethod::LabFeed,
            code: Some(code.into()),
            concept: concept.into(),
            value: 1.0,
            unit: "x".into(),
            reference_range: Some(ReferenceRange::new(Some(0.0), Some(10.0))),
            confidence: Confidence::FULL,
        }
    }

    struct StubEmbed;
    impl Embedder for StubEmbed {
        fn embed_query(&self, _: &str) -> Vec<f32> {
            vec![1.0, 0.0]
        }
        fn embed_record(&self, _: &ProvRecord) -> Vec<f32> {
            vec![1.0, 0.0]
        }
    }

    struct StubIndex {
        neighbours: Vec<(ProvRecord, f32)>,
        links: Vec<ProvRecord>,
    }
    impl Index for StubIndex {
        fn neighbours(&self, _: &[f32]) -> Vec<(ProvRecord, f32)> {
            self.neighbours.clone()
        }
        fn graph_links(&self, _: &[ProvRecord]) -> Vec<ProvRecord> {
            self.links.clone()
        }
    }

    fn query<'a>(codes: &'a [String]) -> Query<'a> {
        Query {
            text: "why am I tired?",
            concept_codes: codes,
            now: 1000 * DAY,
            top_k: 10,
            recency_weight: 0.2,
        }
    }

    #[test]
    fn fuses_direct_vector_and_graph_with_reasons() {
        let ferritin = rec("f", "2276-4", "Ferritin", 5);
        let sleep = rec("s", "93832-4", "Deep sleep", 1);
        let vitd = rec("d", "1989-3", "Vitamin D", 30);
        let all = vec![ferritin.clone()];
        let idx = StubIndex {
            neighbours: vec![(sleep.clone(), 0.82)],
            links: vec![vitd.clone()],
        };
        let codes = vec!["2276-4".to_string()];
        let out = retrieve(&query(&codes), &all, &StubEmbed, &idx).unwrap();

        // all three concepts surfaced despite sharing no code
        assert_eq!(out.len(), 3);
        let by_concept: std::collections::BTreeMap<_, _> = out
            .iter()
            .map(|r| (r.record.concept.as_str(), r.reason))
            .collect();
        assert_eq!(by_concept["Ferritin"], RetrievalReason::DirectMatch);
        assert_eq!(by_concept["Deep sleep"], RetrievalReason::VectorNeighbour);
        assert_eq!(by_concept["Vitamin D"], RetrievalReason::GraphLinked);
        // direct match ranks first (similarity 1.0)
        assert_eq!(out[0].record.concept, "Ferritin");
    }

    #[test]
    fn dedupes_and_keeps_direct_reason() {
        let ferritin = rec("f", "2276-4", "Ferritin", 5);
        let all = vec![ferritin.clone()];
        // same record also returned as a vector neighbour
        let idx = StubIndex {
            neighbours: vec![(ferritin.clone(), 0.9)],
            links: vec![],
        };
        let codes = vec!["2276-4".to_string()];
        let out = retrieve(&query(&codes), &all, &StubEmbed, &idx).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].reason, RetrievalReason::DirectMatch);
    }

    #[test]
    fn respects_top_k() {
        let all: Vec<ProvRecord> = (0..5)
            .map(|i| rec(&format!("r{i}"), &format!("c{i}"), "X", i))
            .collect();
        let idx = StubIndex {
            neighbours: all.iter().map(|r| (r.clone(), 0.5)).collect(),
            links: vec![],
        };
        let codes: Vec<String> = vec![];
        let mut q = query(&codes);
        q.top_k = 2;
        let out = retrieve(&q, &all, &StubEmbed, &idx).unwrap();
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn recency_breaks_similarity_ties() {
        let fresh = rec("fresh", "a", "A", 0);
        let stale = rec("stale", "b", "B", 720);
        let idx = StubIndex {
            neighbours: vec![(fresh.clone(), 0.8), (stale.clone(), 0.8)],
            links: vec![],
        };
        let codes: Vec<String> = vec![];
        let out = retrieve(&query(&codes), &[], &StubEmbed, &idx).unwrap();
        assert_eq!(out[0].record.id.0, "fresh"); // equal sim, fresher wins
    }

    #[test]
    fn rejects_bad_params_and_similarity() {
        let codes: Vec<String> = vec![];
        let mut q = query(&codes);
        q.top_k = 0;
        assert_eq!(
            retrieve(
                &q,
                &[],
                &StubEmbed,
                &StubIndex {
                    neighbours: vec![],
                    links: vec![]
                }
            ),
            Err(RetrievalError::BadTopK)
        );
        let idx = StubIndex {
            neighbours: vec![(rec("x", "c", "X", 0), 1.5)],
            links: vec![],
        };
        assert_eq!(
            retrieve(&query(&codes), &[], &StubEmbed, &idx),
            Err(RetrievalError::BadSimilarity)
        );
    }
}
