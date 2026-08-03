use ruv_neural_core::attestation::{verify_neurosleep_bundle, SignedNeuroSleepBundleV1};
use ruv_neural_core::neurosleep::{
    FeatureValue, NeuroSleepPayloadV1, SleepState, Species, StateQeegFeatures, NEUROSLEEP_SCHEMA_V1,
};

use crate::contract::MAX_BUNDLE_BYTES;
use crate::storage::{replay_key, NONCE_DOMAIN, RECORDING_DOMAIN};
use crate::{
    AcquisitionConfidence, AdmissionPolicy, ConsentRequest, IngestContext, IngestDisposition,
    InterpretationMaturity, MetricId, NeuroSignerTrustStore, NeuroSleepIngest,
    NeuroSleepScalarRecord, ResearchConsentPolicy, ScalarValue, SealedStudyPartition,
    VerificationCode, VerificationReport,
};

pub fn verify_and_store(
    bytes: &[u8],
    trust: &impl NeuroSignerTrustStore,
    consent: &impl ResearchConsentPolicy,
    policy: &AdmissionPolicy,
    context: IngestContext<'_>,
    partition: &mut SealedStudyPartition,
) -> IngestDisposition {
    if bytes.len() > policy.maximum_bundle_bytes.min(MAX_BUNDLE_BYTES) {
        return rejected(VerificationCode::BundleTooLarge);
    }
    let bundle: SignedNeuroSleepBundleV1 = match serde_json::from_slice(bytes) {
        Ok(bundle) => bundle,
        Err(_) => return rejected(VerificationCode::MalformedBundle),
    };
    if bundle.schema != NEUROSLEEP_SCHEMA_V1 {
        return quarantined(VerificationCode::UnsupportedSchema);
    }
    let Some(enrollment) = trust.enrollment(&bundle.signer_key_id) else {
        return rejected(VerificationCode::UntrustedSigner);
    };
    if trust.is_revoked(&bundle.signer_key_id, context.verified_at_ms) {
        return rejected(VerificationCode::RevokedSigner);
    }
    // Authenticate every signed field before applying any payload policy or
    // emitting a payload-specific failure reason.
    if verify_neurosleep_bundle(&bundle, &enrollment.verifying_key_ed25519).is_err()
        || bundle.payload.payload_sha256().ok() != Some(bundle.payload_sha256)
    {
        return rejected(VerificationCode::BundleIntegrityFailed);
    }

    let payload = &bundle.payload;
    if enrollment.study_id != payload.study_id
        || enrollment.subject_pseudonym != payload.subject_pseudonym
    {
        return rejected(VerificationCode::SignerBindingMismatch);
    }
    if payload.study_id != context.study_id
        || payload.subject_pseudonym != context.subject_pseudonym
        || !partition.ensure_participant(context.study_id, context.subject_pseudonym)
    {
        return rejected(VerificationCode::ContextBindingMismatch);
    }
    if !payload
        .consent_scope
        .iter()
        .any(|s| s == context.required_scope)
        || !consent.permits(&ConsentRequest {
            study_id: &payload.study_id,
            subject_pseudonym: &payload.subject_pseudonym,
            signed_scopes: &payload.consent_scope,
            required_scope: context.required_scope,
            verified_at_ms: context.verified_at_ms,
        })
    {
        return rejected(VerificationCode::ConsentDenied);
    }
    if !payload.quality.accepted || !payload.quality.reason_codes.is_empty() {
        return quarantined(VerificationCode::QualityRejected);
    }
    let latest_plausible_end = context
        .verified_at_ms
        .checked_add(policy.maximum_future_skew_ms.max(0))
        .unwrap_or(i64::MAX);
    if payload.night_end_ms > latest_plausible_end {
        return quarantined(VerificationCode::ImplausibleTimestamp);
    }
    if !policy.allowed_species.contains(&payload.species) {
        return quarantined(VerificationCode::SpeciesNotAllowed);
    }
    if !policy
        .allowed_extractor_sha256
        .contains(&payload.algorithm.extractor_sha256)
    {
        return quarantined(VerificationCode::ExtractorNotAllowed);
    }
    let Some(expected_profile) = policy
        .compatibility_profiles
        .get(&payload.compatibility_fingerprint)
    else {
        return quarantined(VerificationCode::CompatibilityProfileNotAllowed);
    };
    let actual_profile = match crate::CompatibilityBinding::from_payload(payload) {
        Ok(profile) => profile,
        Err(_) => return quarantined(VerificationCode::CompatibilityProfileMismatch),
    };
    if &actual_profile != expected_profile {
        return quarantined(VerificationCode::CompatibilityProfileMismatch);
    }

    let maturity = match interpretation_maturity(payload, policy) {
        Ok(value) => value,
        Err(code) => return quarantined(code),
    };
    let confidence = AcquisitionConfidence::from_payload(payload);
    let scalars = match map_scalars(payload, bundle.payload_sha256, confidence, maturity) {
        Ok(records) => records,
        Err(code) => return quarantined(code),
    };
    let mut receipt = NeuroSleepIngest {
        payload_sha256: bundle.payload_sha256,
        opaque_partition_key: partition.opaque_partition_key(&bundle.payload_sha256),
        compatibility_fingerprint: payload.compatibility_fingerprint.clone(),
        scalar_record_count: scalars.len(),
        acquisition_confidence: confidence,
        interpretation_maturity: maturity,
        idempotent_reimport: false,
        verified_night: crate::VerifiedNeuroSleepNight {
            night_start_ms: payload.night_start_ms,
            compatibility_fingerprint: payload.compatibility_fingerprint.clone(),
            nrem_theta_coherence: nrem_theta_coherence(payload),
            acquisition_confidence: confidence,
            interpretation_maturity: maturity,
        },
    };
    if partition.by_digest.contains(&bundle.payload_sha256) {
        receipt.idempotent_reimport = true;
        return IngestDisposition::Accepted(receipt);
    }

    let recording_key = replay_key(
        &partition.opaque_index_key,
        RECORDING_DOMAIN,
        payload,
        &payload.recording_id,
    );
    let nonce_key = replay_key(
        &partition.opaque_index_key,
        NONCE_DOMAIN,
        payload,
        &payload.nonce,
    );
    if partition.by_recording.contains_key(&recording_key)
        || partition.by_nonce.contains_key(&nonce_key)
    {
        return rejected(VerificationCode::ReplayConflict);
    }
    if partition
        .commit(bytes, payload, &scalars, &receipt)
        .is_err()
    {
        return rejected(VerificationCode::StorageFailed);
    }
    IngestDisposition::Accepted(receipt)
}

