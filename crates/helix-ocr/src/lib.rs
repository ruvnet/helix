//! # helix-ocr — ADR-022: OCR lab-document ingestion
//!
//! For most users the lab **PDF is the primary ingestion path** — Quest and
//! Labcorp have no consumer APIs (ADR-012). RuVector ships on-device OCR, so
//! Helix can extract values from scanned/exported lab documents without sending
//! them to a cloud service (ADR-001/013).
//!
//! OCR is messy, and a misread digit could fabricate or hide a red-flag — so this
//! is the **most conservative** ingestion route. The flow is `extract → gate →
//! record | queue`:
//!
//! - extraction produces *candidate* analytes (label, value, unit, OCR confidence);
//! - a **sanity gate** rejects/queues anything below an OCR-confidence floor,
//!   non-finite, unit-less, or physiologically implausible;
//! - survivors become `ProvRecord`s with `method = OcrExtraction` and a
//!   **capped confidence derived from the OCR confidence** — always lower than a
//!   structured feed, so the analyst and Verifier (ADR-008) weight them accordingly.
//!
//! Borderline candidates go to a human-review queue (ADR-004) — never silently
//! coerced. The OCR engine is injected, so the gate is pure and fully testable.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use helix_provenance::{
    Confidence, EpochMillis, MeasurementMethod, ProvRecord, RecordId, ReferenceRange,
};

const SOURCE: &str = "ocr";

/// A raw candidate analyte as extracted from a lab document by OCR.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcrCandidate {
    /// Raw label text, e.g. "Ferritin".
    pub label: String,
    /// Parsed numeric value (already converted from the OCR string by the engine).
    pub value: f64,
    /// Parsed unit, e.g. "ng/mL" (empty if OCR could not read one).
    pub unit: String,
    pub reference_low: Option<f64>,
    pub reference_high: Option<f64>,
    /// Per-field OCR confidence, 0..=1.
    pub ocr_confidence: f64,
}

/// A coarse plausibility band for an analyte (outer bounds; anything outside is
/// almost certainly an OCR error, not a real reading).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Plausible {
    pub min: f64,
    pub max: f64,
}

/// Why a candidate was sent to review instead of accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueReason {
    LowOcrConfidence,
    NonFinite,
    MissingUnit,
    Implausible,
}

/// The gate outcome for one candidate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Gated {
    /// Accepted into the dossier as a conservative-confidence record.
    Accepted(ProvRecord),
    /// Sent to the human-review queue with the reason and the raw candidate.
    Queued {
        reason: QueueReason,
        candidate: OcrCandidate,
    },
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum OcrError {
    #[error("ocr confidence floor must be in 0.0..=1.0, got {0}")]
    BadFloor(f64),
}

/// Lab document the OCR engine processed, plus when it was imported.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcrDocument {
    pub doc_label: String,
    pub imported_at: EpochMillis,
    pub candidates: Vec<OcrCandidate>,
}

/// Gate one candidate. `plausible` is the optional coarse range for this analyte;
/// `floor` is the minimum OCR confidence to accept. The accepted record's
/// confidence is the OCR confidence **capped at 0.8** (never as trusted as a
/// structured feed).
pub fn gate_candidate(
    c: &OcrCandidate,
    doc_label: &str,
    imported_at: EpochMillis,
    plausible: Option<Plausible>,
    floor: f64,
) -> Result<Gated, OcrError> {
    if !(0.0..=1.0).contains(&floor) {
        return Err(OcrError::BadFloor(floor));
    }
    // Order: non-finite → unit → ocr-confidence → plausibility.
    if !c.value.is_finite() {
        return Ok(queue(c, QueueReason::NonFinite));
    }
    if c.unit.trim().is_empty() {
        return Ok(queue(c, QueueReason::MissingUnit));
    }
    if c.ocr_confidence < floor {
        return Ok(queue(c, QueueReason::LowOcrConfidence));
    }
    if let Some(p) = plausible {
        if c.value < p.min || c.value > p.max {
            return Ok(queue(c, QueueReason::Implausible));
        }
    }
    let conf = c.ocr_confidence.clamp(0.0, 0.8); // capped — OCR is never a clean feed
    Ok(Gated::Accepted(ProvRecord {
        id: RecordId::from(format!("ocr-{}-{}-{}", doc_label, c.label, imported_at)),
        source: SOURCE.to_string(),
        measured_at: imported_at,
        method: MeasurementMethod::OcrExtraction,
        code: None, // normalized later by ADR-004 (label not yet mapped to LOINC)
        concept: c.label.clone(),
        value: c.value,
        unit: c.unit.clone(),
        reference_range: Some(ReferenceRange::new(c.reference_low, c.reference_high)),
        confidence: Confidence::new(conf),
    }))
}

