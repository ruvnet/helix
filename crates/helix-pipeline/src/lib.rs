//! # helix-pipeline — the grounded-answer pipeline (ADR-002 orchestration of ADR-005/006/007/009)
//!
//! This is the keystone: it composes the four anti-hallucination primitives into
//! the single flow §4 of the spec describes, in the order that makes the product
//! safe:
//!
//! 1. **Abstain first (ADR-006).** No data / stale / low-confidence → return a
//!    [`GapNotice`], never a guess.
//! 2. **Escalate (ADR-009).** If the latest value trips a red-flag threshold,
//!    optimization is suppressed and the Escalation Guardian message takes over.
//! 3. **Compute deterministically (ADR-007).** Trend facts (latest, mean, slope,
//!    direction, %-change, range crossings) are computed in code, never by an LLM.
//! 4. **Ground (ADR-005).** Every surfaced claim resolves to the actual records.
//! 5. **Tier (ADR-006).** Any recommendation carries its evidence tier; it is
//!    omitted entirely when escalation suppresses optimization.
//!
//! The whole pipeline is pure and clock-injected, so the end-to-end behaviour is
//! deterministic and covered by the integration test in `tests/`.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use helix_escalation::{EscalationLevel, EscalationResult, ThresholdRegistry};
use helix_evidence::{assess, AnswerVerdict, EvidenceTier, GapNotice, TieredRecommendation};
use helix_numeric::{
    self as num, range_crossings, slope_per_day, trend_direction, Point, RangeCrossing,
    TrendDirection,
};
use helix_provenance::{ground, DraftClaim, EpochMillis, GroundedClaim, ProvRecord, RecordId};

/// Everything the pipeline needs to answer a single concept question. Borrowed
/// so the caller keeps ownership of the vault records.
#[derive(Debug, Clone)]
pub struct AnalyzeRequest<'a> {
    /// Canonical concept code (LOINC etc., ADR-004).
    pub concept_code: &'a str,
    /// All records the retrieval layer pulled for this concept.
    pub records: &'a [ProvRecord],
    /// Caller-supplied "now" (the pipeline never reads the clock).
    pub now: EpochMillis,
    pub staleness_window_days: i64,
    pub confidence_floor: f64,
    /// Reference bounds for range-crossing detection.
    pub reference_low: Option<f64>,
    pub reference_high: Option<f64>,
    /// Slope dead-band (units/day) below which a trend reads as flat.
    pub flat_band_per_day: f64,
    /// Optional scale-invariant dead-band: fraction of the reference-range span
    /// over the observation window (ADR-036). When `> 0` and a reference range is
    /// present, this supersedes the absolute `flat_band_per_day` so one threshold
    /// works across markers of different scales. `0.0` = use the absolute band.
    pub flat_band_frac: f64,
}

/// Deterministically-computed trend facts (ADR-007 output handed to the analyst).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrendFacts {
    pub latest_value: f64,
    pub latest_at: EpochMillis,
    pub mean: f64,
    /// `None` when there are fewer than 2 points (trend undefined).
    pub slope_per_day: Option<f64>,
    pub direction: TrendDirection,
    pub percent_change: Option<f64>,
    pub crossings: Vec<RangeCrossing>,
    pub sample_size: usize,
}

/// A full grounded answer. `escalation.level == None` in the normal case.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroundedAnswer {
    pub escalation: EscalationResult,
    pub claims: Vec<GroundedClaim>,
    pub trend: TrendFacts,
    /// `None` when escalation suppresses optimization (ADR-009).
    pub recommendation: Option<TieredRecommendation>,
}

