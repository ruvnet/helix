//! # helix-ontology — ADR-004: Canonical Ontology Normalization
//!
//! The same concept arrives in a dozen formats and units across sources.
//! Reasoning over un-normalized text invites error, so Helix normalizes
//! **everything** to canonical code systems before it enters the analytic graph:
//!
//! - **LOINC** — labs/observations
//! - **RxNorm** — medications
//! - **SNOMED CT** — conditions/clinical findings
//! - **ICD-10-CM** — diagnoses/billing
//! - **UCUM** — units of measure
//!
//! with **FHIR** as the interchange model. The non-negotiable rule (ADR-004):
//! data that cannot be confidently mapped is **not silently coerced** — it goes
//! to a human-review queue. This crate models the code systems, the
//! normalization outcome, and that review-queue policy. It is deliberately
//! dependency-light: the actual code tables are loaded by the caller; here we
//! enforce *shape* and *policy*.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The canonical code systems Helix normalizes to. Each carries its FHIR system
/// URI so a normalized concept round-trips into a FHIR `Coding`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeSystem {
    Loinc,
    RxNorm,
    SnomedCt,
    Icd10Cm,
    Ucum,
}

impl CodeSystem {
    /// The canonical FHIR `system` URI for this code system.
    pub const fn fhir_uri(self) -> &'static str {
        match self {
            CodeSystem::Loinc => "http://loinc.org",
            CodeSystem::RxNorm => "http://www.nlm.nih.gov/research/umls/rxnorm",
            CodeSystem::SnomedCt => "http://snomed.info/sct",
            CodeSystem::Icd10Cm => "http://hl7.org/fhir/sid/icd-10-cm",
            CodeSystem::Ucum => "http://unitsofmeasure.org",
        }
    }

    /// Which clinical domain this system is the canonical choice for.
    pub const fn domain(self) -> Domain {
        match self {
            CodeSystem::Loinc => Domain::Observation,
            CodeSystem::RxNorm => Domain::Medication,
            CodeSystem::SnomedCt => Domain::Condition,
            CodeSystem::Icd10Cm => Domain::Condition,
            CodeSystem::Ucum => Domain::Unit,
        }
    }
}

/// Clinical domain of an incoming datum, used to pick the target code system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Domain {
    Observation,
    Medication,
    Condition,
    Unit,
}

impl Domain {
    /// The canonical code system Helix maps this domain to (ADR-004).
    pub const fn canonical_system(self) -> CodeSystem {
        match self {
            Domain::Observation => CodeSystem::Loinc,
            Domain::Medication => CodeSystem::RxNorm,
            Domain::Condition => CodeSystem::SnomedCt,
            Domain::Unit => CodeSystem::Ucum,
        }
    }
}

/// A successfully normalized concept: a canonical code plus a FHIR-ready coding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalCode {
    pub system: CodeSystem,
    /// The code value, e.g. "2276-4" (LOINC ferritin).
    pub code: String,
    /// Human-readable display, e.g. "Ferritin [Mass/volume] in Serum or Plasma".
    pub display: String,
}

impl CanonicalCode {
    pub fn new(system: CodeSystem, code: impl Into<String>, display: impl Into<String>) -> Self {
        Self {
            system,
            code: code.into(),
            display: display.into(),
        }
    }

    /// Render as a FHIR `Coding` JSON object (interchange model).
    pub fn to_fhir_coding(&self) -> serde_json::Value {
        serde_json::json!({
            "system": self.system.fhir_uri(),
            "code": self.code,
            "display": self.display,
        })
    }
}

/// A raw, un-normalized term as it arrived from a source, with whatever hint the
/// connector could attach.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawTerm {
    pub text: String,
    pub domain: Domain,
    /// Source-native unit string if any (to be UCUM-normalized).
    pub unit: Option<String>,
}

/// The outcome of normalizing one [`RawTerm`]. A confident match is normalized;
/// anything uncertain is routed to review — never silently coerced (ADR-004).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum NormalizationOutcome {
    /// Mapped to a canonical code at or above the confidence floor.
    Normalized {
        canonical: CanonicalCode,
        confidence: f64,
    },
    /// Could not be mapped confidently — queued for human review.
    Queued(ReviewItem),
}

