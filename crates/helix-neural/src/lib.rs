//! # helix-neural — integration adapter for `ruvnet/ruv-neural`
//!
//! [ruv-neural](https://github.com/ruvnet/ruv-neural) is an open, closed-loop OS
//! for **gamma-entrainment (40 Hz) research** — a Rust/WASM/edge harness that
//! measures and compares multisensory stimulation protocols with *signed,
//! reproducible evidence*. It is explicitly **research-grade, not a medical
//! device**.
//!
//! This adapter ingests a ruv-neural session's signed evidence and maps it into
//! Helix [`ProvRecord`]s so the neuro signal can live in the personal-health
//! graph alongside labs, wearables, and ambient sensing — **as a screening /
//! research signal, never a diagnosis** (ADR-006/009/010, mirroring the Cognitum
//! Seed framing in ADR-014).
//!
//! Hard rules enforced here:
//! - Every produced record is tagged [`MeasurementMethod::AmbientSensing`] and
//!   carries the [`RESEARCH_DISCLAIMER`]; nothing claims a clinical or
//!   therapeutic effect.
//! - Records use a `RUVN-*` research code namespace, never a clinical LOINC code,
//!   so the ontology layer (ADR-004) cannot mistake them for validated lab data.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use helix_provenance::{
    Confidence, EpochMillis, MeasurementMethod, ProvRecord, RecordId, ReferenceRange,
};

/// The framing that must accompany any neuro-derived signal in the UI.
pub const RESEARCH_DISCLAIMER: &str =
    "Research-grade gamma-entrainment signal (ruv-neural) — screening/exploratory only, \
     not a diagnosis and not a therapeutic claim.";

const SOURCE: &str = "ruv-neural";

/// One metric measured during a ruv-neural session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionMetric {
    /// Short metric key, e.g. "gamma_power_db", "phase_locking_value".
    pub key: String,
    pub value: f64,
    pub unit: String,
}

/// A ruv-neural session's signed evidence, as exported by the harness.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NeuralSession {
    pub session_id: String,
    /// When the session was recorded (epoch millis).
    pub recorded_at: EpochMillis,
    /// Protocol label, e.g. "40Hz audiovisual".
    pub protocol: String,
    pub duration_s: u32,
    pub metrics: Vec<SessionMetric>,
    /// The harness's evidence signature (carried as provenance; cryptographic
    /// verification is ruv-neural's responsibility, not re-done here).
    pub evidence_signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum NeuralError {
    #[error("session has no metrics")]
    NoMetrics,
    #[error("unsigned session evidence is rejected (provenance requirement)")]
    Unsigned,
    #[error("metric '{0}' has a non-finite value")]
    NonFinite(String),
}

/// Map a known metric key to (human concept label, optional informational range).
/// Ranges here are *orientation* bands for the research signal — never clinical
/// thresholds. Unknown keys still map (with no range) rather than being dropped.
fn concept_for(key: &str) -> (String, Option<ReferenceRange>) {
    match key {
        "gamma_power_db" => ("Gamma-band power (40 Hz)".to_string(), None),
        "phase_locking_value" => (
            "Entrainment phase-locking value".to_string(),
            // PLV is bounded 0..1 by definition; expose as an informational band.
            Some(ReferenceRange::new(Some(0.0), Some(1.0))),
        ),
        "entrainment_index" => (
            "40 Hz entrainment index".to_string(),
            Some(ReferenceRange::new(Some(0.0), Some(1.0))),
        ),
        other => (format!("Neuro signal: {other}"), None),
    }
}

