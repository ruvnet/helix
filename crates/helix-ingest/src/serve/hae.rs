//! Health Auto Export (HAE) JSON → `Vec<ProvRecord>` — the live ongoing connector.
//!
//! HAE is the iPhone app that POSTs Apple Health data as JSON on a schedule. Its
//! automation payload is:
//!
//! ```json
//! { "data": { "metrics": [
//!     { "name": "heart_rate", "units": "count/min",
//!       "data": [ { "date": "2026-06-01 10:00:00 -0700", "Min": 58, "Avg": 62, "Max": 121 } ] },
//!     { "name": "step_count", "units": "count",
//!       "data": [ { "date": "2026-06-01 00:00:00 -0700", "qty": 8412 } ] }
//! ] } }
//! ```
//!
//! Each metric datum carries a single `qty` OR an aggregate (`Min`/`Avg`/`Max`);
//! we take `qty` else `Avg`. We map a **documented subset** of HAE metric names to
//! (concept, LOINC) using the SAME LOINC codes as `helix-connect`'s Apple import,
//! so HAE and a raw `export.xml` normalize identically. Unmapped metric names are
//! reported honestly in [`HaeParsed::skipped`] — never silently dropped, never
//! faked into coverage. Records are stamped source `"Apple Health"` so they merge
//! with the Apple connector.

use helix_provenance::{Confidence, EpochMillis, MeasurementMethod, ProvRecord, RecordId};
use serde_json::Value;

/// Source label for HAE records (they ARE Apple Health data).
pub const HAE_SOURCE: &str = "Apple Health";
/// Upper bound on records parsed from one payload.
const HAE_MAX_RECORDS: usize = 100_000;

/// Outcome of parsing one HAE payload.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct HaeParsed {
    pub records: Vec<ProvRecord>,
    /// `"name×N"` per mapped metric (N = points accepted) — for an honest summary.
    pub mapped: Vec<String>,
    /// Metric names present in the payload but NOT in our supported subset.
    pub skipped: Vec<String>,
}

/// Map a documented subset of HAE metric names to `(concept, LOINC code)`.
/// LOINC codes mirror `helix-connect`'s `hk_map`. Unknown → `None` (→ skipped).
fn hae_map(name: &str) -> Option<(&'static str, &'static str)> {
    Some(match name {
        "heart_rate" => ("Heart rate", "8867-4"),
        "resting_heart_rate" => ("Resting heart rate", "40443-4"),
        "heart_rate_variability" => ("Heart rate variability (SDNN)", "80404-7"),
        "step_count" => ("Steps", "55423-8"),
        "weight_body_mass" => ("Body weight", "29463-7"),
        "body_mass_index" => ("Body mass index", "39156-5"),
        "blood_oxygen_saturation" | "oxygen_saturation" => ("Oxygen saturation (SpO2)", "59408-5"),
        "respiratory_rate" => ("Respiratory rate", "9279-1"),
        "body_temperature" => ("Body temperature", "8310-5"),
        "blood_glucose" => ("Blood glucose", "2339-0"),
        "vo2_max" => ("VO2 max", "84376-3"),
        "blood_pressure_systolic" => ("Systolic blood pressure", "8480-6"),
        "blood_pressure_diastolic" => ("Diastolic blood pressure", "8462-4"),
        "active_energy" => ("Active energy burned", "41981-2"),
        _ => return None,
    })
}

/// The supported metric names (for docs / an API self-description).
pub fn supported_metrics() -> &'static [&'static str] {
    &[
        "heart_rate",
        "resting_heart_rate",
        "heart_rate_variability",
        "step_count",
        "weight_body_mass",
        "body_mass_index",
        "blood_oxygen_saturation",
        "respiratory_rate",
        "body_temperature",
        "blood_glucose",
        "vo2_max",
        "blood_pressure_systolic",
        "blood_pressure_diastolic",
        "active_energy",
    ]
}

