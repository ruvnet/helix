use helix_vault::SealKey;
use ruv_neural_core::attestation::{
    sign_neurosleep_bundle, PersistentEd25519Signer, SignedNeuroSleepBundleV1,
};
use ruv_neural_core::neurosleep::{FeatureValue, Species};

use super::*;

const FIXTURE: &[u8] = include_bytes!(
    "../../../../ruv-neural/ruv-neural-core/tests/fixtures/neurosleep-v1/valid_bundle.json"
);
const STUDY: &str = "constantino-method-fixture";
const SUBJECT: &str = "subject-random-001";
const SCOPE: &str = "local_neurosleep_research_v1";
const EXTRACTOR: &str = "2222222222222222222222222222222222222222222222222222222222222222";

struct Trust {
    enrollment: Option<SignerEnrollment>,
    revoked: bool,
}

impl NeuroSignerTrustStore for Trust {
    fn enrollment(&self, _: &str) -> Option<SignerEnrollment> {
        self.enrollment.clone()
    }

    fn is_revoked(&self, _: &str, _: i64) -> bool {
        self.revoked
    }
}

struct Consent(bool);
impl ResearchConsentPolicy for Consent {
    fn permits(&self, request: &ConsentRequest<'_>) -> bool {
        self.0
            && request.study_id == STUDY
            && request.subject_pseudonym == SUBJECT
            && request.required_scope == SCOPE
            && request.signed_scopes.iter().any(|scope| scope == SCOPE)
    }
}

fn fixture_bundle() -> SignedNeuroSleepBundleV1 {
    serde_json::from_slice(FIXTURE).unwrap()
}

fn fixture_key() -> [u8; 32] {
    let trust: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../ruv-neural/ruv-neural-core/tests/fixtures/neurosleep-v1/trust_profile.json"
    ))
    .unwrap();
    let bytes: Vec<u8> = serde_json::from_value(trust["verifying_key_ed25519"].clone()).unwrap();
    bytes.try_into().unwrap()
}

fn trust() -> Trust {
    Trust {
        enrollment: Some(SignerEnrollment {
            verifying_key_ed25519: fixture_key(),
            study_id: STUDY.to_string(),
            subject_pseudonym: SUBJECT.to_string(),
        }),
        revoked: false,
    }
}

fn policy() -> AdmissionPolicy {
    let payload = fixture_bundle().payload;
    let mut policy = AdmissionPolicy::new([Species::Mouse].to_vec(), [EXTRACTOR.to_string()]);
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

fn context() -> IngestContext<'static> {
    IngestContext {
        study_id: STUDY,
        subject_pseudonym: SUBJECT,
        required_scope: SCOPE,
        verified_at_ms: 1_800_000_000_000,
    }
}

fn partition() -> SealedStudyPartition {
    SealedStudyPartition::new(STUDY, SUBJECT, SealKey::from_bytes([7; 32]), [9; 32])
}

fn code(disposition: IngestDisposition) -> VerificationCode {
    match disposition {
        IngestDisposition::Rejected(r) | IngestDisposition::Quarantined(r) => r.code,
        IngestDisposition::Accepted(_) => panic!("expected refusal"),
    }
}

#[test]
fn accepts_authoritative_fixture_and_seals_envelope_and_scalars() {
    let mut store = partition();
    let accepted = verify_and_store(
        FIXTURE,
        &trust(),
        &Consent(true),
        &policy(),
        context(),
        &mut store,
    );
    let IngestDisposition::Accepted(receipt) = accepted else {
        panic!("fixture should be accepted: {accepted:?}");
    };
    assert_eq!(receipt.scalar_record_count, 18);
    assert_eq!(
        receipt.interpretation_maturity,
        InterpretationMaturity::PreclinicalMouseModel
    );
    assert!((receipt.acquisition_confidence.get() - 0.9025).abs() < 1e-12);
    assert_eq!(receipt.verified_night.nrem_theta_coherence, Some(0.7));
    let view_json = serde_json::to_string(&receipt.verified_night).unwrap();
    for forbidden in [
        "study_id",
        "subject_pseudonym",
        "recording_id",
        "nonce",
        "payload_sha256",
        "signer",
    ] {
        assert!(!view_json.contains(forbidden));
    }
    assert_eq!(store.sealed_record_count(), 19);
    let dump = store.sealed_dump_for_audit();
    for forbidden in [STUDY, SUBJECT, "recording-001", "theta", "2.5"] {
        assert!(!dump.contains(forbidden), "sealed dump leaked {forbidden}");
    }
}