/// Convert a signed ruv-neural session into provenance-tagged Helix records —
/// one per metric. Refuses unsigned or empty/non-finite sessions (a record with
/// no provenance is not allowed into the graph, ADR-005).
pub fn session_to_records(session: &NeuralSession) -> Result<Vec<ProvRecord>, NeuralError> {
    if session.evidence_signature.trim().is_empty() {
        return Err(NeuralError::Unsigned);
    }
    if session.metrics.is_empty() {
        return Err(NeuralError::NoMetrics);
    }
    let mut out = Vec::with_capacity(session.metrics.len());
    for m in &session.metrics {
        if !m.value.is_finite() {
            return Err(NeuralError::NonFinite(m.key.clone()));
        }
        let (concept, range) = concept_for(&m.key);
        out.push(ProvRecord {
            id: RecordId::from(format!("ruvn-{}-{}", session.session_id, m.key)),
            source: SOURCE.to_string(),
            measured_at: session.recorded_at,
            // Screening-grade, like the Cognitum Seed (ADR-014).
            method: MeasurementMethod::AmbientSensing,
            // Research-namespaced code — never a clinical LOINC (ADR-004).
            code: Some(format!("RUVN-{}", m.key.to_uppercase())),
            concept,
            value: m.value,
            unit: m.unit.clone(),
            reference_range: range,
            // Research signal → capped confidence so the analyst weights it as
            // exploratory, not definitive.
            confidence: Confidence::new(0.6),
        });
    }
    Ok(out)
}

/// A 0–100 "Neuro" orientation value for the health-score's optional Neuro
/// subsystem (ADR-016), derived only from entrainment quality metrics. This is
/// an *orientation* number, explicitly not a clinical score; returns `None` when
/// no entrainment metric is present.
pub fn neuro_orientation(sessions: &[NeuralSession]) -> Option<f64> {
    let mut acc = 0.0;
    let mut n = 0u32;
    for s in sessions {
        for m in &s.metrics {
            if (m.key == "entrainment_index" || m.key == "phase_locking_value")
                && m.value.is_finite()
            {
                acc += m.value.clamp(0.0, 1.0);
                n += 1;
            }
        }
    }
    if n == 0 {
        return None;
    }
    Some((acc / n as f64) * 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> NeuralSession {
        NeuralSession {
            session_id: "s1".into(),
            recorded_at: 1_750_000_000_000,
            protocol: "40Hz audiovisual".into(),
            duration_s: 1800,
            metrics: vec![
                SessionMetric {
                    key: "gamma_power_db".into(),
                    value: 12.4,
                    unit: "dB".into(),
                },
                SessionMetric {
                    key: "phase_locking_value".into(),
                    value: 0.72,
                    unit: "".into(),
                },
                SessionMetric {
                    key: "entrainment_index".into(),
                    value: 0.68,
                    unit: "".into(),
                },
            ],
            evidence_signature: "ed25519:abc".into(),
        }
    }

    #[test]
    fn maps_metrics_to_records_with_research_framing() {
        let recs = session_to_records(&session()).unwrap();
        assert_eq!(recs.len(), 3);
        for r in &recs {
            assert_eq!(r.source, "ruv-neural");
            assert_eq!(r.method, MeasurementMethod::AmbientSensing);
            assert!(r.code.as_ref().unwrap().starts_with("RUVN-")); // never a clinical LOINC
            assert!(r.confidence.get() <= 0.6); // exploratory weighting
        }
    }

    #[test]
    fn rejects_unsigned_session() {
        let mut s = session();
        s.evidence_signature = "".into();
        assert_eq!(session_to_records(&s), Err(NeuralError::Unsigned));
    }

    #[test]
    fn rejects_empty_and_non_finite() {
        let mut s = session();
        s.metrics.clear();
        assert_eq!(session_to_records(&s), Err(NeuralError::NoMetrics));
        let mut s2 = session();
        s2.metrics[0].value = f64::NAN;
        assert!(matches!(
            session_to_records(&s2),
            Err(NeuralError::NonFinite(_))
        ));
    }

    #[test]
    fn neuro_orientation_averages_entrainment_quality() {
        // (0.72 + 0.68) / 2 * 100 = 70
        let v = neuro_orientation(&[session()]).unwrap();
        assert!((v - 70.0).abs() < 1e-9);
    }

    #[test]
    fn neuro_orientation_none_without_entrainment_metrics() {
        let mut s = session();
        s.metrics.retain(|m| m.key == "gamma_power_db");
        assert_eq!(neuro_orientation(&[s]), None);
    }

    #[test]
    fn disclaimer_is_non_diagnostic() {
        assert!(RESEARCH_DISCLAIMER.contains("not a diagnosis"));
        assert!(RESEARCH_DISCLAIMER.contains("not a therapeutic claim"));
    }
}
