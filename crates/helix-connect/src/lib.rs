//! # helix-connect — ADR-029: connector clients (FHIR/SMART + wearables)
//!
//! The buildable, testable core of the live-API connector tier (ADR-012): the
//! request/response shapes, **FHIR Observation → ProvRecord** parsing, the OAuth
//! token model, and the **degradation ladder** — all behind a transport trait so
//! they're fully tested with a sandbox and a real HTTP transport (with partner
//! auth) drops in later. No partner credential is needed to build or test this.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use helix_provenance::{
    Confidence, EpochMillis, MeasurementMethod, ProvRecord, RecordId, ReferenceRange,
};

/// Abstracts the network. Production: a real HTTP client + per-provider auth.
/// Tests: a sandbox returning canned payloads. `get` returns the response body.
pub trait HttpTransport {
    fn get(&self, url: &str, bearer: Option<&str>) -> Result<String, ConnectError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ConnectError {
    #[error("transport error: {0}")]
    Transport(String),
    #[error("authorization failed or token expired")]
    Unauthorized,
    #[error("rate limited; retry after {retry_after_s}s")]
    RateLimited { retry_after_s: u64 },
    #[error("service unavailable")]
    Unavailable,
    #[error("could not parse FHIR resource: {0}")]
    Parse(String),
}

/// An OAuth token (SMART on FHIR / wearable). **Secret** — never logged; held only
/// in the user's vault (ADR-001).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OAuthToken {
    pub access_token: String,
    pub refresh_token: Option<String>,
    /// Absolute expiry (epoch millis).
    pub expires_at: EpochMillis,
    pub scopes: Vec<String>,
}

impl OAuthToken {
    pub fn is_expired(&self, now: EpochMillis) -> bool {
        now >= self.expires_at
    }
    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|s| s == scope)
    }
}

impl std::fmt::Debug for OAuthTokenRedacted<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OAuthToken(***redacted***)")
    }
}
/// Wrapper to print a token without leaking it (for logs/audit).
pub struct OAuthTokenRedacted<'a>(pub &'a OAuthToken);

/// The fallback tier a connector drops to when the live API can't serve (ADR-012).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FallbackTier {
    /// Live API succeeded — no fallback.
    LiveApi,
    /// Ask the user to export the data from the provider's portal.
    UserExport,
    /// Parse a PDF/scan via OCR (helix-ocr, ADR-022).
    PdfOcr,
    /// Manual entry.
    Manual,
}

/// Map a live-API failure to the next degradation tier (ADR-012 made executable).
pub fn fallback_for(err: &ConnectError) -> FallbackTier {
    match err {
        ConnectError::Unauthorized | ConnectError::Unavailable => FallbackTier::UserExport,
        ConnectError::RateLimited { .. } => FallbackTier::UserExport,
        ConnectError::Parse(_) => FallbackTier::PdfOcr,
        ConnectError::Transport(_) => FallbackTier::UserExport,
    }
}