#[test]
fn rejects_semantic_tamper_before_payload_policy_and_writes_nothing() {
    let tampered = String::from_utf8(FIXTURE.to_vec())
        .unwrap()
        .replace("\"value\":2.5", "\"value\":2.6");
    let mut store = partition();
    assert_eq!(
        code(verify_and_store(
            tampered.as_bytes(),
            &trust(),
            &Consent(true),
            &policy(),
            context(),
            &mut store,
        )),
        VerificationCode::BundleIntegrityFailed
    );

    let mut tampered_subject: serde_json::Value = serde_json::from_slice(FIXTURE).unwrap();
    tampered_subject["payload"]["subject_pseudonym"] = "another-subject".into();
    assert_eq!(
        code(verify_and_store(
            &serde_json::to_vec(&tampered_subject).unwrap(),
            &trust(),
            &Consent(true),
            &policy(),
            context(),
            &mut store,
        )),
        VerificationCode::BundleIntegrityFailed
    );
    assert_eq!(store.sealed_record_count(), 0);
}

#[test]
fn size_schema_and_context_boundaries_fail_closed() {
    let mut store = partition();
    let oversized = vec![b' '; 1_048_577];
    let mut permissive = policy();
    permissive.maximum_bundle_bytes = usize::MAX;
    assert_eq!(
        code(verify_and_store(
            &oversized,
            &trust(),
            &Consent(true),
            &permissive,
            context(),
            &mut store,
        )),
        VerificationCode::BundleTooLarge
    );

    let mut future: serde_json::Value = serde_json::from_slice(FIXTURE).unwrap();
    future["schema"] = "ruv-neural/neurosleep/2".into();
    assert_eq!(
        code(verify_and_store(
            &serde_json::to_vec(&future).unwrap(),
            &trust(),
            &Consent(true),
            &policy(),
            context(),
            &mut store,
        )),
        VerificationCode::UnsupportedSchema
    );

    let wrong_context = IngestContext {
        subject_pseudonym: "wrong-subject",
        ..context()
    };
    assert_eq!(
        code(verify_and_store(
            FIXTURE,
            &trust(),
            &Consent(true),
            &policy(),
            wrong_context,
            &mut store,
        )),
        VerificationCode::ContextBindingMismatch
    );
    let mut wrong_partition = SealedStudyPartition::new(
        "other-study",
        SUBJECT,
        SealKey::from_bytes([7; 32]),
        [9; 32],
    );
    assert_eq!(
        code(verify_and_store(
            FIXTURE,
            &trust(),
            &Consent(true),
            &policy(),
            context(),
            &mut wrong_partition,
        )),
        VerificationCode::ContextBindingMismatch
    );
    let mut wrong_subject_partition = SealedStudyPartition::new(
        STUDY,
        "another-subject",
        SealKey::from_bytes([7; 32]),
        [9; 32],
    );
    assert_eq!(
        code(verify_and_store(
            FIXTURE,
            &trust(),
            &Consent(true),
            &policy(),
            context(),
            &mut wrong_subject_partition,
        )),
        VerificationCode::ContextBindingMismatch
    );
    assert_eq!(store.sealed_record_count(), 0);
}

#[test]
fn trust_revocation_binding_and_consent_fail_closed() {
    let mut store = partition();
    let untrusted = Trust {
        enrollment: None,
        revoked: false,
    };
    assert_eq!(
        code(verify_and_store(
            FIXTURE,
            &untrusted,
            &Consent(true),
            &policy(),
            context(),
            &mut store
        )),
        VerificationCode::UntrustedSigner
    );
    let revoked = Trust {
        enrollment: trust().enrollment,
        revoked: true,
    };
    assert_eq!(
        code(verify_and_store(
            FIXTURE,
            &revoked,
            &Consent(true),
            &policy(),
            context(),
            &mut store
        )),
        VerificationCode::RevokedSigner
    );
    assert_eq!(
        code(verify_and_store(
            FIXTURE,
            &trust(),
            &Consent(false),
            &policy(),
            context(),
            &mut store
        )),
        VerificationCode::ConsentDenied
    );
    let wrong_binding = Trust {
        enrollment: Some(SignerEnrollment {
            subject_pseudonym: "another-subject".into(),
            ..trust().enrollment.unwrap()
        }),
        revoked: false,
    };
    assert_eq!(
        code(verify_and_store(
            FIXTURE,
            &wrong_binding,
            &Consent(true),
            &policy(),
            context(),
            &mut store
        )),
        VerificationCode::SignerBindingMismatch
    );
    assert_eq!(store.sealed_record_count(), 0);
}

