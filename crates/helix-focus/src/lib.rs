//! # helix-focus — ADR-032: evidence-based "focus areas" (non-diagnostic)
//!
//! Picks the few things in the user's own data "worth attention" — by
//! **deterministic, explainable rules** (ADR-007), never an opaque model guess,
//! and never a diagnosis (ADR-010). Each item says *why* and cites the records
//! that triggered it (ADR-005).
//!
//! Rules: a value **out of reference range** (worse if newly so), a **worsening
//! trajectory** (sustained adverse slope), or a **stale critical marker** (retest
//! prompt, ADR-006). Red flags are handed to the Escalation Guardian (ADR-009)
//! upstream; this crate surfaces non-emergent attention items.

use serde::{Deserialize, Serialize};

use helix_numeric::{slope_per_day, Point};
use helix_provenance::{EpochMillis, ProvRecord, RangePosition};

/// Why a concept was surfaced as a focus area.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FocusReason {
    /// Latest value is outside its reference range.
    OutOfRange,
    /// Sustained adverse trend (moving the wrong way over time).
    WorseningTrend,
    /// A critical marker whose latest value is older than the staleness window.
    StaleCritical,
}

/// Severity for ranking (not a clinical grade).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info = 0,
    Watch = 1,
    Elevated = 2,
}

/// One focus item — "worth attention", with its reason, citations, and a
/// non-diagnostic message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FocusItem {
    pub concept: String,
    pub reason: FocusReason,
    pub severity: Severity,
    pub message: String,
    /// Record ids that triggered this item (ADR-005 provenance).
    pub cites: Vec<String>,
}

/// Config for focus selection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FocusConfig {
    /// Adverse slope magnitude (units/day) above which a trend is "worsening".
    pub trend_band_per_day: f64,
    /// Days after which a critical marker is "stale".
    pub stale_days: i64,
    /// Concept codes considered critical (for the stale-critical rule).
    pub critical_codes: Vec<String>,
}

impl Default for FocusConfig {
    fn default() -> Self {
        Self {
            trend_band_per_day: 0.0,
            stale_days: 365,
            critical_codes: vec![],
        }
    }
}

fn latest<'a>(recs: &'a [&ProvRecord]) -> &'a ProvRecord {
    recs.iter().copied().max_by_key(|r| r.measured_at).unwrap()
}

/// Whether a worsening trend for this concept means *rising* or *falling* depends
/// on the marker; without that knowledge we treat any out-of-range *and* moving
/// further out as worsening. Here we conservatively flag a sustained move toward
/// (or further past) the nearest breached bound.
fn adverse_slope(recs: &[&ProvRecord], band: f64) -> Option<f64> {
    if recs.len() < 3 {
        return None;
    }
    let mut pts: Vec<Point> = recs
        .iter()
        .map(|r| Point::new(r.measured_at, r.value))
        .collect();
    pts.sort_by_key(|p| p.t);
    let s = slope_per_day(&pts).ok()?;
    let last = latest(recs);
    match last.range_position() {
        // Below range and still falling, or above range and still rising → worsening.
        Some(RangePosition::Below) if s < -band => Some(s),
        Some(RangePosition::Above) if s > band => Some(s),
        _ => None,
    }
}