/// Parse a FHIR R4 `Observation` JSON value into a [`ProvRecord`].
/// Extracts LOINC code (ADR-004), value + UCUM unit, effective date, reference
/// range, and source. Un-parseable → `ConnectError::Parse` (→ review queue).
pub fn parse_observation(
    obs: &serde_json::Value,
    source: &str,
) -> Result<ProvRecord, ConnectError> {
    if obs["resourceType"].as_str() != Some("Observation") {
        return Err(ConnectError::Parse("not an Observation".into()));
    }
    let id = obs["id"].as_str().unwrap_or("obs").to_string();

    // LOINC coding.
    let coding = obs["code"]["coding"]
        .as_array()
        .and_then(|a| {
            a.iter()
                .find(|c| c["system"].as_str() == Some("http://loinc.org"))
        })
        .ok_or_else(|| ConnectError::Parse("no LOINC coding".into()))?;
    let code = coding["code"]
        .as_str()
        .ok_or_else(|| ConnectError::Parse("no code".into()))?;
    let concept = coding["display"]
        .as_str()
        .or_else(|| obs["code"]["text"].as_str())
        .unwrap_or(code)
        .to_string();

    // valueQuantity.
    let vq = &obs["valueQuantity"];
    let value = vq["value"]
        .as_f64()
        .ok_or_else(|| ConnectError::Parse("no valueQuantity.value".into()))?;
    if !value.is_finite() {
        return Err(ConnectError::Parse("non-finite value".into()));
    }
    let unit = vq["unit"]
        .as_str()
        .or_else(|| vq["code"].as_str())
        .unwrap_or("")
        .to_string();

    // effectiveDateTime → epoch millis (very small ISO-8601 parse: needs at least YYYY).
    let measured_at = parse_iso_millis(obs["effectiveDateTime"].as_str().unwrap_or(""))
        .ok_or_else(|| ConnectError::Parse("bad effectiveDateTime".into()))?;

    // referenceRange[0].
    let rr = obs["referenceRange"].as_array().and_then(|a| a.first());
    let reference_range =
        rr.map(|r| ReferenceRange::new(r["low"]["value"].as_f64(), r["high"]["value"].as_f64()));

    Ok(ProvRecord {
        id: RecordId::from(format!("fhir-{source}-{id}")),
        source: source.to_string(),
        measured_at,
        method: MeasurementMethod::LabFeed,
        code: Some(code.to_string()),
        concept,
        value,
        unit,
        reference_range,
        confidence: Confidence::FULL, // structured feed = full confidence
    })
}