/// The pipeline outcome: either an honest abstention or a grounded answer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum AnswerOutcome {
    Abstained(GapNotice),
    Answered(Box<GroundedAnswer>),
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum PipelineError {
    #[error("evidence assessment failed: {0}")]
    Evidence(#[from] helix_evidence::EvidenceError),
    #[error("escalation failed: {0}")]
    Escalation(#[from] helix_escalation::EscalationError),
    #[error("numeric computation failed: {0}")]
    Numeric(#[from] helix_numeric::NumericError),
}

fn latest_record(records: &[ProvRecord]) -> Option<&ProvRecord> {
    records.iter().max_by_key(|r| r.measured_at)
}

/// Build the time series (sorted ascending) the numeric engine consumes.
fn series_of(records: &[ProvRecord]) -> Vec<Point> {
    let mut pts: Vec<Point> = records
        .iter()
        .map(|r| Point::new(r.measured_at, r.value))
        .collect();
    pts.sort_by_key(|p| p.t);
    pts
}

/// Observation window (days) spanned by a sorted series — newest minus oldest.
fn window_days_of(series: &[Point]) -> f64 {
    match (series.first(), series.last()) {
        (Some(a), Some(b)) => (b.t - a.t) as f64 / 86_400_000.0,
        _ => 0.0,
    }
}

/// Escalation that treats "concept not in the registry" as *no red-flag rule*
/// (level None) rather than an error — the registry only errors when asked about
/// a code it is supposed to know. A concept with no acute critical value simply
/// has no rule.
fn escalate(
    registry: &ThresholdRegistry,
    code: &str,
    value: f64,
) -> Result<EscalationResult, PipelineError> {
    if registry.get(code).is_none() {
        return Ok(EscalationResult {
            level: EscalationLevel::None,
            suppress_optimization: false,
            message: String::new(),
        });
    }
    Ok(registry.evaluate(code, value)?)
}

/// Run the full grounded-answer pipeline for one concept.
pub fn analyze(
    req: &AnalyzeRequest<'_>,
    registry: &ThresholdRegistry,
) -> Result<AnswerOutcome, PipelineError> {
    // 1. Abstain first (ADR-006).
    let latest = latest_record(req.records);
    match assess(
        req.now,
        latest,
        req.staleness_window_days,
        req.confidence_floor,
    )? {
        AnswerVerdict::Abstain(notice) => return Ok(AnswerOutcome::Abstained(notice)),
        AnswerVerdict::Answer => {}
    }
    // `latest` is Some here (assess abstains on None).
    let latest = latest.expect("assess returned Answer so a record exists");

    // 2. Escalate (ADR-009).
    let escalation = escalate(registry, req.concept_code, latest.value)?;

    // 3. Deterministic numerics (ADR-007).
    let series = series_of(req.records);
    let mean = num::mean(&series)?;
    let (slope, direction, pct) = if series.len() >= 2 {
        let s = slope_per_day(&series)?;
        // Prefer the scale-invariant relative band (ADR-036) when configured and a
        // reference range is available; otherwise the absolute band.
        let dir = match (req.flat_band_frac, req.reference_low, req.reference_high) {
            (frac, Some(lo), Some(hi)) if frac > 0.0 && hi > lo => {
                let window_days = window_days_of(&series);
                num::trend_direction_relative(s, hi - lo, window_days, frac)
            }
            _ => trend_direction(s, req.flat_band_per_day),
        };
        (Some(s), dir, num::percent_change(&series).ok())
    } else {
        (None, TrendDirection::Flat, None)
    };
    let crossings = if series.len() >= 2 {
        range_crossings(&series, req.reference_low, req.reference_high)?
    } else {
        Vec::new()
    };
    let trend = TrendFacts {
        latest_value: latest.value,
        latest_at: latest.measured_at,
        mean,
        slope_per_day: slope,
        direction,
        percent_change: pct,
        crossings,
        sample_size: series.len(),
    };

    // 4. Ground every claim (ADR-005).
    let cites: Vec<RecordId> = req.records.iter().map(|r| r.id.clone()).collect();
    let dir_word = match trend.direction {
        TrendDirection::Rising => "trending up",
        TrendDirection::Falling => "trending down",
        TrendDirection::Flat => "stable",
    };
    let claim_text = format!(
        "Your {} is {} {} and {} over your last {} reading(s).",
        latest.concept, latest.value, latest.unit, dir_word, trend.sample_size
    );
    let draft = DraftClaim::new(claim_text, cites);
    let claims = vec![ground(&draft, req.records).expect("citations are exactly the evidence set")];

    // 5. Tiered recommendation — suppressed when escalation fires (ADR-009).
    let recommendation = if escalation.suppress_optimization {
        None
    } else {
        Some(TieredRecommendation::new(
            format!(
                "Track {} on your next panel to confirm the {} trend.",
                latest.concept, dir_word
            ),
            EvidenceTier::YourData,
        ))
    };

    Ok(AnswerOutcome::Answered(Box::new(GroundedAnswer {
        escalation,
        claims,
        trend,
        recommendation,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: i64 = 86_400_000;

    #[test]
    fn abstains_when_no_records() {
        let reg = helix_escalation::builtin_registry_v1();
        let req = AnalyzeRequest {
            concept_code: "2276-4",
            records: &[],
            now: 100 * DAY,
            staleness_window_days: 365,
            confidence_floor: 0.5,
            reference_low: Some(30.0),
            reference_high: Some(400.0),
            flat_band_per_day: 0.0,
            flat_band_frac: 0.0,
        };
        assert!(matches!(
            analyze(&req, &reg).unwrap(),
            AnswerOutcome::Abstained(_)
        ));
    }
}
