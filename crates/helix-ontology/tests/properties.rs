//! Property tests (proptest) for ontology normalization (ADR-004).
//!
//! The load-bearing invariant: a term is **only** `Normalized` when a candidate
//! is both above the floor AND clearly ahead of the runner-up. Everything else
//! is queued for human review — never silently coerced. We assert this holds for
//! arbitrary candidate score distributions.

use helix_ontology::{
    normalize, CanonicalCode, CodeSystem, Domain, NormalizationOutcome, RawTerm, ScoredCandidate,
};
use proptest::prelude::*;

fn term() -> RawTerm {
    RawTerm {
        text: "x".into(),
        domain: Domain::Observation,
        unit: None,
    }
}

fn cands(scores: Vec<f64>) -> Vec<ScoredCandidate> {
    scores
        .into_iter()
        .enumerate()
        .map(|(i, s)| ScoredCandidate {
            canonical: CanonicalCode::new(CodeSystem::Loinc, format!("c{i}"), "d"),
            score: s,
        })
        .collect()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// Normalization is sound: if the outcome is Normalized, the chosen candidate
    /// was >= floor and beat the runner-up by >= margin. (No silent coercion.)
    #[test]
    fn normalized_implies_confident_and_unambiguous(
        scores in prop::collection::vec(0.0f64..=1.0, 1..8),
        floor in 0.0f64..=1.0,
        margin in 0.0f64..=0.5,
    ) {
        let outcome = normalize(&term(), cands(scores.clone()), floor, margin).unwrap();
        if let NormalizationOutcome::Normalized { confidence, .. } = outcome {
            // top score cleared the floor
            prop_assert!(confidence >= floor);
            // and beat the runner-up by the margin
            let mut sorted = scores.clone();
            sorted.sort_by(|a, b| b.partial_cmp(a).unwrap());
            if let Some(&second) = sorted.get(1) {
                prop_assert!(sorted[0] - second >= margin - 1e-12);
            }
            // confidence equals the top score
            prop_assert!((confidence - sorted[0]).abs() < 1e-9);
        }
    }

    /// Anything below the floor is ALWAYS queued, never normalized.
    #[test]
    fn below_floor_always_queued(
        scores in prop::collection::vec(0.0f64..=0.49, 1..6),
        margin in 0.0f64..=0.3,
    ) {
        let floor = 0.5;
        let outcome = normalize(&term(), cands(scores), floor, margin).unwrap();
        prop_assert!(matches!(outcome, NormalizationOutcome::Queued(_)));
    }

    /// normalize never panics and always returns Ok for finite, in-range inputs.
    #[test]
    fn total_for_valid_inputs(
        scores in prop::collection::vec(0.0f64..=1.0, 0..10),
        floor in 0.0f64..=1.0,
        margin in 0.0f64..=1.0,
    ) {
        prop_assert!(normalize(&term(), cands(scores), floor, margin).is_ok());
    }
}
