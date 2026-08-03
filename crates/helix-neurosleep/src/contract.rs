use std::collections::{BTreeMap, BTreeSet};

use ruv_neural_core::neurosleep::{NeuroSleepPayloadV1, NullReason, SleepState, Species};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub(crate) const MAX_BUNDLE_BYTES: usize = 1_048_576;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignerEnrollment {
    pub verifying_key_ed25519: [u8; 32],
    pub study_id: String,
    pub subject_pseudonym: String,
}

pub trait NeuroSignerTrustStore {
    fn enrollment(&self, key_id: &str) -> Option<SignerEnrollment>;
    fn is_revoked(&self, key_id: &str, at_ms: i64) -> bool;
}

pub struct ConsentRequest<'a> {
    pub study_id: &'a str,
    pub subject_pseudonym: &'a str,
    pub signed_scopes: &'a [String],
    pub required_scope: &'a str,
    pub verified_at_ms: i64,
}

pub trait ResearchConsentPolicy {
    fn permits(&self, request: &ConsentRequest<'_>) -> bool;
}

#[derive(Debug, Clone, Copy)]
pub struct IngestContext<'a> {
    pub study_id: &'a str,
    pub subject_pseudonym: &'a str,
    pub required_scope: &'a str,
    pub verified_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct AdmissionPolicy {
    pub allowed_species: Vec<Species>,
    pub allowed_extractor_sha256: BTreeSet<String>,
    pub maximum_bundle_bytes: usize,
    pub maximum_future_skew_ms: i64,
    pub allowed_interpretation_maturities: BTreeSet<InterpretationMaturity>,
    pub allowed_citations: BTreeMap<String, InterpretationMaturity>,
    pub compatibility_profiles: BTreeMap<String, CompatibilityBinding>,
}

impl AdmissionPolicy {
    pub fn new(
        allowed_species: Vec<Species>,
        allowed_extractor_sha256: impl IntoIterator<Item = String>,
    ) -> Self {
        Self {
            allowed_species,
            allowed_extractor_sha256: allowed_extractor_sha256.into_iter().collect(),
            maximum_bundle_bytes: MAX_BUNDLE_BYTES,
            maximum_future_skew_ms: 300_000,
            allowed_interpretation_maturities: [
                InterpretationMaturity::Hypothesis,
                InterpretationMaturity::PreclinicalMouseModel,
            ]
            .into_iter()
            .collect(),
            allowed_citations: BTreeMap::new(),
            compatibility_profiles: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompatibilityBinding {
    pub acquisition_sha256: [u8; 32],
    pub algorithm_sha256: [u8; 32],
}

impl CompatibilityBinding {
    pub fn from_payload(payload: &NeuroSleepPayloadV1) -> Result<Self, String> {
        let acquisition = serde_json_canonicalizer::to_vec(&payload.acquisition)
            .map_err(|error| error.to_string())?;
        let algorithm = serde_json_canonicalizer::to_vec(&payload.algorithm)
            .map_err(|error| error.to_string())?;
        Ok(Self {
            acquisition_sha256: Sha256::digest(acquisition).into(),
            algorithm_sha256: Sha256::digest(algorithm).into(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AcquisitionConfidence(f64);

impl AcquisitionConfidence {
    pub(crate) fn from_payload(payload: &NeuroSleepPayloadV1) -> Self {
        Self(
            (payload.quality.valid_coverage_fraction * (1.0 - payload.quality.artifact_fraction))
                .clamp(0.0, 1.0),
        )
    }

    pub fn get(self) -> f64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterpretationMaturity {
    Hypothesis,
    PreclinicalMouseModel,
    HumanObservational,
    HumanValidated,
}

impl InterpretationMaturity {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "hypothesis" => Some(Self::Hypothesis),
            "preclinical_mouse_model" => Some(Self::PreclinicalMouseModel),
            "human_observational" => Some(Self::HumanObservational),
            "human_validated" => Some(Self::HumanValidated),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricId {
    WakeDuration,
    NremDuration,
    NremMeanBoutDuration,
    RemDuration,
    RemBoutCount,
    DeltaAbsolutePower,
    DeltaRelativePower,
    ThetaAbsolutePower,
    ThetaRelativePower,
    AlphaAbsolutePower,
    ThetaPeakFrequency,
    ThetaPeakPower,
    FrontalParietalFullBandCoherence,
    FrontalParietalThetaCoherence,
    AperiodicExponent,
    AperiodicOffset,
    SpectralFitError,
    ArtifactBurden,
}

impl MetricId {
    pub const fn unit(self) -> &'static str {
        match self {
            Self::WakeDuration
            | Self::NremDuration
            | Self::NremMeanBoutDuration
            | Self::RemDuration => "s",
            Self::RemBoutCount => "count",
            // Keep the exact authoritative V1 contract unit until rUv Neural
            // coordinates any PSD-vs-integrated-band correction and schema bump.
            Self::DeltaAbsolutePower | Self::ThetaAbsolutePower | Self::AlphaAbsolutePower => {
                "uV2_per_hz"
            }
            Self::DeltaRelativePower
            | Self::ThetaRelativePower
            | Self::FrontalParietalFullBandCoherence
            | Self::FrontalParietalThetaCoherence
            | Self::ArtifactBurden => "ratio",
            Self::ThetaPeakFrequency => "Hz",
            Self::ThetaPeakPower | Self::AperiodicOffset => "log10_uV2_per_hz",
            Self::AperiodicExponent => "dimensionless",
            Self::SpectralFitError => "log10_power",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ScalarValue {
    Observed { value: f64 },
    Null { reason: NullReason },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NeuroSleepScalarRecord {
    pub metric: MetricId,
    pub stage: Option<SleepState>,
    pub value: ScalarValue,
    pub unit: String,
    pub night_start_ms: i64,
    pub compatibility_fingerprint: String,
    pub acquisition_confidence: AcquisitionConfidence,
    pub interpretation_maturity: InterpretationMaturity,
    pub source_payload_sha256: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NeuroSleepIngest {
    pub payload_sha256: [u8; 32],
    pub opaque_partition_key: String,
    pub compatibility_fingerprint: String,
    pub scalar_record_count: usize,
    pub acquisition_confidence: AcquisitionConfidence,
    pub interpretation_maturity: InterpretationMaturity,
    pub idempotent_reimport: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationCode {
    BundleTooLarge,
    MalformedBundle,
    UnsupportedSchema,
    BundleIntegrityFailed,
    UntrustedSigner,
    RevokedSigner,
    SignerBindingMismatch,
    ContextBindingMismatch,
    ConsentDenied,
    QualityRejected,
    SpeciesNotAllowed,
    ExtractorNotAllowed,
    UnitNotAllowed,
    InterpretationMaturityNotAllowed,
    CitationNotAllowed,
    CompatibilityProfileNotAllowed,
    CompatibilityProfileMismatch,
    ImplausibleTimestamp,
    MetricDomainInvalid,
    ReplayConflict,
    StorageFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationReport {
    pub code: VerificationCode,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "disposition", rename_all = "snake_case")]
pub enum IngestDisposition {
    Accepted(NeuroSleepIngest),
    Quarantined(VerificationReport),
    Rejected(VerificationReport),
}