/// Parse a HAE payload. Accepts `{data:{metrics:[...]}}` or a bare `{metrics:[...]}`.
pub fn parse(payload: &Value) -> HaeParsed {
    let metrics = payload["data"]["metrics"]
        .as_array()
        .or_else(|| payload["metrics"].as_array());
    let Some(metrics) = metrics else {
        return HaeParsed::default();
    };

    let mut out = HaeParsed::default();
    for metric in metrics {
        let name = metric["name"].as_str().unwrap_or("");
        let Some((concept, code)) = hae_map(name) else {
            if !name.is_empty() {
                out.skipped.push(name.to_string());
            }
            continue;
        };
        let unit = metric["units"].as_str().unwrap_or("").to_string();
        let points = metric["data"].as_array().cloned().unwrap_or_default();

        let mut accepted = 0usize;
        for p in &points {
            if out.records.len() >= HAE_MAX_RECORDS {
                break;
            }
            let Some(value) = point_value(p) else { continue };
            let measured_at = p["date"].as_str().and_then(parse_hae_date).unwrap_or(0);
            out.records.push(ProvRecord {
                id: RecordId::from(format!("hae-{HAE_SOURCE}-{code}-{measured_at}")),
                source: HAE_SOURCE.to_string(),
                measured_at,
                method: MeasurementMethod::Device,
                code: Some(code.to_string()),
                concept: concept.to_string(),
                value,
                unit: unit.clone(),
                reference_range: None,
                confidence: Confidence::new(0.9),
            });
            accepted += 1;
        }
        if accepted > 0 {
            out.mapped.push(format!("{name}×{accepted}"));
        }
    }
    out
}

/// A HAE datum's numeric value: `qty` if present, else `Avg` (aggregate metrics).
fn point_value(p: &Value) -> Option<f64> {
    let v = p["qty"].as_f64().or_else(|| p["Avg"].as_f64())?;
    v.is_finite().then_some(v)
}

/// Parse HAE's `YYYY-MM-DD HH:MM:SS ±ZZZZ` (or ISO `T…Z`) into epoch millis.
/// Timezone offset is applied so the stored instant is UTC. Self-contained (no
/// chrono); mirrors `helix-connect`'s private Apple-date logic.
fn parse_hae_date(s: &str) -> Option<EpochMillis> {
    let s = s.trim();
    let b = s.as_bytes();
    if b.len() < 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let num = |a: usize, z: usize| s.get(a..z)?.parse::<i64>().ok();
    let (y, mo, d) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) {
        return None;
    }
    let (mut hh, mut mi, mut ss) = (0i64, 0i64, 0i64);
    // Time part follows a 'T' (ISO) or a ' ' (HAE) separator at index 10.
    if b.len() >= 19 && (b[10] == b'T' || b[10] == b' ') {
        hh = num(11, 13)?;
        mi = num(14, 16)?;
        ss = num(17, 19)?;
    }
    // Optional " +0530" / "-0700" offset at the tail (HAE local time → UTC).
    let off_ms = s.get(19..).and_then(|tz| {
        let tz = tz.trim();
        let sign = match tz.chars().next()? {
            '+' => 1,
            '-' => -1,
            _ => return None,
        };
        let hh: i64 = tz.get(1..3)?.parse().ok()?;
        let mm: i64 = tz.get(3..5)?.parse().ok()?;
        Some(sign * (hh * 3600 + mm * 60) * 1000)
    });
    // Civil-date → days since epoch (Howard Hinnant).
    let yy = if mo <= 2 { y - 1 } else { y };
    let era = yy.div_euclid(400);
    let yoe = yy - era * 400;
    let doy = (153 * (if mo > 2 { mo - 3 } else { mo + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    let local = (days * 86400 + hh * 3600 + mi * 60 + ss) * 1000;
    Some(local - off_ms.unwrap_or(0))
}