fn nrem_theta_coherence(payload: &NeuroSleepPayloadV1) -> Option<f64> {
    let features = payload
        .qeeg_by_stage
        .iter()
        .find(|features| features.state == SleepState::Nrem)?;
    match &features.frontal_parietal_theta_coherence {
        FeatureValue::Observed { value, unit } if unit == "ratio" => Some(*value),
        _ => None,
    }
}

fn interpretation_maturity(
    payload: &NeuroSleepPayloadV1,
    policy: &AdmissionPolicy,
) -> Result<InterpretationMaturity, VerificationCode> {
    let mut maturity = InterpretationMaturity::HumanValidated;
    for citation in &payload.literature_context {
        let asserted = InterpretationMaturity::parse(&citation.evidence_maturity)
            .ok_or(VerificationCode::InterpretationMaturityNotAllowed)?;
        let qualified = policy
            .allowed_citations
            .get(&citation.identifier)
            .ok_or(VerificationCode::CitationNotAllowed)?;
        if asserted != *qualified {
            return Err(VerificationCode::CitationNotAllowed);
        }
        maturity = maturity.min(asserted);
    }
    if payload.literature_context.is_empty() {
        maturity = InterpretationMaturity::Hypothesis;
    }
    if payload.species == Species::Mouse
        && maturity != InterpretationMaturity::PreclinicalMouseModel
    {
        return Err(VerificationCode::InterpretationMaturityNotAllowed);
    }
    if !policy.allowed_interpretation_maturities.contains(&maturity) {
        return Err(VerificationCode::InterpretationMaturityNotAllowed);
    }
    Ok(maturity)
}

pub(crate) fn map_scalars(
    payload: &NeuroSleepPayloadV1,
    digest: [u8; 32],
    confidence: AcquisitionConfidence,
    maturity: InterpretationMaturity,
) -> Result<Vec<NeuroSleepScalarRecord>, VerificationCode> {
    let mut out = Vec::new();
    let summary = &payload.stage_summary;
    for (metric, value) in [
        (MetricId::WakeDuration, &summary.wake_duration),
        (MetricId::NremDuration, &summary.nrem_duration),
        (
            MetricId::NremMeanBoutDuration,
            &summary.nrem_mean_bout_duration,
        ),
        (MetricId::RemDuration, &summary.rem_duration),
        (MetricId::RemBoutCount, &summary.rem_bout_count),
    ] {
        out.push(scalar(
            payload, digest, confidence, maturity, metric, None, value,
        )?);
    }
    for features in &payload.qeeg_by_stage {
        append_stage_scalars(&mut out, payload, digest, confidence, maturity, features)?;
    }
    out.push(NeuroSleepScalarRecord {
        metric: MetricId::ArtifactBurden,
        stage: None,
        value: ScalarValue::Observed {
            value: payload.quality.artifact_fraction,
        },
        unit: MetricId::ArtifactBurden.unit().to_string(),
        night_start_ms: payload.night_start_ms,
        compatibility_fingerprint: payload.compatibility_fingerprint.clone(),
        acquisition_confidence: confidence,
        interpretation_maturity: maturity,
        source_payload_sha256: digest,
    });
    Ok(out)
}

