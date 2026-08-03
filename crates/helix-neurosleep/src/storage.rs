use std::collections::{BTreeMap, BTreeSet};

use helix_vault::{RecordId, SealKey, UserKeyring, VaultStore};
use hmac::{Hmac, Mac};
use ruv_neural_core::neurosleep::NeuroSleepPayloadV1;
use sha2::Sha256;
use thiserror::Error;

use crate::{NeuroSleepIngest, NeuroSleepScalarRecord};

const ENVELOPE_DOMAIN: &[u8] = b"helix/neurosleep/envelope/v1\0";
const SCALAR_DOMAIN: &[u8] = b"helix/neurosleep/scalar/v1\0";
pub(crate) const RECORDING_DOMAIN: &[u8] = b"helix/neurosleep/recording/v1\0";
pub(crate) const NONCE_DOMAIN: &[u8] = b"helix/neurosleep/nonce/v1\0";
const PARTITION_DOMAIN: &[u8] = b"helix/neurosleep/partition/v1\0";

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(crate) enum CommitError {
    #[error("sealed research partition commit failed")]
    Failed,
}

pub struct SealedStudyPartition {
    keyring: UserKeyring,
    pub(crate) opaque_index_key: [u8; 32],
    participant_binding: [u8; 32],
    store: VaultStore,
    pub(crate) by_digest: BTreeSet<[u8; 32]>,
    pub(crate) by_recording: BTreeMap<[u8; 32], [u8; 32]>,
    pub(crate) by_nonce: BTreeMap<[u8; 32], [u8; 32]>,
}

impl SealedStudyPartition {
    pub fn new(
        study_id: &str,
        subject_pseudonym: &str,
        seal_key: SealKey,
        opaque_index_key: [u8; 32],
    ) -> Self {
        Self {
            keyring: UserKeyring::new(seal_key),
            opaque_index_key,
            participant_binding: opaque_hmac(
                &opaque_index_key,
                PARTITION_DOMAIN,
                &[study_id, subject_pseudonym],
            ),
            store: VaultStore::new(),
            by_digest: BTreeSet::new(),
            by_recording: BTreeMap::new(),
            by_nonce: BTreeMap::new(),
        }
    }

    pub fn sealed_record_count(&self) -> usize {
        self.store.len()
    }

    pub fn sealed_dump_for_audit(&self) -> String {
        serde_json::to_string(&self.store).expect("VaultStore serialization is infallible")
    }

    pub(crate) fn ensure_participant(&self, study_id: &str, subject_pseudonym: &str) -> bool {
        self.participant_binding
            == opaque_hmac(
                &self.opaque_index_key,
                PARTITION_DOMAIN,
                &[study_id, subject_pseudonym],
            )
    }

    pub(crate) fn opaque_partition_key(&self, digest: &[u8; 32]) -> String {
        hex_string(&opaque_hmac(
            &self.opaque_index_key,
            PARTITION_DOMAIN,
            &[&hex_string(digest)],
        ))
    }

    pub(crate) fn commit(
        &mut self,
        bundle_bytes: &[u8],
        payload: &NeuroSleepPayloadV1,
        scalars: &[NeuroSleepScalarRecord],
        receipt: &NeuroSleepIngest,
    ) -> Result<(), CommitError> {
        self.commit_with(
            bundle_bytes,
            payload,
            scalars,
            receipt,
            |keyring, store, id, bytes| {
                keyring
                    .seal_into(store, id, bytes)
                    .map_err(|_| CommitError::Failed)
            },
        )
    }