/// Tiny ISO-8601 → epoch-millis parser for `YYYY-MM-DD[THH:MM:SS]Z`. Date-only or
/// with time; returns `None` on anything it can't read. (No chrono dependency.)
fn parse_iso_millis(s: &str) -> Option<EpochMillis> {
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
    if b.len() >= 19 && b[10] == b'T' {
        hh = num(11, 13)?;
        mi = num(14, 16)?;
        ss = num(17, 19)?;
    }
    // days since epoch via a civil-date algorithm (Howard Hinnant).
    let yy = if mo <= 2 { y - 1 } else { y };
    let era = yy.div_euclid(400);
    let yoe = yy - era * 400;
    let doy = (153 * (if mo > 2 { mo - 3 } else { mo + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    Some(((days * 86400 + hh * 3600 + mi * 60 + ss) * 1000) as EpochMillis)
}

/// A connector: fetch FHIR observations for a concept, parsing them into records,
/// or report the fallback tier on failure (ADR-012 degradation).
pub struct FhirConnector<'a, T: HttpTransport> {
    pub transport: &'a T,
    pub base_url: String,
    pub source: String,
}

/// Outcome of a connector pull.
#[derive(Debug, Clone, PartialEq)]
pub enum PullResult {
    /// Records parsed from the live API (plus any that went to review).
    Imported {
        records: Vec<ProvRecord>,
        queued_for_review: usize,
    },
    /// The live API couldn't serve; here's the fallback tier to use instead.
    Degraded {
        tier: FallbackTier,
        reason: ConnectError,
    },
}

impl<'a, T: HttpTransport> FhirConnector<'a, T> {
    pub fn new(transport: &'a T, base_url: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            transport,
            base_url: base_url.into(),
            source: source.into(),
        }
    }

    /// Pull `Observation`s for a LOINC code, refreshing nothing here (the caller
    /// supplies a valid token). On transport failure, degrade per ADR-012.
    pub fn pull_observations(
        &self,
        loinc: &str,
        token: &OAuthToken,
        now: EpochMillis,
    ) -> PullResult {
        if token.is_expired(now) {
            let e = ConnectError::Unauthorized;
            return PullResult::Degraded {
                tier: fallback_for(&e),
                reason: e,
            };
        }
        let url = format!(
            "{}/Observation?code=http://loinc.org|{}",
            self.base_url, loinc
        );
        let body = match self.transport.get(&url, Some(&token.access_token)) {
            Ok(b) => b,
            Err(e) => {
                return PullResult::Degraded {
                    tier: fallback_for(&e),
                    reason: e,
                }
            }
        };
        let bundle: serde_json::Value = match serde_json::from_str(&body) {
            Ok(v) => v,
            Err(e) => {
                let err = ConnectError::Parse(e.to_string());
                return PullResult::Degraded {
                    tier: fallback_for(&err),
                    reason: err,
                };
            }
        };
        let mut records = Vec::new();
        let mut queued = 0usize;
        let entries = bundle["entry"].as_array().cloned().unwrap_or_default();
        for entry in &entries {
            match parse_observation(&entry["resource"], &self.source) {
                Ok(r) => records.push(r),
                Err(_) => queued += 1, // → human-review queue (ADR-012/004)
            }
        }
        PullResult::Imported {
            records,
            queued_for_review: queued,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(
        code: &str,
        display: &str,
        value: f64,
        unit: &str,
        date: &str,
    ) -> serde_json::Value {
        serde_json::json!({
            "resourceType": "Observation", "id": "o1",
            "code": { "coding": [{ "system": "http://loinc.org", "code": code, "display": display }] },
            "valueQuantity": { "value": value, "unit": unit },
            "effectiveDateTime": date,
            "referenceRange": [{ "low": { "value": 30.0 }, "high": { "value": 400.0 } }]
        })
    }

    fn bundle(obs: Vec<serde_json::Value>) -> String {
        serde_json::json!({
            "resourceType": "Bundle",
            "entry": obs.into_iter().map(|o| serde_json::json!({"resource": o})).collect::<Vec<_>>()
        })
        .to_string()
    }

    struct Sandbox(Result<String, ConnectError>);
    impl HttpTransport for Sandbox {
        fn get(&self, _: &str, _: Option<&str>) -> Result<String, ConnectError> {
            self.0.clone()
        }
    }

    fn token(exp: EpochMillis) -> OAuthToken {
        OAuthToken {
            access_token: "secret".into(),
            refresh_token: Some("r".into()),
            expires_at: exp,
            scopes: vec!["patient/Observation.read".into()],
        }
    }

    #[test]
    fn parses_fhir_observation_to_record() {
        let obs = observation("2276-4", "Ferritin", 28.0, "ng/mL", "2026-06-19T10:00:00Z");
        let r = parse_observation(&obs, "MyChart").unwrap();
        assert_eq!(r.code.as_deref(), Some("2276-4"));
        assert_eq!(r.concept, "Ferritin");
        assert_eq!(r.value, 28.0);
        assert_eq!(r.unit, "ng/mL");
        assert_eq!(r.reference_range.unwrap().low, Some(30.0));
        assert!(r.measured_at > 0);
    }

    #[test]
    fn date_only_parses() {
        let obs = observation("2276-4", "Ferritin", 28.0, "ng/mL", "2026-06-19");
        assert!(parse_observation(&obs, "s").is_ok());
    }

    #[test]
    fn rejects_non_observation_and_missing_loinc() {
        assert!(parse_observation(&serde_json::json!({"resourceType": "Patient"}), "s").is_err());
        let no_loinc = serde_json::json!({
            "resourceType": "Observation", "id": "x",
            "code": { "coding": [{ "system": "http://snomed.info/sct", "code": "1" }] },
            "valueQuantity": { "value": 1.0, "unit": "x" }, "effectiveDateTime": "2026-01-01"
        });
        assert!(parse_observation(&no_loinc, "s").is_err());
    }

    #[test]
    fn connector_imports_and_queues_unparseable() {
        let good = observation("2276-4", "Ferritin", 28.0, "ng/mL", "2026-06-19");
        let bad = serde_json::json!({"resourceType": "Observation", "id": "b"}); // no code/value
        let sandbox = Sandbox(Ok(bundle(vec![good, bad])));
        let conn = FhirConnector::new(&sandbox, "https://ehr/fhir", "MyChart");
        match conn.pull_observations("2276-4", &token(1_000_000), 0) {
            PullResult::Imported {
                records,
                queued_for_review,
            } => {
                assert_eq!(records.len(), 1);
                assert_eq!(queued_for_review, 1);
            }
            other => panic!("expected import, got {other:?}"),
        }
    }

    #[test]
    fn expired_token_degrades_to_export() {
        let sandbox = Sandbox(Ok(bundle(vec![])));
        let conn = FhirConnector::new(&sandbox, "u", "s");
        match conn.pull_observations("2276-4", &token(100), 200) {
            PullResult::Degraded { tier, reason } => {
                assert_eq!(tier, FallbackTier::UserExport);
                assert_eq!(reason, ConnectError::Unauthorized);
            }
            other => panic!("expected degraded, got {other:?}"),
        }
    }

    #[test]
    fn transport_failure_degrades() {
        let sandbox = Sandbox(Err(ConnectError::RateLimited { retry_after_s: 30 }));
        let conn = FhirConnector::new(&sandbox, "u", "s");
        assert!(matches!(
            conn.pull_observations("2276-4", &token(1_000_000), 0),
            PullResult::Degraded {
                tier: FallbackTier::UserExport,
                ..
            }
        ));
    }

    #[test]
    fn parse_failure_degrades_to_ocr() {
        assert_eq!(
            fallback_for(&ConnectError::Parse("x".into())),
            FallbackTier::PdfOcr
        );
    }

    #[test]
    fn token_helpers_and_redaction() {
        let t = token(1000);
        assert!(t.is_expired(1000));
        assert!(!t.is_expired(999));
        assert!(t.has_scope("patient/Observation.read"));
        assert!(format!("{:?}", OAuthTokenRedacted(&t)).contains("redacted"));
    }
}

// --- Apple Health export.xml import (ADR-029) ---------------------------------

/// Map a curated set of Apple HealthKit quantity identifiers to (concept, LOINC
/// code, default unit). Unknown types are skipped (returned to review upstream).
fn hk_map(hk_type: &str) -> Option<(&'static str, &'static str)> {
    let t = hk_type
        .strip_prefix("HKQuantityTypeIdentifier")
        .unwrap_or(hk_type);
    Some(match t {
        "HeartRate" => ("Heart rate", "8867-4"),
        "RestingHeartRate" => ("Resting heart rate", "40443-4"),
        "HeartRateVariabilitySDNN" => ("Heart rate variability (SDNN)", "80404-7"),
        "StepCount" => ("Steps", "55423-8"),
        "BodyMass" => ("Body weight", "29463-7"),
        "BodyMassIndex" => ("Body mass index", "39156-5"),
        "OxygenSaturation" => ("Oxygen saturation (SpO2)", "59408-5"),
        "BloodPressureSystolic" => ("Systolic blood pressure", "8480-6"),
        "BloodPressureDiastolic" => ("Diastolic blood pressure", "8462-4"),
        "RespiratoryRate" => ("Respiratory rate", "9279-1"),
        "BodyTemperature" => ("Body temperature", "8310-5"),
        "BloodGlucose" => ("Blood glucose", "2339-0"),
        "VO2Max" => ("VO2 max", "84376-3"),
        _ => return None,
    })
}

/// Read the value of attribute `name` from a `<Record ...>` element body.
fn attr<'a>(elem: &'a str, name: &str) -> Option<&'a str> {
    let key = format!("{name}=\"");
    let start = elem.find(&key)? + key.len();
    let rest = &elem[start..];
    let end = rest.find('"')?;
    Some(&rest[..end])
}

/// Parse Apple Health's `startDate`/`creationDate` format
/// `YYYY-MM-DD HH:MM:SS ±ZZZZ` (or ISO `T`) into epoch millis. Timezone offset is
/// applied if present.
fn parse_apple_date(s: &str) -> Option<EpochMillis> {
    let s = s.trim();
    if s.len() < 19 {
        return parse_iso_millis(s);
    }
    let date = &s[..10];
    let time = &s[11..19];
    let base = parse_iso_millis(&format!("{date}T{time}"))?;
    // optional " +0530" / " -0700" offset at the tail
    let off_ms = s.get(20..).and_then(|tz| {
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
    // stored time is local; subtract offset to get UTC epoch
    Some(base - off_ms.unwrap_or(0))
}

/// Parse an Apple Health `export.xml` string into provenance records. Scans
/// `<Record ...>` elements, maps known HealthKit types (ADR-004), and skips
/// unknown/invalid ones. Bounded by `max_records` to avoid huge exports blowing
/// memory.
pub fn parse_apple_health(xml: &str, source: &str, max_records: usize) -> Vec<ProvRecord> {
    let mut out = Vec::new();
    let mut idx = 0usize;
    while let Some(rel) = xml[idx..].find("<Record ") {
        let start = idx + rel;
        let end = match xml[start..].find('>') {
            Some(e) => start + e,
            None => break,
        };
        let elem = &xml[start..end];
        idx = end + 1;

        let (Some(ty), Some(val_s)) = (attr(elem, "type"), attr(elem, "value")) else {
            continue;
        };
        let Some((concept, code)) = hk_map(ty) else {
            continue;
        };
        let Ok(value) = val_s.parse::<f64>() else {
            continue;
        };
        if !value.is_finite() {
            continue;
        }
        let unit = attr(elem, "unit").unwrap_or("").to_string();
        let measured_at = attr(elem, "startDate")
            .or_else(|| attr(elem, "creationDate"))
            .and_then(parse_apple_date)
            .unwrap_or(0);

        out.push(ProvRecord {
            id: RecordId::from(format!("apple-{source}-{}-{measured_at}", code)),
            source: source.to_string(),
            measured_at,
            method: MeasurementMethod::Device,
            code: Some(code.to_string()),
            concept: concept.to_string(),
            value,
            unit,
            reference_range: None,
            confidence: Confidence::new(0.9),
        });
        if out.len() >= max_records {
            break;
        }
    }
    out
}

#[cfg(test)]
mod apple_tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0"?>
<HealthData>
 <Record type="HKQuantityTypeIdentifierHeartRate" unit="count/min" startDate="2026-06-01 10:00:00 -0700" value="62"/>
 <Record type="HKQuantityTypeIdentifierRestingHeartRate" unit="count/min" startDate="2026-06-01 06:00:00 -0700" value="54"/>
 <Record type="HKQuantityTypeIdentifierBodyMass" unit="kg" startDate="2026-06-02 08:00:00 +0000" value="72.5"/>
 <Record type="HKQuantityTypeIdentifierSomethingUnknown" unit="x" startDate="2026-06-02" value="1"/>
 <Record type="HKQuantityTypeIdentifierStepCount" unit="count" startDate="2026-06-02" value="not_a_number"/>
</HealthData>"#;

    #[test]
    fn parses_known_records_skips_unknown_and_bad() {
        let recs = parse_apple_health(SAMPLE, "Apple Health", 1000);
        assert_eq!(recs.len(), 3); // HR, RHR, BodyMass; unknown + non-numeric skipped
        let hr = &recs[0];
        assert_eq!(hr.concept, "Heart rate");
        assert_eq!(hr.code.as_deref(), Some("8867-4"));
        assert_eq!(hr.value, 62.0);
        assert_eq!(hr.method, MeasurementMethod::Device);
        assert!(hr.measured_at > 0);
    }

    #[test]
    fn respects_max_records() {
        assert_eq!(parse_apple_health(SAMPLE, "s", 1).len(), 1);
    }

    #[test]
    fn apple_date_applies_offset() {
        // 10:00 -0700 → 17:00 UTC
        let utc = parse_apple_date("2026-06-01 10:00:00 -0700").unwrap();
        let utc_ref = parse_apple_date("2026-06-01 17:00:00 +0000").unwrap();
        assert_eq!(utc, utc_ref);
    }
}