/// An item parked in the human-review queue, with enough context for a curator
/// to resolve it. Nothing here ever reaches the analytic graph until resolved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReviewItem {
    pub raw: RawTerm,
    pub reason: ReviewReason,
    /// Best-guess candidates the matcher produced, ranked, for the curator.
    pub candidates: Vec<ScoredCandidate>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScoredCandidate {
    pub canonical: CanonicalCode,
    pub score: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewReason {
    /// No candidate cleared the confidence floor.
    LowConfidence,
    /// Multiple candidates were close — ambiguous.
    Ambiguous,
    /// The matcher produced nothing at all.
    NoCandidate,
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum OntologyError {
    #[error("confidence floor must be in 0.0..=1.0, got {0}")]
    BadFloor(f64),
    #[error("a candidate score was not finite")]
    NonFinite,
}

/// Normalize a raw term given a ranked candidate list (produced upstream by the
/// matcher: exact code lookup → fuzzy NLP). Policy, not matching, lives here:
///
/// - top candidate ≥ `floor` **and** clearly ahead of the runner-up → Normalized
/// - top candidate ≥ `floor` but within `ambiguity_margin` of #2 → Queued(Ambiguous)
/// - nothing ≥ `floor` → Queued(LowConfidence) / Queued(NoCandidate)
///
/// This is the ADR-004 gate: confident or queued, never silently coerced.
pub fn normalize(
    raw: &RawTerm,
    mut candidates: Vec<ScoredCandidate>,
    floor: f64,
    ambiguity_margin: f64,
) -> Result<NormalizationOutcome, OntologyError> {
    if !(0.0..=1.0).contains(&floor) {
        return Err(OntologyError::BadFloor(floor));
    }
    if candidates.iter().any(|c| !c.score.is_finite()) {
        return Err(OntologyError::NonFinite);
    }
    if candidates.is_empty() {
        return Ok(NormalizationOutcome::Queued(ReviewItem {
            raw: raw.clone(),
            reason: ReviewReason::NoCandidate,
            candidates,
        }));
    }
    // Rank descending by score.
    candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
    // Copy the scores we need so we can move `candidates` later without a live borrow.
    let top_score = candidates[0].score;
    let second_score = candidates.get(1).map(|c| c.score);

    if top_score < floor {
        return Ok(NormalizationOutcome::Queued(ReviewItem {
            raw: raw.clone(),
            reason: ReviewReason::LowConfidence,
            candidates,
        }));
    }
    // Ambiguity check against the runner-up.
    if let Some(second) = second_score {
        if top_score - second < ambiguity_margin {
            return Ok(NormalizationOutcome::Queued(ReviewItem {
                raw: raw.clone(),
                reason: ReviewReason::Ambiguous,
                candidates,
            }));
        }
    }
    Ok(NormalizationOutcome::Normalized {
        canonical: candidates.into_iter().next().unwrap().canonical,
        confidence: top_score,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(system: CodeSystem, code: &str, display: &str, score: f64) -> ScoredCandidate {
        ScoredCandidate {
            canonical: CanonicalCode::new(system, code, display),
            score,
        }
    }

    fn ferritin_raw() -> RawTerm {
        RawTerm {
            text: "Ferritin, Serum".into(),
            domain: Domain::Observation,
            unit: Some("ng/mL".into()),
        }
    }

    #[test]
    fn confident_unambiguous_match_is_normalized() {
        let out = normalize(
            &ferritin_raw(),
            vec![
                cand(CodeSystem::Loinc, "2276-4", "Ferritin", 0.97),
                cand(CodeSystem::Loinc, "20567-4", "Ferritin other", 0.40),
            ],
            0.85,
            0.10,
        )
        .unwrap();
        match out {
            NormalizationOutcome::Normalized {
                canonical,
                confidence,
            } => {
                assert_eq!(canonical.code, "2276-4");
                assert_eq!(canonical.system, CodeSystem::Loinc);
                assert!((confidence - 0.97).abs() < 1e-9);
            }
            other => panic!("expected normalized, got {other:?}"),
        }
    }

    #[test]
    fn ambiguous_near_tie_is_queued() {
        let out = normalize(
            &ferritin_raw(),
            vec![
                cand(CodeSystem::Loinc, "2276-4", "Ferritin", 0.90),
                cand(CodeSystem::Loinc, "20567-4", "Ferritin other", 0.88),
            ],
            0.85,
            0.10,
        )
        .unwrap();
        match out {
            NormalizationOutcome::Queued(item) => assert_eq!(item.reason, ReviewReason::Ambiguous),
            other => panic!("expected queued/ambiguous, got {other:?}"),
        }
    }

    #[test]
    fn below_floor_is_queued_not_coerced() {
        let out = normalize(
            &ferritin_raw(),
            vec![cand(CodeSystem::Loinc, "2276-4", "Ferritin", 0.55)],
            0.85,
            0.10,
        )
        .unwrap();
        match out {
            NormalizationOutcome::Queued(item) => {
                assert_eq!(item.reason, ReviewReason::LowConfidence)
            }
            other => panic!("expected queued/low-confidence, got {other:?}"),
        }
    }

    #[test]
    fn empty_candidates_queued_no_candidate() {
        let out = normalize(&ferritin_raw(), vec![], 0.85, 0.10).unwrap();
        match out {
            NormalizationOutcome::Queued(item) => {
                assert_eq!(item.reason, ReviewReason::NoCandidate)
            }
            other => panic!("expected queued/no-candidate, got {other:?}"),
        }
    }

    #[test]
    fn bad_floor_and_non_finite_rejected() {
        assert_eq!(
            normalize(&ferritin_raw(), vec![], 1.5, 0.1),
            Err(OntologyError::BadFloor(1.5))
        );
        assert_eq!(
            normalize(
                &ferritin_raw(),
                vec![cand(CodeSystem::Loinc, "x", "y", f64::NAN)],
                0.85,
                0.1
            ),
            Err(OntologyError::NonFinite)
        );
    }

    #[test]
    fn canonical_system_per_domain() {
        assert_eq!(Domain::Observation.canonical_system(), CodeSystem::Loinc);
        assert_eq!(Domain::Medication.canonical_system(), CodeSystem::RxNorm);
        assert_eq!(Domain::Condition.canonical_system(), CodeSystem::SnomedCt);
        assert_eq!(Domain::Unit.canonical_system(), CodeSystem::Ucum);
    }

    #[test]
    fn fhir_coding_round_trip() {
        let c = CanonicalCode::new(CodeSystem::Loinc, "2276-4", "Ferritin");
        let v = c.to_fhir_coding();
        assert_eq!(v["system"], "http://loinc.org");
        assert_eq!(v["code"], "2276-4");
    }
}