    pub(crate) fn commit_with<F>(
        &mut self,
        bundle_bytes: &[u8],
        payload: &NeuroSleepPayloadV1,
        scalars: &[NeuroSleepScalarRecord],
        receipt: &NeuroSleepIngest,
        mut seal_one: F,
    ) -> Result<(), CommitError>
    where
        F: FnMut(&UserKeyring, &mut VaultStore, RecordId, &[u8]) -> Result<(), CommitError>,
    {
        let mut staged_store = self.store.clone();
        let digest_hex = hex_string(&receipt.payload_sha256);
        let envelope_key = opaque_hmac(&self.opaque_index_key, ENVELOPE_DOMAIN, &[&digest_hex]);
        seal_one(
            &self.keyring,
            &mut staged_store,
            RecordId(hex_string(&envelope_key)),
            bundle_bytes,
        )?;
        for (index, scalar) in scalars.iter().enumerate() {
            let index_text = index.to_string();
            let key = opaque_hmac(
                &self.opaque_index_key,
                SCALAR_DOMAIN,
                &[&digest_hex, &index_text],
            );
            let encoded = serde_json::to_vec(scalar).map_err(|_| CommitError::Failed)?;
            seal_one(
                &self.keyring,
                &mut staged_store,
                RecordId(hex_string(&key)),
                &encoded,
            )?;
        }

        let recording_key = replay_key(
            &self.opaque_index_key,
            RECORDING_DOMAIN,
            payload,
            &payload.recording_id,
        );
        let nonce_key = replay_key(
            &self.opaque_index_key,
            NONCE_DOMAIN,
            payload,
            &payload.nonce,
        );
        self.store = staged_store;
        self.by_recording
            .insert(recording_key, receipt.payload_sha256);
        self.by_nonce.insert(nonce_key, receipt.payload_sha256);
        self.by_digest.insert(receipt.payload_sha256);
        Ok(())
    }
}

pub(crate) fn replay_key(
    key: &[u8; 32],
    domain: &[u8],
    payload: &NeuroSleepPayloadV1,
    value: &str,
) -> [u8; 32] {
    opaque_hmac(
        key,
        domain,
        &[&payload.study_id, &payload.subject_pseudonym, value],
    )
}

fn opaque_hmac(key: &[u8; 32], domain: &[u8], parts: &[&str]) -> [u8; 32] {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC accepts any key size");
    mac.update(domain);
    for part in parts {
        mac.update(&(part.len() as u64).to_be_bytes());
        mac.update(part.as_bytes());
    }
    mac.finalize().into_bytes().into()
}

fn hex_string(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use ruv_neural_core::attestation::SignedNeuroSleepBundleV1;

    use super::*;
    use crate::ingest::map_scalars;
    use crate::{AcquisitionConfidence, InterpretationMaturity};

    const FIXTURE: &[u8] = include_bytes!(
        "../../../../ruv-neural/ruv-neural-core/tests/fixtures/neurosleep-v1/valid_bundle.json"
    );

    #[test]
    fn staged_failure_has_no_partial_visibility_or_replay_state() {
        let bundle: SignedNeuroSleepBundleV1 = serde_json::from_slice(FIXTURE).unwrap();
        let confidence = AcquisitionConfidence::from_payload(&bundle.payload);
        let scalars = map_scalars(
            &bundle.payload,
            bundle.payload_sha256,
            confidence,
            InterpretationMaturity::PreclinicalMouseModel,
        )
        .unwrap();
        let receipt = NeuroSleepIngest {
            payload_sha256: bundle.payload_sha256,
            opaque_partition_key: "opaque".into(),
            compatibility_fingerprint: bundle.payload.compatibility_fingerprint.clone(),
            scalar_record_count: scalars.len(),
            acquisition_confidence: confidence,
            interpretation_maturity: InterpretationMaturity::PreclinicalMouseModel,
            idempotent_reimport: false,
        };
        let mut store = SealedStudyPartition::new(
            &bundle.payload.study_id,
            &bundle.payload.subject_pseudonym,
            SealKey::from_bytes([7; 32]),
            [9; 32],
        );
        let mut calls = 0;
        let result = store.commit_with(
            FIXTURE,
            &bundle.payload,
            &scalars,
            &receipt,
            |keyring, staged, id, bytes| {
                calls += 1;
                if calls == 2 {
                    return Err(CommitError::Failed);
                }
                keyring
                    .seal_into(staged, id, bytes)
                    .map_err(|_| CommitError::Failed)
            },
        );
        assert_eq!(result, Err(CommitError::Failed));
        assert_eq!(store.sealed_record_count(), 0);
        assert!(store.by_digest.is_empty());
        assert!(store.by_recording.is_empty());
        assert!(store.by_nonce.is_empty());
    }
}