/// Select focus areas from records grouped by concept code. `now` is supplied
/// (deterministic, ADR-007). Returns items ranked by severity then recency.
pub fn select_focus(records: &[ProvRecord], now: EpochMillis, cfg: &FocusConfig) -> Vec<FocusItem> {
    // group by code
    let mut by_code: std::collections::BTreeMap<&str, Vec<&ProvRecord>> = Default::default();
    for r in records {
        if let Some(c) = &r.code {
            by_code.entry(c.as_str()).or_default().push(r);
        }
    }

    let mut out = Vec::new();
    for (code, recs) in &by_code {
        let last = latest(recs);
        let cites: Vec<String> = recs.iter().map(|r| r.id.0.clone()).collect();

        // Rule 1: out of range.
        if matches!(
            last.range_position(),
            Some(RangePosition::Below) | Some(RangePosition::Above)
        ) {
            // Rule 2 (worsening) upgrades severity if also trending adversely.
            let worsening = adverse_slope(recs, cfg.trend_band_per_day).is_some();
            out.push(FocusItem {
                concept: last.concept.clone(),
                reason: if worsening { FocusReason::WorseningTrend } else { FocusReason::OutOfRange },
                severity: if worsening { Severity::Elevated } else { Severity::Watch },
                message: format!(
                    "{} is {} its reference range{} — worth discussing with your clinician (not a diagnosis).",
                    last.concept,
                    if matches!(last.range_position(), Some(RangePosition::Below)) { "below" } else { "above" },
                    if worsening { " and trending further out" } else { "" }
                ),
                cites: cites.clone(),
            });
        } else if let Some(_s) = adverse_slope(recs, cfg.trend_band_per_day) {
            // Worsening trend even while still in range (early signal).
            out.push(FocusItem {
                concept: last.concept.clone(),
                reason: FocusReason::WorseningTrend,
                severity: Severity::Watch,
                message: format!(
                    "{} is trending the wrong way — worth keeping an eye on.",
                    last.concept
                ),
                cites: cites.clone(),
            });
        }

        // Rule 3: stale critical marker.
        if cfg.critical_codes.iter().any(|c| c == code) {
            let age_days = (now - last.measured_at).max(0) / 86_400_000;
            if age_days > cfg.stale_days {
                out.push(FocusItem {
                    concept: last.concept.clone(),
                    reason: FocusReason::StaleCritical,
                    severity: Severity::Watch,
                    message: format!(
                        "Your last {} is {} days old — consider retesting.",
                        last.concept, age_days
                    ),
                    cites,
                });
            }
        }
    }

    // rank: severity desc, then most-recent trigger
    out.sort_by(|a, b| b.severity.cmp(&a.severity).then(a.concept.cmp(&b.concept)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use helix_provenance::{Confidence, MeasurementMethod, RecordId, ReferenceRange};

    const DAY: i64 = 86_400_000;

    fn rec(
        id: &str,
        code: &str,
        concept: &str,
        days_ago: i64,
        value: f64,
        lo: f64,
        hi: f64,
    ) -> ProvRecord {
        ProvRecord {
            id: RecordId::from(id),
            source: "Quest".into(),
            measured_at: 1000 * DAY - days_ago * DAY,
            method: MeasurementMethod::LabFeed,
            code: Some(code.into()),
            concept: concept.into(),
            value,
            unit: "ng/mL".into(),
            reference_range: Some(ReferenceRange::new(Some(lo), Some(hi))),
            confidence: Confidence::FULL,
        }
    }

    #[test]
    fn flags_out_of_range_value() {
        let recs = vec![rec("f", "2276-4", "Ferritin", 0, 22.0, 30.0, 400.0)];
        let out = select_focus(&recs, 1000 * DAY, &FocusConfig::default());
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].reason, FocusReason::OutOfRange);
        assert!(out[0].message.contains("below"));
        assert!(out[0].message.contains("not a diagnosis"));
    }

    #[test]
    fn worsening_out_of_range_is_elevated() {
        // ferritin below range and still falling across 3 draws
        let recs = vec![
            rec("a", "2276-4", "Ferritin", 60, 33.0, 30.0, 400.0),
            rec("b", "2276-4", "Ferritin", 30, 28.0, 30.0, 400.0),
            rec("c", "2276-4", "Ferritin", 0, 22.0, 30.0, 400.0),
        ];
        let out = select_focus(&recs, 1000 * DAY, &FocusConfig::default());
        assert_eq!(out[0].reason, FocusReason::WorseningTrend);
        assert_eq!(out[0].severity, Severity::Elevated);
    }

    #[test]
    fn in_range_stable_is_not_flagged() {
        let recs = vec![rec("x", "2823-3", "Potassium", 0, 4.2, 3.5, 5.1)];
        assert!(select_focus(&recs, 1000 * DAY, &FocusConfig::default()).is_empty());
    }

    #[test]
    fn stale_critical_marker_prompts_retest() {
        let cfg = FocusConfig {
            critical_codes: vec!["2823-3".into()],
            stale_days: 365,
            ..Default::default()
        };
        let recs = vec![rec("k", "2823-3", "Potassium", 500, 4.2, 3.5, 5.1)]; // in range but 500d old
        let out = select_focus(&recs, 1000 * DAY, &cfg);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].reason, FocusReason::StaleCritical);
        assert!(out[0].message.contains("retesting"));
    }

    #[test]
    fn ranking_puts_elevated_first() {
        let recs = vec![
            rec("p", "x", "Calm marker", 0, 9.0, 3.0, 10.0), // in range
            rec("a", "y", "Bad-a", 60, 33.0, 30.0, 400.0),
            rec("b", "y", "Bad-a", 30, 28.0, 30.0, 400.0),
            rec("c", "y", "Bad-a", 0, 22.0, 30.0, 400.0), // worsening → elevated
        ];
        let out = select_focus(&recs, 1000 * DAY, &FocusConfig::default());
        assert_eq!(out[0].severity, Severity::Elevated);
    }
}
