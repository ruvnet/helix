//! # helix-provenance — ADR-005: Retrieval-Grounded, Provenance-Required Answering
//!
//! The load-bearing anti-hallucination primitive. Every factual claim Helix
//! surfaces MUST resolve to a stored [`ProvRecord`] carrying source, timestamp,
//! units, reference range and confidence. A claim with no backing record is not
//! a claim — it is suppressed at construction time.
//!
//! This crate encodes that rule as a *type-level* invariant: you cannot build a
//! [`GroundedClaim`] without handing it the [`ProvRecord`]s that back it, and
//! [`ground`] rejects any draft claim whose cited record ids are not present in
//! the retrieved evidence set.
//!
//! Pure, `no_std`-friendly logic (no I/O, no clock) so it is deterministic and
//! trivially testable — see ADR-007's numeric engine for the companion
//! deterministic-computation crate.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Milliseconds since the Unix epoch. Helix never reads the wall clock inside
/// pure logic — timestamps are always passed in by the caller, which keeps every
/// computation reproducible (ADR-007 principle, applied here).
pub type EpochMillis = i64;

/// How a datum entered the vault. Used by the numeric/trend engine to decide
/// whether two values are directly comparable (e.g. you do not trend a manual
/// entry against a lab-feed value without flagging the method change).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeasurementMethod {
    /// Structured feed from a clinical/lab system (FHIR, HL7, vendor API).
    LabFeed,
    /// Parsed out of a PDF/scan via OCR (ADR-012 degradation tier).
    OcrExtraction,
    /// Wearable / device telemetry.
    Device,
    /// Contactless ambient sensing (Cognitum Seed, ADR-014) — screening grade.
    AmbientSensing,
    /// Entered by the user by hand.
    ManualEntry,
    /// Derived by Helix's deterministic engine from other records (ADR-007).
    Derived,
}

/// A closed reference interval `[low, high]` in the record's units. Either bound
/// may be open (`None`) for one-sided ranges (e.g. "troponin < 0.04").
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ReferenceRange {
    pub low: Option<f64>,
    pub high: Option<f64>,
}

impl ReferenceRange {
    pub const fn new(low: Option<f64>, high: Option<f64>) -> Self {
        Self { low, high }
    }

    /// Where a value sits relative to the range.
    pub fn classify(&self, value: f64) -> RangePosition {
        if let Some(low) = self.low {
            if value < low {
                return RangePosition::Below;
            }
        }
        if let Some(high) = self.high {
            if value > high {
                return RangePosition::Above;
            }
        }
        RangePosition::Within
    }
}

/// Result of comparing a value against its [`ReferenceRange`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RangePosition {
    Below,
    Within,
    Above,
}

/// Calibrated confidence in a single datum, `0.0..=1.0`. Distinct from evidence
/// tier (ADR-006): tier is *what kind* of support; confidence is *how solid*
/// this particular measurement is (e.g. an OCR extraction with a low parser
/// score carries lower confidence than a structured lab feed).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Confidence(f64);

impl Confidence {
    /// Clamps into `0.0..=1.0`; never panics.
    pub fn new(v: f64) -> Self {
        Self(v.clamp(0.0, 1.0))
    }
    pub fn get(self) -> f64 {
        self.0
    }
    pub const FULL: Confidence = Confidence(1.0);
}

/// Stable identifier for a stored datum. In the real vault this is the
/// content-addressed key; here it is an opaque string so the crate stays
/// storage-agnostic.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RecordId(pub String);

impl<S: Into<String>> From<S> for RecordId {
    fn from(s: S) -> Self {
        RecordId(s.into())
    }
}

/// A single provenance-tagged datum. This is the schema ADR-005 mandates be
/// attached to **every** value at ingestion and held immutable thereafter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProvRecord {
    pub id: RecordId,
    /// Originating system, e.g. "Quest", "Oura", "Apple Health", "Seed".
    pub source: String,
    /// When the measurement was *taken* (not when ingested).
    pub measured_at: EpochMillis,
    pub method: MeasurementMethod,
    /// The canonical code for the concept (LOINC/RxNorm/SNOMED, ADR-004).
    /// `None` only for not-yet-normalized data still in the review queue.
    pub code: Option<String>,
    /// Human-readable concept label, e.g. "Ferritin".
    pub concept: String,
    pub value: f64,
    /// UCUM unit string, e.g. "ng/mL".
    pub unit: String,
    pub reference_range: Option<ReferenceRange>,
    pub confidence: Confidence,
}

impl ProvRecord {
    /// Position of this record's value against its own reference range, if any.
    pub fn range_position(&self) -> Option<RangePosition> {
        self.reference_range.map(|r| r.classify(self.value))
    }
}

/// A claim drafted by the analyst (ADR-002 FM-Analyst) that asserts something
/// and *names* the records it believes back it. It is "ungrounded" until
/// [`ground`] has verified those records exist in the retrieved evidence set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DraftClaim {
    /// Natural-language assertion, e.g. "your ferritin is low and trending down".
    pub text: String,
    /// Record ids the analyst cites as support.
    pub cites: Vec<RecordId>,
}

impl DraftClaim {
    pub fn new(text: impl Into<String>, cites: impl IntoIterator<Item = RecordId>) -> Self {
        Self {
            text: text.into(),
            cites: cites.into_iter().collect(),
        }
    }
}

