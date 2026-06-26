//! # helix-verifier — ADR-008: Verifier/Critic Agent + Swarm Consensus
//!
//! A single agent can be confidently wrong. Before any clinically meaningful
//! claim reaches the user, a *second, independent* verifier re-derives it from
//! the source records and checks its evidence tier. Unsupported or over-reaching
//! claims are dropped or down-graded — the gate in §4 of the spec.
//!
//! Two invariants this crate encodes:
//!
//! 1. **Re-derivation, not trust.** The verifier is handed the claim and the
//!    *same* evidence set, and independently confirms every cited record exists
//!    and actually supports the assertion. It never takes the synthesizer's word.
//! 2. **Cross-family fusion.** The verifier model MUST come from a different
//!    model family than the synthesizer (ADR-008/018). That constraint is made
//!    explicit as a type — [`ModelFamily`] — and [`verify`] refuses to run a
//!    self-check where synthesizer and verifier families match.
//!
//! The actual LLM call is abstracted behind [`ClaimChecker`] so this crate stays
//! pure and testable; the production checker plugs a different-family model in.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use helix_evidence::EvidenceTier;
use helix_provenance::{GroundedClaim, ProvRecord};

/// The model family that produced (or is checking) a claim. The fusion rule
/// requires synthesizer ≠ verifier family.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelFamily(pub String);

impl<S: Into<String>> From<S> for ModelFamily {
    fn from(s: S) -> Self {
        ModelFamily(s.into())
    }
}

/// How meaningful a claim is — drives whether full verification + consensus is
/// required (ADR-008 tunes cost: informational claims get a lighter touch).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Criticality {
    /// Background / informational — single-pass check.
    Informational,
    /// Clinically meaningful — requires full verification + consensus quorum.
    Clinical,
}

/// One verifier's structured verdict on a claim (the `ClaimVerdict` of ADR-008).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClaimVerdict {
    /// Does every cited record exist and support the assertion?
    pub attribution_ok: bool,
    /// Is the asserted evidence tier justified by the backing records?
    pub tier_ok: bool,
    /// The tier the verifier believes is justified (may down-grade).
    pub justified_tier: EvidenceTier,
    /// Free-text rationale for the audit trail.
    pub rationale: String,
}

impl ClaimVerdict {
    pub fn supports(&self) -> bool {
        self.attribution_ok && self.tier_ok
    }
}

/// Pluggable claim checker — in production this wraps a different-family LLM;
/// in tests it is a deterministic stub.
pub trait ClaimChecker {
    fn family(&self) -> ModelFamily;
    /// Re-derive the claim against the evidence and asserted tier.
    fn check(
        &self,
        claim: &GroundedClaim,
        evidence: &[ProvRecord],
        asserted_tier: EvidenceTier,
    ) -> ClaimVerdict;
}

/// Final disposition after verification (+ consensus for clinical claims).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "disposition", rename_all = "snake_case")]
pub enum Verification {
    /// Passed — safe to surface, at the (possibly down-graded) tier.
    Approved { tier: EvidenceTier },
    /// Tier was reduced but the claim stands.
    DownGraded {
        from: EvidenceTier,
        to: EvidenceTier,
    },
    /// Rejected — the claim is dropped before the user sees it.
    Rejected { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum VerifierError {
    /// The fusion invariant was violated: verifier shares the synthesizer's family.
    #[error(
        "verifier family '{0}' must differ from synthesizer family (cross-family fusion, ADR-008)"
    )]
    SameFamily(String),
    /// A clinical claim needs an odd quorum ≥ 3 for a real majority.
    #[error("clinical consensus needs an odd quorum >= 3, got {0}")]
    BadQuorum(usize),
    /// Fewer checkers supplied than the requested quorum.
    #[error("need {needed} checkers for quorum, got {got}")]
    NotEnoughCheckers { needed: usize, got: usize },
}

