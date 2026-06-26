//! # helix-cohort — ADR-024: privacy-preserving cohort feature extraction
//!
//! The local primitive ADR-011 federates: it turns a user's dossier into a
//! feature vector that is **safe to leave the device — or contributes nothing**.
//! Re-identification from "anonymized" health data is a real hazard (§7.4), so
//! this is built and tested *before* any federation transport.
//!
//! Pipeline: **generalize → suppress → add DP noise → account ε**, and refuse
//! when nothing safe survives:
//!
//! 1. features arrive already coarsened to non-identifying **bands** (never raw
//!    values, dates, ids, or free text — those never leave the vault);
//! 2. a feature is **suppressed** if its estimated cohort cell is below the
//!    k threshold, or it is a flagged quasi-identifier that is rare;
//! 3. survivors get **Laplace noise** calibrated to an ε budget split across
//!    features (differential privacy);
//! 4. if nothing survives, return an error and contribute nothing.
//!
//! The DP noise source is injected (`NoiseSource`) so production uses a CSPRNG
//! while the policy is pure and exhaustively testable. Genomic features are
//! excluded by default (ADR-021 / GINA).

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A generalized (already-binned) feature ready for the privacy gate. The caller
/// is responsible for never putting a raw value, date, id, or free text here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeneralizedFeature {
    /// Stable key, e.g. "vitamin_d_band". Genomic keys (prefix `geno`) are dropped.
    pub key: String,
    /// Normalized band value in `[0,1]` (sensitivity 1.0 for the DP mechanism).
    pub value: f64,
    /// Estimated number of people sharing this generalized value (cohort cell).
    pub cohort_cell_size: u32,
    /// True if this feature is a quasi-identifier (age band, rare condition, …).
    pub quasi_identifier: bool,
}

/// Policy config for the privacy gate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CohortConfig {
    /// Total ε budget for this contribution (split evenly across survivors).
    pub epsilon: f64,
    /// k-anonymity threshold: cells smaller than this are suppressed.
    pub k_threshold: u32,
    /// Feature keys explicitly suppressed regardless.
    pub suppressed_keys: Vec<String>,
}

/// A contribution-safe cohort vector: noised band values + the ε actually spent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CohortVector {
    pub features: Vec<(String, f64)>,
    pub epsilon_spent: f64,
    /// Keys that were suppressed (for the user's transparency / audit).
    pub suppressed: Vec<String>,
}