#[test]
fn unknown_fields_species_extractor_quality_and_units_are_closed() {
    let mut store = partition();
    let mut unknown: serde_json::Value = serde_json::from_slice(FIXTURE).unwrap();
    unknown["payload"]["raw_samples"] = serde_json::json!([1, 2, 3]);
    assert_eq!(
        code(verify_and_store(
            &serde_json::to_vec(&unknown).unwrap(),
            &trust(),
            &Consent(true),
            &policy(),
            context(),
            &mut store
        )),
        VerificationCode::MalformedBundle
    );

    let human_policy = AdmissionPolicy::new([Species::Human].to_vec(), [EXTRACTOR.to_string()]);
    assert_eq!(
        code(verify_and_store(
            FIXTURE,
            &trust(),
            &Consent(true),
            &human_policy,
            context(),
            &mut store
        )),
        VerificationCode::SpeciesNotAllowed
    );
    let wrong_extractor = AdmissionPolicy::new([Species::Mouse].to_vec(), ["aa".repeat(32)]);
    assert_eq!(
        code(verify_and_store(
            FIXTURE,
            &trust(),
            &Consent(true),
            &wrong_extractor,
            context(),
            &mut store
        )),
        VerificationCode::ExtractorNotAllowed
    );

    let signer = PersistentEd25519Signer::from_bytes("study-key-2026-01", &[4; 32]).unwrap();
    let signed_trust = signer_trust(&signer);
    let mut payload = fixture_bundle().payload;
    payload.quality.accepted = false;
    payload.quality.reason_codes = vec!["artifact".into()];
    let rejected_quality = sign_neurosleep_bundle(payload, &signer).unwrap();
    assert_eq!(
        code(verify_and_store(
            &serde_json::to_vec(&rejected_quality).unwrap(),
            &signed_trust,
            &Consent(true),
            &policy(),
            context(),
            &mut store
        )),
        VerificationCode::QualityRejected
    );

    let mut payload = fixture_bundle().payload;
    let FeatureValue::Observed { unit, .. } = &mut payload.stage_summary.nrem_duration else {
        unreachable!()
    };
    *unit = "minutes".into();
    assert!(sign_neurosleep_bundle(payload, &signer).is_err());

    let mut payload = fixture_bundle().payload;
    payload.literature_context[0].evidence_maturity = "unknown_maturity".into();
    let unknown_maturity = sign_neurosleep_bundle(payload, &signer).unwrap();
    assert_eq!(
        code(verify_and_store(
            &serde_json::to_vec(&unknown_maturity).unwrap(),
            &signed_trust,
            &Consent(true),
            &policy(),
            context(),
            &mut store
        )),
        VerificationCode::InterpretationMaturityNotAllowed
    );
}

fn signer_trust(signer: &PersistentEd25519Signer) -> Trust {
    Trust {
        enrollment: Some(SignerEnrollment {
            verifying_key_ed25519: signer.verifying_key_bytes(),
            study_id: STUDY.into(),
            subject_pseudonym: SUBJECT.into(),
        }),
        revoked: false,
    }
}

#[test]
fn reimport_is_idempotent_and_nonce_or_recording_reuse_conflicts() {
    let signer = PersistentEd25519Signer::from_bytes("study-key", &[5; 32]).unwrap();
    let local_trust = signer_trust(&signer);
    let mut payload = fixture_bundle().payload;
    let first = sign_neurosleep_bundle(payload.clone(), &signer).unwrap();
    let first_bytes = serde_json::to_vec(&first).unwrap();
    let mut store = partition();
    let IngestDisposition::Accepted(initial) = verify_and_store(
        &first_bytes,
        &local_trust,
        &Consent(true),
        &policy(),
        context(),
        &mut store,
    ) else {
        panic!("initial import failed")
    };
    assert!(!initial.idempotent_reimport);
    let IngestDisposition::Accepted(reimport) = verify_and_store(
        &first_bytes,
        &local_trust,
        &Consent(true),
        &policy(),
        context(),
        &mut store,
    ) else {
        panic!("reimport failed")
    };
    assert!(reimport.idempotent_reimport);
    assert_eq!(store.sealed_record_count(), 19);

    payload.bundle_id = "different-recording-conflict".into();
    payload.nonce = "fresh-nonce".into();
    let conflict = sign_neurosleep_bundle(payload.clone(), &signer).unwrap();
    assert_eq!(
        code(verify_and_store(
            &serde_json::to_vec(&conflict).unwrap(),
            &local_trust,
            &Consent(true),
            &policy(),
            context(),
            &mut store
        )),
        VerificationCode::ReplayConflict
    );
    payload.bundle_id = "different-nonce-conflict".into();
    payload.recording_id = "fresh-recording".into();
    payload.nonce = fixture_bundle().payload.nonce;
    let conflict = sign_neurosleep_bundle(payload, &signer).unwrap();
    assert_eq!(
        code(verify_and_store(
            &serde_json::to_vec(&conflict).unwrap(),
            &local_trust,
            &Consent(true),
            &policy(),
            context(),
            &mut store
        )),
        VerificationCode::ReplayConflict
    );
    assert_eq!(store.sealed_record_count(), 19);
}