/// Verify a single claim. `synthesizer` is the family that drafted it; every
/// checker must be a different family (the fusion rule). For [`Criticality::
/// Clinical`] claims, `quorum` independent checkers vote and a strict majority
/// must support; informational claims need a single supporting check.
pub fn verify(
    claim: &GroundedClaim,
    evidence: &[ProvRecord],
    asserted_tier: EvidenceTier,
    synthesizer: &ModelFamily,
    checkers: &[&dyn ClaimChecker],
    criticality: Criticality,
    quorum: usize,
) -> Result<Verification, VerifierError> {
    // Fusion invariant: no checker may share the synthesizer's family.
    for c in checkers {
        if &c.family() == synthesizer {
            return Err(VerifierError::SameFamily(c.family().0));
        }
    }

    let needed = match criticality {
        Criticality::Informational => 1,
        Criticality::Clinical => {
            if quorum < 3 || quorum % 2 == 0 {
                return Err(VerifierError::BadQuorum(quorum));
            }
            quorum
        }
    };
    if checkers.len() < needed {
        return Err(VerifierError::NotEnoughCheckers {
            needed,
            got: checkers.len(),
        });
    }

    let verdicts: Vec<ClaimVerdict> = checkers
        .iter()
        .take(needed)
        .map(|c| c.check(claim, evidence, asserted_tier))
        .collect();

    // Attribution failure by a strict majority => reject outright.
    let attribution_fail = verdicts.iter().filter(|v| !v.attribution_ok).count();
    if attribution_fail * 2 > needed {
        return Ok(Verification::Rejected {
            reason: "majority of verifiers could not attribute the claim to source".to_string(),
        });
    }

    let supporting = verdicts.iter().filter(|v| v.supports()).count();
    if supporting * 2 > needed {
        return Ok(Verification::Approved {
            tier: asserted_tier,
        });
    }

    // Not enough support at the asserted tier — try to down-grade to the
    // strictest tier a majority *can* justify (higher enum value = weaker).
    let mut justified = verdicts
        .iter()
        .map(|v| v.justified_tier)
        .collect::<Vec<_>>();
    justified.sort();
    // The median-ish majority-justified tier: the weakest tier such that a
    // majority justifies at least that strength.
    if let Some(&maj_tier) = justified.get(needed / 2) {
        if maj_tier > asserted_tier {
            return Ok(Verification::DownGraded {
                from: asserted_tier,
                to: maj_tier,
            });
        }
    }

    Ok(Verification::Rejected {
        reason: "insufficient independent support for the claim".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use helix_provenance::{
        ground, Confidence, DraftClaim, MeasurementMethod, ProvRecord, RecordId, ReferenceRange,
    };

    fn rec() -> ProvRecord {
        ProvRecord {
            id: RecordId::from("r1"),
            source: "Quest".into(),
            measured_at: 1_000,
            method: MeasurementMethod::LabFeed,
            code: Some("2276-4".into()),
            concept: "Ferritin".into(),
            value: 28.0,
            unit: "ng/mL".into(),
            reference_range: Some(ReferenceRange::new(Some(30.0), Some(400.0))),
            confidence: Confidence::FULL,
        }
    }

    fn a_claim() -> (GroundedClaim, Vec<ProvRecord>) {
        let ev = vec![rec()];
        let draft = DraftClaim::new("Ferritin is low.", [RecordId::from("r1")]);
        (ground(&draft, &ev).unwrap(), ev)
    }

    struct Stub {
        family: &'static str,
        attribution_ok: bool,
        tier_ok: bool,
        justified: EvidenceTier,
    }
    impl ClaimChecker for Stub {
        fn family(&self) -> ModelFamily {
            ModelFamily::from(self.family)
        }
        fn check(&self, _: &GroundedClaim, _: &[ProvRecord], _: EvidenceTier) -> ClaimVerdict {
            ClaimVerdict {
                attribution_ok: self.attribution_ok,
                tier_ok: self.tier_ok,
                justified_tier: self.justified,
                rationale: "stub".into(),
            }
        }
    }

    fn checker(family: &'static str, ok: bool) -> Stub {
        Stub {
            family,
            attribution_ok: ok,
            tier_ok: ok,
            justified: if ok {
                EvidenceTier::YourData
            } else {
                EvidenceTier::Heuristic
            },
        }
    }

    #[test]
    fn approves_informational_with_one_supporting_check() {
        let (claim, ev) = a_claim();
        let c = checker("haiku", true);
        let v = verify(
            &claim,
            &ev,
            EvidenceTier::YourData,
            &ModelFamily::from("gpt"),
            &[&c],
            Criticality::Informational,
            1,
        )
        .unwrap();
        assert_eq!(
            v,
            Verification::Approved {
                tier: EvidenceTier::YourData
            }
        );
    }

    #[test]
    fn rejects_same_family_as_synthesizer() {
        let (claim, ev) = a_claim();
        let c = checker("gpt", true);
        let err = verify(
            &claim,
            &ev,
            EvidenceTier::YourData,
            &ModelFamily::from("gpt"),
            &[&c],
            Criticality::Informational,
            1,
        )
        .unwrap_err();
        assert!(matches!(err, VerifierError::SameFamily(_)));
    }

    #[test]
    fn clinical_majority_supports_approves() {
        let (claim, ev) = a_claim();
        let (c1, c2, c3) = (checker("a", true), checker("b", true), checker("c", false));
        let v = verify(
            &claim,
            &ev,
            EvidenceTier::YourData,
            &ModelFamily::from("synth"),
            &[&c1, &c2, &c3],
            Criticality::Clinical,
            3,
        )
        .unwrap();
        assert_eq!(
            v,
            Verification::Approved {
                tier: EvidenceTier::YourData
            }
        );
    }

    #[test]
    fn clinical_majority_attribution_fail_rejects() {
        let (claim, ev) = a_claim();
        let (c1, c2, c3) = (checker("a", false), checker("b", false), checker("c", true));
        let v = verify(
            &claim,
            &ev,
            EvidenceTier::YourData,
            &ModelFamily::from("synth"),
            &[&c1, &c2, &c3],
            Criticality::Clinical,
            3,
        )
        .unwrap();
        assert!(matches!(v, Verification::Rejected { .. }));
    }

    #[test]
    fn even_quorum_rejected() {
        let (claim, ev) = a_claim();
        let c = checker("a", true);
        let err = verify(
            &claim,
            &ev,
            EvidenceTier::YourData,
            &ModelFamily::from("synth"),
            &[&c, &c],
            Criticality::Clinical,
            4,
        )
        .unwrap_err();
        assert!(matches!(err, VerifierError::BadQuorum(4)));
    }

    #[test]
    fn down_grades_when_majority_justifies_weaker_tier() {
        let (claim, ev) = a_claim();
        // All three attribute OK but justify only Tier 3 against a Tier-1 assertion.
        let mk = |fam: &'static str| Stub {
            family: fam,
            attribution_ok: true,
            tier_ok: false,
            justified: EvidenceTier::PeerReviewed,
        };
        let (c1, c2, c3) = (mk("a"), mk("b"), mk("c"));
        let v = verify(
            &claim,
            &ev,
            EvidenceTier::YourData,
            &ModelFamily::from("synth"),
            &[&c1, &c2, &c3],
            Criticality::Clinical,
            3,
        )
        .unwrap();
        assert_eq!(
            v,
            Verification::DownGraded {
                from: EvidenceTier::YourData,
                to: EvidenceTier::PeerReviewed
            }
        );
    }
}