fn append_stage_scalars(
    out: &mut Vec<NeuroSleepScalarRecord>,
    payload: &NeuroSleepPayloadV1,
    digest: [u8; 32],
    confidence: AcquisitionConfidence,
    maturity: InterpretationMaturity,
    f: &StateQeegFeatures,
) -> Result<(), VerificationCode> {
    for (metric, value) in [
        (MetricId::DeltaAbsolutePower, &f.delta_absolute_power),
        (MetricId::DeltaRelativePower, &f.delta_relative_power),
        (MetricId::ThetaAbsolutePower, &f.theta_absolute_power),
        (MetricId::ThetaRelativePower, &f.theta_relative_power),
        (MetricId::AlphaAbsolutePower, &f.alpha_absolute_power),
        (MetricId::ThetaPeakFrequency, &f.theta_peak_frequency),
        (MetricId::ThetaPeakPower, &f.theta_peak_power),
        (
            MetricId::FrontalParietalFullBandCoherence,
            &f.frontal_parietal_full_band_coherence,
        ),
        (
            MetricId::FrontalParietalThetaCoherence,
            &f.frontal_parietal_theta_coherence,
        ),
        (MetricId::AperiodicExponent, &f.aperiodic_exponent),
        (MetricId::AperiodicOffset, &f.aperiodic_offset),
        (MetricId::SpectralFitError, &f.spectral_fit_error),
    ] {
        out.push(scalar(
            payload,
            digest,
            confidence,
            maturity,
            metric,
            Some(f.state),
            value,
        )?);
    }
    Ok(())
}

fn scalar(
    payload: &NeuroSleepPayloadV1,
    digest: [u8; 32],
    confidence: AcquisitionConfidence,
    maturity: InterpretationMaturity,
    metric: MetricId,
    stage: Option<SleepState>,
    feature: &FeatureValue,
) -> Result<NeuroSleepScalarRecord, VerificationCode> {
    let value = match feature {
        FeatureValue::Observed { value, unit } => {
            if unit != metric.unit() {
                return Err(VerificationCode::UnitNotAllowed);
            }
            if !valid_domain(metric, *value) {
                return Err(VerificationCode::MetricDomainInvalid);
            }
            ScalarValue::Observed { value: *value }
        }
        FeatureValue::Null { reason } => ScalarValue::Null { reason: *reason },
    };
    Ok(NeuroSleepScalarRecord {
        metric,
        stage,
        value,
        unit: metric.unit().to_string(),
        night_start_ms: payload.night_start_ms,
        compatibility_fingerprint: payload.compatibility_fingerprint.clone(),
        acquisition_confidence: confidence,
        interpretation_maturity: maturity,
        source_payload_sha256: digest,
    })
}

fn valid_domain(metric: MetricId, value: f64) -> bool {
    if !value.is_finite() {
        return false;
    }
    match metric {
        MetricId::WakeDuration
        | MetricId::NremDuration
        | MetricId::NremMeanBoutDuration
        | MetricId::RemDuration
        | MetricId::DeltaAbsolutePower
        | MetricId::ThetaAbsolutePower
        | MetricId::AlphaAbsolutePower
        | MetricId::AperiodicExponent
        | MetricId::SpectralFitError => value >= 0.0,
        MetricId::RemBoutCount => value >= 0.0 && value.fract() == 0.0,
        MetricId::DeltaRelativePower
        | MetricId::ThetaRelativePower
        | MetricId::FrontalParietalFullBandCoherence
        | MetricId::FrontalParietalThetaCoherence
        | MetricId::ArtifactBurden => (0.0..=1.0).contains(&value),
        MetricId::ThetaPeakFrequency => (4.0..=8.0).contains(&value),
        MetricId::ThetaPeakPower | MetricId::AperiodicOffset => true,
    }
}

fn rejected(code: VerificationCode) -> IngestDisposition {
    IngestDisposition::Rejected(VerificationReport { code })
}

fn quarantined(code: VerificationCode) -> IngestDisposition {
    IngestDisposition::Quarantined(VerificationReport { code })
}