/// Source of uniform samples in `(-0.5, 0.5)` for the Laplace mechanism.
/// Production: CSPRNG. Tests: a deterministic stub.
pub trait NoiseSource {
    fn uniform(&mut self) -> f64;
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum CohortError {
    #[error("epsilon must be > 0, got {0}")]
    BadEpsilon(f64),
    #[error("a feature band value was not finite or out of [0,1]")]
    BadValue,
    #[error("nothing safe to contribute after generalization + suppression")]
    NothingSafeToShare,
}

const GENO_PREFIX: &str = "geno";

fn is_excluded(f: &GeneralizedFeature, cfg: &CohortConfig) -> bool {
    f.key.to_lowercase().starts_with(GENO_PREFIX) // genomics excluded (ADR-021/GINA)
        || cfg.suppressed_keys.iter().any(|k| k == &f.key)
        || f.cohort_cell_size < cfg.k_threshold // below k-anonymity
        || (f.quasi_identifier && f.cohort_cell_size < cfg.k_threshold.saturating_mul(2))
    // quasi-identifiers need a wider cell
}

/// Laplace noise with scale `b` from a uniform sample `u` in `(-0.5, 0.5)`:
/// `-b * sgn(u) * ln(1 - 2|u|)`.
fn laplace(u: f64, b: f64) -> f64 {
    let u = u.clamp(-0.499_999, 0.499_999);
    -b * u.signum() * (1.0 - 2.0 * u.abs()).ln()
}

/// Run the privacy gate. Returns a contribution-safe [`CohortVector`], or
/// [`CohortError::NothingSafeToShare`] if no feature survives.
pub fn generalize(
    features: &[GeneralizedFeature],
    cfg: &CohortConfig,
    noise: &mut dyn NoiseSource,
) -> Result<CohortVector, CohortError> {
    if cfg.epsilon.is_nan() || cfg.epsilon <= 0.0 {
        return Err(CohortError::BadEpsilon(cfg.epsilon));
    }
    for f in features {
        if !f.value.is_finite() || !(0.0..=1.0).contains(&f.value) {
            return Err(CohortError::BadValue);
        }
    }

    let mut survivors = Vec::new();
    let mut suppressed = Vec::new();
    for f in features {
        if is_excluded(f, cfg) {
            suppressed.push(f.key.clone());
        } else {
            survivors.push(f);
        }
    }
    if survivors.is_empty() {
        return Err(CohortError::NothingSafeToShare);
    }

    // Split the ε budget across survivors; sensitivity is 1.0 (normalized bands).
    let per_eps = cfg.epsilon / survivors.len() as f64;
    let b = 1.0 / per_eps;
    let out: Vec<(String, f64)> = survivors
        .iter()
        .map(|f| {
            let noised = (f.value + laplace(noise.uniform(), b)).clamp(0.0, 1.0);
            (f.key.clone(), noised)
        })
        .collect();

    Ok(CohortVector {
        features: out,
        epsilon_spent: per_eps * survivors.len() as f64,
        suppressed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic noise: always returns 0.0 → laplace(0,b) == 0 (no shift),
    /// so we can assert the *policy* (suppression, ε accounting) exactly.
    struct ZeroNoise;
    impl NoiseSource for ZeroNoise {
        fn uniform(&mut self) -> f64 {
            0.0
        }
    }

    /// Fixed nonzero sample to confirm noise is actually applied.
    struct FixedNoise(f64);
    impl NoiseSource for FixedNoise {
        fn uniform(&mut self) -> f64 {
            self.0
        }
    }

    fn feat(key: &str, value: f64, cell: u32, qi: bool) -> GeneralizedFeature {
        GeneralizedFeature {
            key: key.into(),
            value,
            cohort_cell_size: cell,
            quasi_identifier: qi,
        }
    }

    fn cfg() -> CohortConfig {
        CohortConfig {
            epsilon: 1.0,
            k_threshold: 10,
            suppressed_keys: vec![],
        }
    }

    #[test]
    fn passes_safe_features_and_accounts_epsilon() {
        let feats = vec![
            feat("vitamin_d_band", 0.5, 5000, false),
            feat("sleep_band", 0.8, 8000, false),
        ];
        let v = generalize(&feats, &cfg(), &mut ZeroNoise).unwrap();
        assert_eq!(v.features.len(), 2);
        assert!((v.epsilon_spent - 1.0).abs() < 1e-9);
        // ZeroNoise → values unchanged
        assert!((v.features[0].1 - 0.5).abs() < 1e-9);
    }

    #[test]
    fn suppresses_below_k_anonymity() {
        let feats = vec![
            feat("rare_condition", 1.0, 3, false), // cell < k=10
            feat("vitamin_d_band", 0.5, 5000, false),
        ];
        let v = generalize(&feats, &cfg(), &mut ZeroNoise).unwrap();
        assert_eq!(v.features.len(), 1);
        assert!(v.suppressed.contains(&"rare_condition".to_string()));
    }

    #[test]
    fn quasi_identifiers_need_wider_cell() {
        // cell 15 >= k(10) but < 2k(20) and it's a quasi-identifier → suppressed.
        let feats = vec![feat("age_band", 0.4, 15, true)];
        assert_eq!(
            generalize(&feats, &cfg(), &mut ZeroNoise),
            Err(CohortError::NothingSafeToShare)
        );
        // cell 25 >= 2k → allowed
        let ok = vec![feat("age_band", 0.4, 25, true)];
        assert_eq!(
            generalize(&ok, &cfg(), &mut ZeroNoise)
                .unwrap()
                .features
                .len(),
            1
        );
    }

    #[test]
    fn genomic_features_excluded_by_default() {
        let feats = vec![feat("geno_risk_t2d", 0.7, 9999, false)];
        assert_eq!(
            generalize(&feats, &cfg(), &mut ZeroNoise),
            Err(CohortError::NothingSafeToShare)
        );
    }

    #[test]
    fn refuses_when_nothing_safe() {
        let feats = vec![feat("rare", 1.0, 1, false)];
        assert_eq!(
            generalize(&feats, &cfg(), &mut ZeroNoise),
            Err(CohortError::NothingSafeToShare)
        );
    }

    #[test]
    fn dp_noise_is_actually_applied() {
        let feats = vec![feat("vitamin_d_band", 0.5, 5000, false)];
        let noised = generalize(&feats, &cfg(), &mut FixedNoise(0.3)).unwrap();
        // with a nonzero uniform sample, the value must move off 0.5
        assert!((noised.features[0].1 - 0.5).abs() > 1e-6);
        assert!((0.0..=1.0).contains(&noised.features[0].1)); // stays in range
    }

    #[test]
    fn rejects_bad_epsilon_and_value() {
        let feats = vec![feat("x", 0.5, 99, false)];
        let mut bad = cfg();
        bad.epsilon = 0.0;
        assert_eq!(
            generalize(&feats, &bad, &mut ZeroNoise),
            Err(CohortError::BadEpsilon(0.0))
        );
        let badv = vec![feat("x", 1.5, 99, false)];
        assert_eq!(
            generalize(&badv, &cfg(), &mut ZeroNoise),
            Err(CohortError::BadValue)
        );
    }
}