fn queue(c: &OcrCandidate, reason: QueueReason) -> Gated {
    Gated::Queued {
        reason,
        candidate: c.clone(),
    }
}

/// Gate every candidate in a document. `plausible_for` supplies the coarse range
/// per analyte label (return `None` for analytes without a curated band).
pub fn ingest_document<F>(
    doc: &OcrDocument,
    floor: f64,
    plausible_for: F,
) -> Result<Vec<Gated>, OcrError>
where
    F: Fn(&str) -> Option<Plausible>,
{
    doc.candidates
        .iter()
        .map(|c| {
            gate_candidate(
                c,
                &doc.doc_label,
                doc.imported_at,
                plausible_for(&c.label),
                floor,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(label: &str, value: f64, unit: &str, conf: f64) -> OcrCandidate {
        OcrCandidate {
            label: label.into(),
            value,
            unit: unit.into(),
            reference_low: Some(30.0),
            reference_high: Some(400.0),
            ocr_confidence: conf,
        }
    }

    fn ferritin_plausible(_: &str) -> Option<Plausible> {
        Some(Plausible {
            min: 1.0,
            max: 2000.0,
        })
    }

    #[test]
    fn accepts_clean_candidate_with_capped_confidence() {
        let c = cand("Ferritin", 28.0, "ng/mL", 0.97);
        match gate_candidate(&c, "doc", 1000, ferritin_plausible("x"), 0.6).unwrap() {
            Gated::Accepted(r) => {
                assert_eq!(r.method, MeasurementMethod::OcrExtraction);
                assert_eq!(r.value, 28.0);
                assert!(r.confidence.get() <= 0.8); // capped below a clean feed
                assert!(r.code.is_none()); // normalized later (ADR-004)
            }
            other => panic!("expected accepted, got {other:?}"),
        }
    }

    #[test]
    fn queues_low_ocr_confidence() {
        let c = cand("Ferritin", 28.0, "ng/mL", 0.40);
        match gate_candidate(&c, "doc", 1000, None, 0.6).unwrap() {
            Gated::Queued { reason, .. } => assert_eq!(reason, QueueReason::LowOcrConfidence),
            other => panic!("expected queued, got {other:?}"),
        }
    }

    #[test]
    fn queues_missing_unit() {
        let c = cand("Ferritin", 28.0, "", 0.99);
        match gate_candidate(&c, "doc", 1000, None, 0.6).unwrap() {
            Gated::Queued { reason, .. } => assert_eq!(reason, QueueReason::MissingUnit),
            other => panic!("expected queued, got {other:?}"),
        }
    }

    #[test]
    fn queues_implausible_value_misread_digit() {
        // A misread "28" as "2800000" must not become a record.
        let c = cand("Ferritin", 2_800_000.0, "ng/mL", 0.99);
        match gate_candidate(&c, "doc", 1000, ferritin_plausible("x"), 0.6).unwrap() {
            Gated::Queued { reason, .. } => assert_eq!(reason, QueueReason::Implausible),
            other => panic!("expected queued, got {other:?}"),
        }
    }

    #[test]
    fn queues_non_finite() {
        let c = cand("Ferritin", f64::NAN, "ng/mL", 0.99);
        match gate_candidate(&c, "doc", 1000, None, 0.6).unwrap() {
            Gated::Queued { reason, .. } => assert_eq!(reason, QueueReason::NonFinite),
            other => panic!("expected queued, got {other:?}"),
        }
    }

    #[test]
    fn bad_floor_errors() {
        let c = cand("Ferritin", 28.0, "ng/mL", 0.9);
        assert_eq!(
            gate_candidate(&c, "d", 1, None, 1.5),
            Err(OcrError::BadFloor(1.5))
        );
    }

    #[test]
    fn ingest_document_gates_all() {
        let doc = OcrDocument {
            doc_label: "quest-2026-06".into(),
            imported_at: 2000,
            candidates: vec![
                cand("Ferritin", 28.0, "ng/mL", 0.95), // accept
                cand("Glucose", 90.0, "", 0.95),       // queue: missing unit
                cand("HDL", 55.0, "mg/dL", 0.30),      // queue: low conf
            ],
        };
        let out = ingest_document(&doc, 0.6, ferritin_plausible).unwrap();
        let accepted = out
            .iter()
            .filter(|g| matches!(g, Gated::Accepted(_)))
            .count();
        let queued = out
            .iter()
            .filter(|g| matches!(g, Gated::Queued { .. }))
            .count();
        assert_eq!(accepted, 1);
        assert_eq!(queued, 2);
    }
}
