//! OpenMed integration boundary for clinical text.
//!
//! OpenMed is a probabilistic candidate-span producer. Helix remains the
//! policy authority: it pins model artifacts, proves complete document
//! coverage, normalizes offsets, adds deterministic identifier findings and
//! fails closed before text can leave the local trust boundary.

use hmac::{Hmac, Mac};
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

type HmacSha256 = Hmac<Sha256>;

pub const POLICY_VERSION: &str = "helix-openmed-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OffsetUnit {
    Utf8Bytes,
    UnicodeScalars,
    Utf16CodeUnits,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenMedSpan {
    pub start: usize,
    pub end: usize,
    #[serde(default = "default_offset_unit")]
    pub offset_unit: OffsetUnit,
    pub entity_type: String,
    #[serde(default)]
    pub canonical_label: Option<String>,
    #[serde(default)]
    pub policy_label: Option<String>,
    pub score: Option<f64>,
    #[serde(default = "default_detector")]
    pub detector: String,
}

fn default_offset_unit() -> OffsetUnit {
    OffsetUnit::Utf8Bytes
}

fn default_detector() -> String {
    "openmed".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDigest {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactLock {
    pub model_id: String,
    /// Immutable source revision. Branch names and moving tags are rejected.
    pub revision: String,
    pub files: Vec<ArtifactDigest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverageWindow {
    pub start_byte: usize,
    pub end_byte: usize,
    pub ordinal: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InferenceCoverage {
    pub text_utf8_len: usize,
    pub windows: Vec<CoverageWindow>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GatePolicy {
    #[serde(default = "default_threshold")]
    pub minimum_score: f64,
    #[serde(default = "default_true")]
    pub redact_quasi_identifiers: bool,
    #[serde(default = "default_true")]
    pub block_unknown_labels: bool,
}

fn default_threshold() -> f64 {
    0.5
}

fn default_true() -> bool {
    true
}

impl Default for GatePolicy {
    fn default() -> Self {
        Self {
            minimum_score: default_threshold(),
            redact_quasi_identifiers: true,
            block_unknown_labels: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GateRequest {
    pub text: String,
    pub spans: Vec<OpenMedSpan>,
    pub coverage: InferenceCoverage,
    pub artifact_lock: ArtifactLock,
    /// Digests observed by the loader before model execution.
    pub loaded_artifacts: Vec<ArtifactDigest>,
    #[serde(default)]
    pub policy: GatePolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataClass {
    ClinicalConcept,
    QuasiIdentifier,
    DirectIdentifier,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClinicalSpanRecord {
    pub start_byte: usize,
    pub end_byte: usize,
    pub entity_type: String,
    pub class: DataClass,
    pub score: f64,
    pub detector: String,
    /// Vault-scoped HMAC of the source slice. Never a portable plain hash.
    pub source_hmac: String,
    pub action: SpanAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpanAction {
    KeepLocal,
    Redact,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReleaseReceipt {
    pub policy_version: String,
    pub model_id: String,
    pub model_revision: String,
    pub artifact_lock_sha256: String,
    pub source_document_hmac: String,
    pub findings: Vec<ClinicalSpanRecord>,
    pub covered_utf8_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum GateOutcome {
    Approved {
        redacted_text: String,
        receipt: ReleaseReceipt,
    },
    Blocked {
        code: String,
        reason: String,
    },
}

#[derive(Debug, Error, PartialEq)]
pub enum GateError {
    #[error("HMAC key must contain at least 32 bytes")]
    WeakHmacKey,
    #[error("invalid model artifact lock: {0}")]
    InvalidArtifactLock(String),
    #[error("loaded model artifacts do not match the lock")]
    ArtifactMismatch,
    #[error("inference coverage is incomplete: {0}")]
    IncompleteCoverage(String),
    #[error("invalid OpenMed span: {0}")]
    InvalidSpan(String),
    #[error("policy is invalid: {0}")]
    InvalidPolicy(String),
    #[error("unknown OpenMed policy label: {0}")]
    UnknownLabel(String),
}

/// Create overlapping Unicode-safe windows. Returned boundaries are UTF-8 byte
/// offsets so downstream coverage checks and redaction use one canonical unit.
pub fn plan_windows(
    text: &str,
    max_scalars: usize,
    overlap_scalars: usize,
) -> Result<InferenceCoverage, GateError> {
    if max_scalars == 0 || overlap_scalars >= max_scalars {
        return Err(GateError::IncompleteCoverage(
            "max_scalars must be positive and overlap must be smaller".into(),
        ));
    }
    if text.is_empty() {
        return Ok(InferenceCoverage {
            text_utf8_len: 0,
            windows: Vec::new(),
        });
    }
    let mut boundaries: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();
    boundaries.push(text.len());
    let scalar_len = boundaries.len() - 1;
    let mut windows = Vec::new();
    let mut start = 0usize;
    while start < scalar_len {
        let end = (start + max_scalars).min(scalar_len);
        windows.push(CoverageWindow {
            start_byte: boundaries[start],
            end_byte: boundaries[end],
            ordinal: windows.len(),
        });
        if end == scalar_len {
            break;
        }
        start = end - overlap_scalars;
    }
    Ok(InferenceCoverage {
        text_utf8_len: text.len(),
        windows,
    })
}

pub fn validate_artifacts(lock: &ArtifactLock, loaded: &[ArtifactDigest]) -> Result<(), GateError> {
    if lock.model_id.trim().is_empty() {
        return Err(GateError::InvalidArtifactLock("empty model id".into()));
    }
    let revision = lock.revision.trim();
    if revision.len() < 12
        || !revision.bytes().all(|b| b.is_ascii_hexdigit())
        || ["main", "master", "latest", "snapshot"]
            .iter()
            .any(|moving| revision.eq_ignore_ascii_case(moving))
    {
        return Err(GateError::InvalidArtifactLock(
            "revision must be an immutable hexadecimal commit or content id".into(),
        ));
    }
    if lock.files.is_empty() {
        return Err(GateError::InvalidArtifactLock("no artifact files".into()));
    }
    let expected = digest_map(&lock.files)?;
    let actual = digest_map(loaded)?;
    if !expected.keys().any(|p| p.ends_with(".onnx")) {
        return Err(GateError::InvalidArtifactLock(
            "at least one ONNX graph must be locked".into(),
        ));
    }
    if expected != actual {
        return Err(GateError::ArtifactMismatch);
    }
    Ok(())
}

fn digest_map(files: &[ArtifactDigest]) -> Result<BTreeMap<&str, String>, GateError> {
    let mut out = BTreeMap::new();
    for file in files {
        if file.path.is_empty()
            || file.path.starts_with('/')
            || file.path.contains(['\\', ':', '?', '#'])
            || file.path.split('/').any(|part| part == "..")
        {
            return Err(GateError::InvalidArtifactLock(format!(
                "unsafe artifact path: {}",
                file.path
            )));
        }
        if file.sha256.len() != 64 || !file.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(GateError::InvalidArtifactLock(format!(
                "invalid SHA-256 for {}",
                file.path
            )));
        }
        if out
            .insert(file.path.as_str(), file.sha256.to_ascii_lowercase())
            .is_some()
        {
            return Err(GateError::InvalidArtifactLock(format!(
                "duplicate artifact path: {}",
                file.path
            )));
        }
    }
    Ok(out)
}

pub fn gate_document(request: &GateRequest, hmac_key: &[u8]) -> Result<GateOutcome, GateError> {
    if hmac_key.len() < 32 {
        return Err(GateError::WeakHmacKey);
    }
    if !request.policy.minimum_score.is_finite()
        || !(0.0..=1.0).contains(&request.policy.minimum_score)
    {
        return Err(GateError::InvalidPolicy(
            "minimum_score must be finite and in 0..=1".into(),
        ));
    }
    validate_artifacts(&request.artifact_lock, &request.loaded_artifacts)?;
    validate_coverage(&request.text, &request.coverage)?;

    let mut findings = normalize_openmed_spans(&request.text, &request.spans, &request.policy)?;
    findings.extend(detect_direct_identifiers(&request.text)?);
    findings = merge_findings(findings);

    if request.policy.block_unknown_labels {
        // Unknown labels are rejected during normalization. This branch makes
        // the fail-closed policy explicit in the serialized configuration.
    }

    let mut records = Vec::with_capacity(findings.len());
    for finding in &findings {
        let action = if finding.class == DataClass::DirectIdentifier
            || (finding.class == DataClass::QuasiIdentifier
                && request.policy.redact_quasi_identifiers)
        {
            SpanAction::Redact
        } else {
            SpanAction::KeepLocal
        };
        records.push(ClinicalSpanRecord {
            start_byte: finding.start,
            end_byte: finding.end,
            entity_type: finding.entity_type.clone(),
            class: finding.class,
            score: finding.score,
            detector: finding.detector.clone(),
            source_hmac: keyed_hash(
                hmac_key,
                &request.text.as_bytes()[finding.start..finding.end],
            )?,
            action,
        });
    }

    let redacted_text = redact(&request.text, &records);
    let lock_bytes = serde_json::to_vec(&request.artifact_lock)
        .map_err(|e| GateError::InvalidArtifactLock(e.to_string()))?;
    let receipt = ReleaseReceipt {
        policy_version: POLICY_VERSION.to_string(),
        model_id: request.artifact_lock.model_id.clone(),
        model_revision: request.artifact_lock.revision.clone(),
        artifact_lock_sha256: sha256_hex(&lock_bytes),
        source_document_hmac: keyed_hash(hmac_key, request.text.as_bytes())?,
        findings: records,
        covered_utf8_bytes: request.text.len(),
    };
    Ok(GateOutcome::Approved {
        redacted_text,
        receipt,
    })
}

/// Converts expected validation failures to a serializable blocked decision.
/// This is the recommended boundary for any egress path.
pub fn release_or_block(request: &GateRequest, hmac_key: &[u8]) -> GateOutcome {
    match gate_document(request, hmac_key) {
        Ok(outcome) => outcome,
        Err(error) => GateOutcome::Blocked {
            code: error_code(&error).to_string(),
            reason: error.to_string(),
        },
    }
}

fn error_code(error: &GateError) -> &'static str {
    match error {
        GateError::WeakHmacKey => "weak_hmac_key",
        GateError::InvalidArtifactLock(_) => "invalid_artifact_lock",
        GateError::ArtifactMismatch => "artifact_mismatch",
        GateError::IncompleteCoverage(_) => "incomplete_coverage",
        GateError::InvalidSpan(_) => "invalid_span",
        GateError::InvalidPolicy(_) => "invalid_policy",
        GateError::UnknownLabel(_) => "unknown_label",
    }
}

fn validate_coverage(text: &str, coverage: &InferenceCoverage) -> Result<(), GateError> {
    if coverage.text_utf8_len != text.len() {
        return Err(GateError::IncompleteCoverage(
            "declared text length does not match input".into(),
        ));
    }
    if text.is_empty() {
        if coverage.windows.is_empty() {
            return Ok(());
        }
        return Err(GateError::IncompleteCoverage(
            "empty input must have no windows".into(),
        ));
    }
    if coverage.windows.is_empty() {
        return Err(GateError::IncompleteCoverage("no inference windows".into()));
    }
    let mut windows = coverage.windows.clone();
    windows.sort_by_key(|w| (w.start_byte, w.end_byte));
    let mut covered_until = 0usize;
    let mut ordinals = BTreeSet::new();
    for window in windows {
        if window.start_byte >= window.end_byte
            || window.end_byte > text.len()
            || !text.is_char_boundary(window.start_byte)
            || !text.is_char_boundary(window.end_byte)
            || !ordinals.insert(window.ordinal)
        {
            return Err(GateError::IncompleteCoverage(
                "invalid window boundary or duplicate ordinal".into(),
            ));
        }
        if window.start_byte > covered_until {
            return Err(GateError::IncompleteCoverage(format!(
                "gap begins at byte {covered_until}"
            )));
        }
        covered_until = covered_until.max(window.end_byte);
    }
    if covered_until != text.len() {
        return Err(GateError::IncompleteCoverage(format!(
            "coverage ends at byte {covered_until} of {}",
            text.len()
        )));
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct Finding {
    start: usize,
    end: usize,
    entity_type: String,
    class: DataClass,
    score: f64,
    detector: String,
}

fn normalize_openmed_spans(
    text: &str,
    spans: &[OpenMedSpan],
    policy: &GatePolicy,
) -> Result<Vec<Finding>, GateError> {
    let mut out = Vec::new();
    for span in spans {
        let score = span
            .score
            .ok_or_else(|| GateError::InvalidSpan("score is required".into()))?;
        if !score.is_finite() || !(0.0..=1.0).contains(&score) {
            return Err(GateError::InvalidSpan("score must be in 0..=1".into()));
        }
        if score < policy.minimum_score {
            continue;
        }
        let start = offset_to_byte(text, span.start, span.offset_unit)?;
        let end = offset_to_byte(text, span.end, span.offset_unit)?;
        if start >= end {
            return Err(GateError::InvalidSpan("start must precede end".into()));
        }
        let class = classify(span);
        let class = match class {
            Some(value) => value,
            None if policy.block_unknown_labels => {
                return Err(GateError::UnknownLabel(span.entity_type.clone()))
            }
            None => DataClass::DirectIdentifier,
        };
        out.push(Finding {
            start,
            end,
            entity_type: span.entity_type.clone(),
            class,
            score,
            detector: span.detector.clone(),
        });
    }
    Ok(out)
}

fn classify(span: &OpenMedSpan) -> Option<DataClass> {
    let label = span
        .policy_label
        .as_deref()
        .or(span.canonical_label.as_deref())
        .unwrap_or(&span.entity_type)
        .to_ascii_lowercase()
        .replace(['-', ' '], "_");
    match label.as_str() {
        "direct_identifier"
        | "name"
        | "first_name"
        | "middle_name"
        | "last_name"
        | "person"
        | "patient"
        | "doctor"
        | "username"
        | "email"
        | "password"
        | "phone"
        | "fax"
        | "ssn"
        | "social_security_number"
        | "mrn"
        | "medical_record_number"
        | "account_number"
        | "certificate_number"
        | "url"
        | "ip_address"
        | "mac_address"
        | "imei"
        | "id_num"
        | "api_key"
        | "pin"
        | "credit_card"
        | "cvv"
        | "iban"
        | "vin"
        | "device_identifier"
        | "biometric_identifier" => Some(DataClass::DirectIdentifier),
        "quasi_identifier" | "date" | "date_of_birth" | "time" | "age" | "location" | "address"
        | "street_address" | "city" | "state" | "zip" | "zipcode" | "postal_code"
        | "organization" | "profession" | "occupation" | "job_title" | "gps_coordinates" => {
            Some(DataClass::QuasiIdentifier)
        }
        "clinical_concept" | "clinical" | "condition" | "diagnosis" | "medication" | "drug"
        | "lab" | "observation" | "procedure" | "anatomy" | "symptom" | "dosage" | "strength"
        | "frequency" | "duration" => Some(DataClass::ClinicalConcept),
        _ => None,
    }
}

fn offset_to_byte(text: &str, offset: usize, unit: OffsetUnit) -> Result<usize, GateError> {
    match unit {
        OffsetUnit::Utf8Bytes => {
            if offset <= text.len() && text.is_char_boundary(offset) {
                Ok(offset)
            } else {
                Err(GateError::InvalidSpan(
                    "UTF-8 offset is not a character boundary".into(),
                ))
            }
        }
        OffsetUnit::UnicodeScalars => {
            if offset == text.chars().count() {
                return Ok(text.len());
            }
            text.char_indices()
                .nth(offset)
                .map(|(index, _)| index)
                .ok_or_else(|| GateError::InvalidSpan("scalar offset exceeds input".into()))
        }
        OffsetUnit::Utf16CodeUnits => {
            let mut units = 0usize;
            for (index, ch) in text.char_indices() {
                if units == offset {
                    return Ok(index);
                }
                units += ch.len_utf16();
                if units > offset {
                    return Err(GateError::InvalidSpan(
                        "UTF-16 offset splits a surrogate pair".into(),
                    ));
                }
            }
            if units == offset {
                Ok(text.len())
            } else {
                Err(GateError::InvalidSpan("UTF-16 offset exceeds input".into()))
            }
        }
    }
}

fn detect_direct_identifiers(text: &str) -> Result<Vec<Finding>, GateError> {
    let patterns = [
        ("EMAIL", r"(?i)\b[A-Z0-9._%+\-]+@[A-Z0-9.\-]+\.[A-Z]{2,}\b"),
        ("SSN", r"\b\d{3}-\d{2}-\d{4}\b"),
        (
            "PHONE",
            r"(?x)\b(?:\+?1[ .-]?)?(?:\(?\d{3}\)?[ .-]?)\d{3}[ .-]\d{4}\b",
        ),
        ("IP_ADDRESS", r"\b(?:\d{1,3}\.){3}\d{1,3}\b"),
        (
            "MRN",
            r"(?i)\b(?:MRN|medical[ ]record[ ](?:number|no\.?))[ :#-]*[A-Z0-9-]{4,}\b",
        ),
    ];
    let mut out = Vec::new();
    for (entity_type, pattern) in patterns {
        let re = Regex::new(pattern)
            .map_err(|error| GateError::InvalidPolicy(format!("detector regex: {error}")))?;
        out.extend(re.find_iter(text).map(|hit| Finding {
            start: hit.start(),
            end: hit.end(),
            entity_type: entity_type.to_string(),
            class: DataClass::DirectIdentifier,
            score: 1.0,
            detector: "helix_deterministic_v1".to_string(),
        }));
    }
    Ok(out)
}

fn merge_findings(mut findings: Vec<Finding>) -> Vec<Finding> {
    findings.sort_by(|a, b| {
        a.start
            .cmp(&b.start)
            .then_with(|| b.end.cmp(&a.end))
            .then_with(|| b.class.cmp(&a.class))
    });
    let mut merged: Vec<Finding> = Vec::new();
    for finding in findings {
        if let Some(last) = merged.last_mut() {
            if finding.start < last.end {
                last.end = last.end.max(finding.end);
                if finding.class > last.class {
                    last.class = finding.class;
                    last.entity_type = finding.entity_type;
                    last.detector = finding.detector;
                }
                last.score = last.score.max(finding.score);
                continue;
            }
        }
        merged.push(finding);
    }
    merged
}

fn redact(text: &str, records: &[ClinicalSpanRecord]) -> String {
    let mut output = String::with_capacity(text.len());
    let mut cursor = 0usize;
    for record in records {
        if record.action != SpanAction::Redact {
            continue;
        }
        output.push_str(&text[cursor..record.start_byte]);
        output.push_str("[REDACTED:");
        output.push_str(&record.entity_type.to_ascii_uppercase());
        output.push(']');
        cursor = record.end_byte;
    }
    output.push_str(&text[cursor..]);
    output
}

fn keyed_hash(key: &[u8], value: &[u8]) -> Result<String, GateError> {
    let mut mac = HmacSha256::new_from_slice(key).map_err(|_| GateError::WeakHmacKey)?;
    mac.update(value);
    Ok(hex(&mac.finalize().into_bytes()))
}

fn sha256_hex(value: &[u8]) -> String {
    hex(&Sha256::digest(value))
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &[u8; 32] = b"helix-test-only-hmac-key-32-byte";

    fn digest(path: &str, digit: char) -> ArtifactDigest {
        ArtifactDigest {
            path: path.into(),
            sha256: std::iter::repeat(digit).take(64).collect(),
        }
    }

    fn request(text: &str) -> GateRequest {
        let files = vec![digest("model.onnx", 'a'), digest("tokenizer.json", 'b')];
        GateRequest {
            text: text.into(),
            spans: Vec::new(),
            coverage: plan_windows(text, 32, 8).unwrap(),
            artifact_lock: ArtifactLock {
                model_id: "openmed/clinical-deid".into(),
                revision: "4ee5b28d0f118a2a521cb781551c1dcd343f8db2".into(),
                files: files.clone(),
            },
            loaded_artifacts: files,
            policy: GatePolicy::default(),
        }
    }

    #[test]
    fn unicode_windows_cover_every_byte() {
        let text = "A🙂é中文 clinical note";
        let coverage = plan_windows(text, 5, 2).unwrap();
        validate_coverage(text, &coverage).unwrap();
        assert!(coverage.windows.len() > 1);
        assert_eq!(coverage.windows.last().unwrap().end_byte, text.len());
    }

    #[test]
    fn converts_utf16_offsets_without_splitting_surrogates() {
        let text = "🙂 Dr. Ada";
        assert_eq!(
            offset_to_byte(text, 3, OffsetUnit::Utf16CodeUnits).unwrap(),
            5
        );
        assert!(offset_to_byte(text, 1, OffsetUnit::Utf16CodeUnits).is_err());
    }

    #[test]
    fn deterministic_detector_catches_identifier_missed_by_model() {
        let text = "Contact ada@example.org about ferritin.";
        let output = gate_document(&request(text), KEY).unwrap();
        let GateOutcome::Approved {
            redacted_text,
            receipt,
        } = output
        else {
            panic!("expected approval")
        };
        assert_eq!(redacted_text, "Contact [REDACTED:EMAIL] about ferritin.");
        assert_eq!(receipt.findings.len(), 1);
        assert!(!serde_json::to_string(&receipt)
            .unwrap()
            .contains("ada@example.org"));
    }

    #[test]
    fn clinical_concepts_are_preserved() {
        let text = "Patient takes metformin";
        let mut req = request(text);
        req.spans.push(OpenMedSpan {
            start: 14,
            end: 23,
            offset_unit: OffsetUnit::Utf8Bytes,
            entity_type: "MEDICATION".into(),
            canonical_label: None,
            policy_label: Some("clinical_concept".into()),
            score: Some(0.99),
            detector: "openmed".into(),
        });
        let GateOutcome::Approved {
            redacted_text,
            receipt,
        } = gate_document(&req, KEY).unwrap()
        else {
            panic!("expected approval")
        };
        assert_eq!(redacted_text, text);
        assert_eq!(receipt.findings[0].action, SpanAction::KeepLocal);
    }

    #[test]
    fn incomplete_coverage_blocks_release() {
        let mut req = request("patient@example.org");
        req.coverage.windows[0].start_byte = 1;
        let outcome = release_or_block(&req, KEY);
        assert!(
            matches!(outcome, GateOutcome::Blocked { code, .. } if code == "incomplete_coverage")
        );
    }

    #[test]
    fn artifact_drift_blocks_release() {
        let mut req = request("note");
        req.loaded_artifacts[0].sha256 = "c".repeat(64);
        assert!(matches!(
            release_or_block(&req, KEY),
            GateOutcome::Blocked { code, .. } if code == "artifact_mismatch"
        ));
    }

    #[test]
    fn artifact_paths_cannot_escape_the_model_root() {
        let mut req = request("note");
        req.artifact_lock.files[0].path = "../model.onnx".into();
        req.loaded_artifacts[0].path = "../model.onnx".into();
        assert!(matches!(
            release_or_block(&req, KEY),
            GateOutcome::Blocked { code, .. } if code == "invalid_artifact_lock"
        ));
    }

    #[test]
    fn unknown_labels_block_release() {
        let mut req = request("secret");
        req.spans.push(OpenMedSpan {
            start: 0,
            end: 6,
            offset_unit: OffsetUnit::Utf8Bytes,
            entity_type: "NEW_UPSTREAM_LABEL".into(),
            canonical_label: None,
            policy_label: None,
            score: Some(0.9),
            detector: "openmed".into(),
        });
        assert!(matches!(
            release_or_block(&req, KEY),
            GateOutcome::Blocked { code, .. } if code == "unknown_label"
        ));
    }

    #[test]
    fn overlapping_findings_redact_once() {
        let text = "Email ada@example.org now";
        let mut req = request(text);
        req.spans.push(OpenMedSpan {
            start: 6,
            end: 21,
            offset_unit: OffsetUnit::Utf8Bytes,
            entity_type: "EMAIL".into(),
            canonical_label: None,
            policy_label: Some("direct_identifier".into()),
            score: Some(0.95),
            detector: "openmed".into(),
        });
        let GateOutcome::Approved {
            redacted_text,
            receipt,
        } = gate_document(&req, KEY).unwrap()
        else {
            panic!("expected approval")
        };
        assert_eq!(redacted_text, "Email [REDACTED:EMAIL] now");
        assert_eq!(receipt.findings.len(), 1);
    }

    #[test]
    fn synthetic_canary_corpus_has_no_identifier_leakage() {
        for n in 0..1_000 {
            let email = format!("patient{n}@example.org");
            let text = format!("MRN: HX{n:06}; email {email}; medication metformin");
            let GateOutcome::Approved {
                redacted_text,
                receipt,
            } = gate_document(&request(&text), KEY).unwrap()
            else {
                panic!("expected approval")
            };
            assert!(!redacted_text.contains(&email));
            assert!(!redacted_text.contains(&format!("HX{n:06}")));
            assert!(!serde_json::to_string(&receipt).unwrap().contains(&email));
        }
    }
}