/// A claim that has passed grounding: every cited id resolved to a real
/// [`ProvRecord`], and those records travel *with* the claim so the UI can
/// render inline citations and the Verifier (ADR-008) can re-derive it.
///
/// There is no public constructor — the only way to obtain one is [`ground`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroundedClaim {
    text: String,
    evidence: Vec<ProvRecord>,
}

impl GroundedClaim {
    pub fn text(&self) -> &str {
        &self.text
    }
    /// The records that back this claim — never empty.
    pub fn evidence(&self) -> &[ProvRecord] {
        &self.evidence
    }
    /// Lowest confidence across the backing evidence (the claim is only as
    /// strong as its weakest support).
    pub fn min_confidence(&self) -> Confidence {
        self.evidence
            .iter()
            .map(|r| r.confidence.get())
            .fold(1.0, f64::min)
            .into()
    }
}

impl From<f64> for Confidence {
    fn from(v: f64) -> Self {
        Confidence::new(v)
    }
}

/// Why a draft claim failed grounding. Each variant is a reason the claim must
/// be suppressed rather than shown — the "no backing datum → no claim" rule.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GroundingError {
    /// The analyst asserted something but cited no records at all.
    #[error("claim cites no records; ungrounded assertions are suppressed")]
    NoCitations,
    /// A cited id was not present in the retrieved evidence set (a fabricated
    /// or stale reference — exactly what must never reach the user).
    #[error("cited record(s) not found in evidence set: {0:?}")]
    DanglingCitations(Vec<RecordId>),
}

/// The grounding gate. Returns a [`GroundedClaim`] only if **every** cited id
/// resolves to a record in `evidence`; otherwise the claim is rejected.
///
/// `evidence` is the set of records the retrieval layer actually pulled for this
/// turn (the only legitimate basis for an answer). This is the architectural
/// enforcement of ADR-005 §1–2: provenance-required, no-data-no-claim.
pub fn ground(
    draft: &DraftClaim,
    evidence: &[ProvRecord],
) -> Result<GroundedClaim, GroundingError> {
    if draft.cites.is_empty() {
        return Err(GroundingError::NoCitations);
    }
    let index: BTreeMap<&RecordId, &ProvRecord> = evidence.iter().map(|r| (&r.id, r)).collect();

    let mut resolved = Vec::with_capacity(draft.cites.len());
    let mut dangling = Vec::new();
    for id in &draft.cites {
        match index.get(id) {
            Some(rec) => resolved.push((*rec).clone()),
            None => dangling.push(id.clone()),
        }
    }
    if !dangling.is_empty() {
        return Err(GroundingError::DanglingCitations(dangling));
    }
    Ok(GroundedClaim {
        text: draft.text.clone(),
        evidence: resolved,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ferritin() -> ProvRecord {
        ProvRecord {
            id: "rec-ferritin-2026-06".into(),
            source: "Quest".into(),
            measured_at: 1_750_000_000_000,
            method: MeasurementMethod::LabFeed,
            code: Some("2276-4".into()), // LOINC ferritin
            concept: "Ferritin".into(),
            value: 28.0,
            unit: "ng/mL".into(),
            reference_range: Some(ReferenceRange::new(Some(30.0), Some(400.0))),
            confidence: Confidence::FULL,
        }
    }

    #[test]
    fn grounds_a_claim_backed_by_real_records() {
        let ev = vec![ferritin()];
        let draft = DraftClaim::new(
            "Your ferritin is below range.",
            [RecordId::from("rec-ferritin-2026-06")],
        );
        let g = ground(&draft, &ev).expect("should ground");
        assert_eq!(g.text(), "Your ferritin is below range.");
        assert_eq!(g.evidence().len(), 1);
        assert_eq!(g.min_confidence().get(), 1.0);
    }

    #[test]
    fn rejects_claim_with_no_citations() {
        let ev = vec![ferritin()];
        let draft = DraftClaim::new("You are probably low on iron.", []);
        assert_eq!(ground(&draft, &ev), Err(GroundingError::NoCitations));
    }

    #[test]
    fn rejects_fabricated_citation() {
        let ev = vec![ferritin()];
        let draft = DraftClaim::new(
            "Your testosterone is low.",
            [RecordId::from("rec-testosterone-made-up")],
        );
        match ground(&draft, &ev) {
            Err(GroundingError::DanglingCitations(ids)) => {
                assert_eq!(ids, vec![RecordId::from("rec-testosterone-made-up")]);
            }
            other => panic!("expected dangling-citation rejection, got {other:?}"),
        }
    }

    #[test]
    fn reference_range_classifies_low_value() {
        assert_eq!(ferritin().range_position(), Some(RangePosition::Below));
    }

    #[test]
    fn confidence_is_clamped() {
        assert_eq!(Confidence::new(1.7).get(), 1.0);
        assert_eq!(Confidence::new(-0.3).get(), 0.0);
    }

    #[test]
    fn grounded_claim_serializes_with_evidence() {
        let ev = vec![ferritin()];
        let draft = DraftClaim::new("Ferritin low.", [RecordId::from("rec-ferritin-2026-06")]);
        let g = ground(&draft, &ev).unwrap();
        let json = serde_json::to_string(&g).unwrap();
        assert!(json.contains("Ferritin"));
        assert!(json.contains("2276-4"));
    }
}
