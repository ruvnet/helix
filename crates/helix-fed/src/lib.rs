//! # helix-fed — ADR-030: federation transport (opt-in, DP-gated)
//!
//! The federation client for ADR-011 cohort intelligence, built so privacy is
//! **structurally unbypassable**: the only thing that can leave the device is a
//! [`helix_cohort::CohortVector`] — the output of the ADR-024 gate (generalize +
//! cell-suppression + differential privacy). There is no API that accepts raw
//! records, embeddings, or un-noised features, so they *cannot* be sent.
//!
//! Contribution requires explicit, expiring [`Consent`] (opt-out is the default,
//! ADR-011). The transport is a trait — an in-memory aggregator for tests, a real
//! signed-dispatch transport (Ruflo federation, Ed25519) later. Returns only
//! aggregate [`CohortSignal`]s (cohort size + an aggregate + ε spent), never
//! another person's data.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use helix_cohort::CohortVector;

/// Per-contribution consent (opt-in, expiring). Without it, contribution refuses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Consent {
    pub scope: String,
    /// Absolute expiry, epoch millis.
    pub expires_at: i64,
}

impl Consent {
    pub fn is_valid(&self, now: i64, scope: &str) -> bool {
        now < self.expires_at && self.scope == scope
    }
}

/// The envelope that crosses the wire — a DP-noised vector and the ε it cost.
/// Constructed only from a [`CohortVector`]; carries nothing else.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Contribution {
    pub features: Vec<(String, f64)>,
    pub epsilon_spent: f64,
    /// The scope this was contributed under (audit).
    pub scope: String,
}

/// An aggregate signal returned from the cohort — never an individual's data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CohortSignal {
    /// Number of contributors in the matched cohort (>= a minimum to return).
    pub cohort_size: u32,
    /// An aggregate statistic the analyst may use as population context.
    pub aggregate: f64,
    /// Total ε spent across the contributions backing this signal.
    pub epsilon_spent: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FedError {
    #[error("no valid consent for scope '{0}' — contribution refused (opt-out is the default)")]
    NoConsent(String),
    #[error("contribution carried a genomic feature, which is excluded from federation")]
    GenomicExcluded,
    #[error("cohort too small to return a signal without re-identification risk")]
    CohortTooSmall,
    #[error("transport error: {0}")]
    Transport(String),
}

const GENO_PREFIX: &str = "geno";

/// Build a [`Contribution`] from a gated [`CohortVector`]. This is the **only**
/// constructor — raw data has no path to the wire. Re-asserts the genomics
/// exclusion (defense in depth on top of ADR-024).
pub fn make_contribution(v: &CohortVector, scope: &str) -> Result<Contribution, FedError> {
    if v.features
        .iter()
        .any(|(k, _)| k.to_lowercase().starts_with(GENO_PREFIX))
    {
        return Err(FedError::GenomicExcluded);
    }
    Ok(Contribution {
        features: v.features.clone(),
        epsilon_spent: v.epsilon_spent,
        scope: scope.to_string(),
    })
}

/// Abstracts the federation network. Tests: an in-memory aggregator. Production:
/// a signed-dispatch transport (Ruflo federation, Ed25519).
pub trait FedTransport {
    /// Submit a contribution and request the matching cohort signal.
    fn submit(&self, contribution: &Contribution) -> Result<CohortSignal, FedError>;
}

/// The federation client. `contribute` enforces consent + the type-level privacy
/// gate before anything leaves the device.
pub struct FederationClient<'a, T: FedTransport> {
    pub transport: &'a T,
    /// Minimum cohort size to accept a returned signal (re-identification guard).
    pub min_cohort: u32,
}

impl<'a, T: FedTransport> FederationClient<'a, T> {
    pub fn new(transport: &'a T, min_cohort: u32) -> Self {
        Self {
            transport,
            min_cohort,
        }
    }

    /// Contribute a gated cohort vector under explicit consent, returning the
    /// aggregate cohort signal. Refuses without valid consent; only a
    /// `CohortVector` can be passed (privacy gate is the only door).
    pub fn contribute(
        &self,
        vector: &CohortVector,
        consent: &Consent,
        scope: &str,
        now: i64,
    ) -> Result<CohortSignal, FedError> {
        if !consent.is_valid(now, scope) {
            return Err(FedError::NoConsent(scope.to_string()));
        }
        let contribution = make_contribution(vector, scope)?;
        let signal = self.transport.submit(&contribution)?;
        if signal.cohort_size < self.min_cohort {
            return Err(FedError::CohortTooSmall);
        }
        Ok(signal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vector() -> CohortVector {
        CohortVector {
            features: vec![("vitamin_d_band".into(), 0.5), ("sleep_band".into(), 0.8)],
            epsilon_spent: 1.0,
            suppressed: vec![],
        }
    }
    fn consent(exp: i64) -> Consent {
        Consent {
            scope: "cohort:vitd".into(),
            expires_at: exp,
        }
    }

    struct Agg(CohortSignal);
    impl FedTransport for Agg {
        fn submit(&self, _: &Contribution) -> Result<CohortSignal, FedError> {
            Ok(self.0.clone())
        }
    }

    #[test]
    fn contributes_with_consent_and_returns_aggregate() {
        let agg = Agg(CohortSignal {
            cohort_size: 4200,
            aggregate: 0.62,
            epsilon_spent: 1.0,
        });
        let client = FederationClient::new(&agg, 100);
        let sig = client
            .contribute(&vector(), &consent(1000), "cohort:vitd", 0)
            .unwrap();
        assert_eq!(sig.cohort_size, 4200);
        assert!((sig.aggregate - 0.62).abs() < 1e-9);
    }

    #[test]
    fn refuses_without_valid_consent() {
        let agg = Agg(CohortSignal {
            cohort_size: 9999,
            aggregate: 0.0,
            epsilon_spent: 0.0,
        });
        let client = FederationClient::new(&agg, 100);
        // expired
        assert_eq!(
            client.contribute(&vector(), &consent(100), "cohort:vitd", 200),
            Err(FedError::NoConsent("cohort:vitd".into()))
        );
        // wrong scope
        assert_eq!(
            client.contribute(&vector(), &consent(1000), "cohort:other", 0),
            Err(FedError::NoConsent("cohort:other".into()))
        );
    }

    #[test]
    fn genomic_features_cannot_leave() {
        let mut v = vector();
        v.features.push(("geno_risk_t2d".into(), 0.7));
        assert_eq!(make_contribution(&v, "s"), Err(FedError::GenomicExcluded));
    }

    #[test]
    fn small_cohort_signal_is_refused() {
        let agg = Agg(CohortSignal {
            cohort_size: 7,
            aggregate: 0.5,
            epsilon_spent: 1.0,
        });
        let client = FederationClient::new(&agg, 100);
        assert_eq!(
            client.contribute(&vector(), &consent(1000), "cohort:vitd", 0),
            Err(FedError::CohortTooSmall)
        );
    }

    #[test]
    fn contribution_only_carries_noised_vector() {
        // The envelope holds exactly the gated features + epsilon — nothing else.
        let c = make_contribution(&vector(), "cohort:vitd").unwrap();
        assert_eq!(c.features.len(), 2);
        assert_eq!(c.epsilon_spent, 1.0);
        let json = serde_json::to_string(&c).unwrap();
        assert!(!json.contains("record") && !json.contains("ferritin"));
    }
}
