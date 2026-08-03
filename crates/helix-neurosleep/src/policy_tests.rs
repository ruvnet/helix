use helix_vault::SealKey;
use ruv_neural_core::attestation::{
    sign_neurosleep_bundle, PersistentEd25519Signer, SignedNeuroSleepBundleV1,
};
use ruv_neural_core::neurosleep::{
    compatibility_fingerprint_v1, FeatureValue, NeuroSleepPayloadV1, Species,
};

use super::*;

const FIXTURE: &[u8] = include_bytes!(
    "../../../../ruv-neural/ruv-neural-core/tests/fixtures/neurosleep-v1/valid_bundle.json"
);
const STUDY: &str = "constantino-method-fixture";
const SUBJECT: &str = "subject-random-001";
const SCOPE: &str = "local_neurosleep_research_v1";
const EXTRACTOR: &str = "2222222222222222222222222222222222222222222222222222222222222222";

struct Trust(SignerEnrollment);
impl NeuroSignerTrustStore for Trust {
    fn enrollment(&self, _: &str) -> Option<SignerEnrollment> {
        Some(self.0.clone())
    }
    fn is_revoked(&self, _: &str, _: i64) -> bool {
        false
    }
}

struct Consent;
impl ResearchConsentPolicy for Consent {
    fn permits(&self, request: &ConsentRequest<'_>) -> bool {
        request.study_id == STUDY
            && request.subject_pseudonym == SUBJECT
            && request.required_scope == SCOPE
    }
}

fn payload() -> NeuroSleepPayloadV1 {
    serde_json::from_slice::<SignedNeuroSleepBundleV1>(FIXTURE)
        .unwrap()
        .payload
}

fn context() -> IngestContext<'static> {
    IngestContext {
        study_id: STUDY,
        subject_pseudonym: SUBJECT,
        required_scope: SCOPE,
        verified_at_ms: 1_800_000_000_000,
    }
}

fn policy() -> AdmissionPolicy {
    let payload = payload();
    let mut policy = AdmissionPolicy::new(vec![Species::Mouse], [EXTRACTOR.into()]);
    policy.allowed_citations.insert(
        "PMID:42252510".into(),
        InterpretationMaturity::PreclinicalMouseModel,
    );
    policy.compatibility_profiles.insert(
        payload.compatibility_fingerprint.clone(),
        CompatibilityBinding::from_payload(&payload).unwrap(),
    );
    policy
}

fn partition() -> SealedStudyPartition {
    SealedStudyPartition::new(STUDY, SUBJECT, SealKey::from_bytes([7; 32]), [9; 32])
}

fn refusal_code(disposition: IngestDisposition) -> VerificationCode {
    match disposition {
        IngestDisposition::Rejected(report) | IngestDisposition::Quarantined(report) => report.code,
        IngestDisposition::Accepted(_) => panic!("expected policy refusal"),
    }
}

fn verify_signed(
    payload: NeuroSleepPayloadV1,
    signer: &PersistentEd25519Signer,
    trust: &Trust,
    policy: &AdmissionPolicy,
) -> VerificationCode {
    let bundle = sign_neurosleep_bundle(payload, signer).unwrap();
    refusal_code(verify_and_store(
        &serde_json::to_vec(&bundle).unwrap(),
        trust,
        &Consent,
        policy,
        context(),
        &mut partition(),
    ))
}

#[test]
fn human_validated_self_assertion_and_future_night_are_quarantined() {
    let signer = PersistentEd25519Signer::from_bytes("study-policy", &[6; 32]).unwrap();
    let trust = Trust(SignerEnrollment {
        verifying_key_ed25519: signer.verifying_key_bytes(),
        study_id: STUDY.into(),
        subject_pseudonym: SUBJECT.into(),
    });
    let mut human = payload();
    human.species = Species::Human;
    human.literature_context[0].evidence_maturity = "human_validated".into();
    let mut human_policy = policy();
    human_policy.allowed_species = vec![Species::Human];
    human_policy.allowed_citations.insert(
        "PMID:42252510".into(),
        InterpretationMaturity::HumanValidated,
    );
    assert_eq!(
        verify_signed(human, &signer, &trust, &human_policy),
        VerificationCode::InterpretationMaturityNotAllowed
    );

    let mut future = payload();
    future.night_end_ms = context().verified_at_ms + 300_001;
    assert_eq!(
        verify_signed(future, &signer, &trust, &policy()),
        VerificationCode::ImplausibleTimestamp
    );
}

#[test]
fn every_metric_domain_class_is_enforced() {
    let signer = PersistentEd25519Signer::from_bytes("study-domains", &[8; 32]).unwrap();
    let trust = Trust(SignerEnrollment {
        verifying_key_ed25519: signer.verifying_key_bytes(),
        study_id: STUDY.into(),
        subject_pseudonym: SUBJECT.into(),
    });
    let mut invalid = Vec::new();

    let mut negative_duration = payload();
    let FeatureValue::Observed { value, .. } = &mut negative_duration.stage_summary.nrem_duration
    else {
        unreachable!()
    };
    *value = -1.0;
    invalid.push(negative_duration);

    let mut fractional_count = payload();
    let FeatureValue::Observed { value, .. } = &mut fractional_count.stage_summary.rem_bout_count
    else {
        unreachable!()
    };
    *value = 1.5;
    invalid.push(fractional_count);

    let mut bad_ratio = payload();
    let FeatureValue::Observed { value, .. } =
        &mut bad_ratio.qeeg_by_stage[0].frontal_parietal_theta_coherence
    else {
        unreachable!()
    };
    *value = 1.1;
    invalid.push(bad_ratio);

    let mut non_theta_peak = payload();
    let FeatureValue::Observed { value, .. } =
        &mut non_theta_peak.qeeg_by_stage[0].theta_peak_frequency
    else {
        unreachable!()
    };
    *value = 9.0;
    invalid.push(non_theta_peak);

    for payload in invalid {
        assert_eq!(
            verify_signed(payload, &signer, &trust, &policy()),
            VerificationCode::MetricDomainInvalid
        );
    }
}

#[test]
fn reviewed_compatibility_projection_detects_method_drift() {
    let signer = PersistentEd25519Signer::from_bytes("study-method", &[10; 32]).unwrap();
    let trust = Trust(SignerEnrollment {
        verifying_key_ed25519: signer.verifying_key_bytes(),
        study_id: STUDY.into(),
        subject_pseudonym: SUBJECT.into(),
    });
    let mut changed = payload();
    changed.acquisition.sampling_rate_hz = 256.0;
    assert!(sign_neurosleep_bundle(changed.clone(), &signer).is_err());
    changed.compatibility_fingerprint =
        compatibility_fingerprint_v1(&changed.acquisition, &changed.algorithm).unwrap();
    assert_eq!(
        verify_signed(changed.clone(), &signer, &trust, &policy()),
        VerificationCode::CompatibilityProfileNotAllowed
    );

    // Helix re-derives the acquisition and algorithm binding itself rather than
    // trusting the fingerprint the signer computed, so an enrolled profile that
    // no longer pins the method it claims to pin fails closed.
    let mut mis_bound = policy();
    mis_bound.compatibility_profiles.insert(
        payload().compatibility_fingerprint,
        CompatibilityBinding::from_payload(&changed).unwrap(),
    );
    assert_eq!(
        verify_signed(payload(), &signer, &trust, &mis_bound),
        VerificationCode::CompatibilityProfileMismatch
    );
}
