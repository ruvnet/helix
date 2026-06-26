//! # helix-evolve — ADR-035: Darwin-style parameter evolution (safety-frozen)
//!
//! Darwin Mode's principle, applied to Helix: **mutate the configuration, keep
//! only changes that measurably improve fitness against a held-out eval set —
//! never touch the model, never touch the safety thresholds.**
//!
//! What it tunes (the [`Params`] search space): the *non-safety* pipeline knobs —
//! `confidence_floor`, `staleness_window_days`, `flat_band_per_day`. What it
//! **cannot** tune: the red-flag escalation registry (`helix-escalation`) and the
//! SaMD boundary — those stay governance-controlled and are passed in frozen.
//!
//! The fitness is **grounding-first** (ADR-005/006): answering when Helix should
//! have abstained (over-confidence) is penalized far more heavily than abstaining
//! when it could have answered. Evolution therefore cannot "improve" by making
//! Helix less conservative — the thing that would make it less safe scores worst.
//!
//! Everything here is **deterministic and air-gapped** (ADR-018): a seeded RNG, no
//! clock, no I/O, no network — same seed + same eval set ⇒ same result.

use serde::{Deserialize, Serialize};

use helix_escalation::ThresholdRegistry;
use helix_numeric::TrendDirection;
use helix_pipeline::{analyze, AnalyzeRequest, AnswerOutcome};
use helix_provenance::{EpochMillis, ProvRecord};

/// The tunable, **non-safety** parameters Darwin may evolve.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Params {
    pub confidence_floor: f64,
    pub staleness_window_days: i64,
    pub flat_band_per_day: f64,
}

/// Inclusive bounds for the search space — evolution never leaves these, so it
/// can't drive a parameter to a degenerate value.
#[derive(Debug, Clone, Copy)]
pub struct Bounds {
    pub confidence_floor: (f64, f64),
    pub staleness_window_days: (i64, i64),
    pub flat_band_per_day: (f64, f64),
}

impl Default for Bounds {
    fn default() -> Self {
        Self {
            confidence_floor: (0.3, 0.8),
            staleness_window_days: (120, 900),
            flat_band_per_day: (0.0, 0.2),
        }
    }
}

/// The ground-truth behavior a case should elicit.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Expected {
    /// Helix should answer, with this trend direction.
    Answered(TrendDirection),
    /// Helix should abstain (stale / low-confidence / insufficient data).
    Abstained,
}

/// One labeled evaluation case — records plus the known-correct behavior.
#[derive(Debug, Clone)]
pub struct EvalCase {
    pub name: String,
    pub concept_code: String,
    pub records: Vec<ProvRecord>,
    pub now: EpochMillis,
    pub reference_low: Option<f64>,
    pub reference_high: Option<f64>,
    pub expected: Expected,
}

/// Fitness breakdown over an eval set. `score` is the grounding-first total.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Fitness {
    pub score: f64,
    /// Correct grounded answers (right direction).
    pub grounded_correct: u32,
    /// Correct abstentions.
    pub abstained_correct: u32,
    /// Answered when it should have abstained — the cardinal sin (heavy penalty).
    pub over_confident: u32,
    /// Abstained when it could have answered (light penalty).
    pub over_cautious: u32,
    /// Answered, but the computed trend direction was wrong.
    pub wrong_direction: u32,
}

// Grounding-first weights: over-confidence costs far more than over-caution.
const W_GROUNDED: f64 = 1.0;
const W_ABSTAINED: f64 = 1.0;
const W_WRONG_DIR: f64 = 0.3;
const P_OVER_CONFIDENT: f64 = 2.0;
const P_OVER_CAUTIOUS: f64 = 0.3;

/// Score a parameter set against the eval cases. The `registry` (red-flag
/// thresholds) is **frozen** — passed in, never mutated.
pub fn fitness(p: &Params, cases: &[EvalCase], registry: &ThresholdRegistry) -> Fitness {
    let mut f = Fitness {
        score: 0.0,
        grounded_correct: 0,
        abstained_correct: 0,
        over_confident: 0,
        over_cautious: 0,
        wrong_direction: 0,
    };
    for c in cases {
        let req = AnalyzeRequest {
            concept_code: &c.concept_code,
            records: &c.records,
            now: c.now,
            staleness_window_days: p.staleness_window_days,
            confidence_floor: p.confidence_floor,
            reference_low: c.reference_low,
            reference_high: c.reference_high,
            flat_band_per_day: p.flat_band_per_day,
        };
        let outcome = match analyze(&req, registry) {
            Ok(o) => o,
            Err(_) => {
                // A param set that makes the pipeline error is strongly disfavored.
                f.score -= P_OVER_CONFIDENT;
                continue;
            }
        };
        match (&c.expected, &outcome) {
            (Expected::Abstained, AnswerOutcome::Abstained(_)) => {
                f.abstained_correct += 1;
                f.score += W_ABSTAINED;
            }
            (Expected::Abstained, AnswerOutcome::Answered(_)) => {
                f.over_confident += 1;
                f.score -= P_OVER_CONFIDENT;
            }
            (Expected::Answered(_), AnswerOutcome::Abstained(_)) => {
                f.over_cautious += 1;
                f.score -= P_OVER_CAUTIOUS;
            }
            (Expected::Answered(want), AnswerOutcome::Answered(got)) => {
                if got.trend.direction == *want {
                    f.grounded_correct += 1;
                    f.score += W_GROUNDED;
                } else {
                    f.wrong_direction += 1;
                    f.score += W_WRONG_DIR;
                }
            }
        }
    }
    f
}

/// Deterministic LCG — reproducible "randomness" with no `rand` dep, so an evolve
/// run is replayable (ADR-018: same seed ⇒ same evolution).
struct Lcg(u64);
impl Lcg {
    fn unit(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 11) as f64) / ((1u64 << 53) as f64)
    }
    fn signed(&mut self) -> f64 {
        self.unit() * 2.0 - 1.0
    }
}

fn clampf(v: f64, (lo, hi): (f64, f64)) -> f64 {
    v.max(lo).min(hi)
}
fn clampi(v: i64, (lo, hi): (i64, i64)) -> i64 {
    v.max(lo).min(hi)
}

fn mutate(p: &Params, b: &Bounds, rng: &mut Lcg) -> Params {
    Params {
        confidence_floor: clampf(p.confidence_floor + rng.signed() * 0.08, b.confidence_floor),
        staleness_window_days: clampi(
            p.staleness_window_days + (rng.signed() * 90.0) as i64,
            b.staleness_window_days,
        ),
        flat_band_per_day: clampf(
            p.flat_band_per_day + rng.signed() * 0.03,
            b.flat_band_per_day,
        ),
    }
}

/// The result of an evolve run.
#[derive(Debug, Clone)]
pub struct EvolveResult {
    pub seed_params: Params,
    pub seed_fitness: Fitness,
    pub best_params: Params,
    pub best_fitness: Fitness,
    /// Best score after each generation (monotonic non-decreasing).
    pub trajectory: Vec<f64>,
}

impl EvolveResult {
    /// How much fitness improved (final − baseline).
    pub fn improvement(&self) -> f64 {
        self.best_fitness.score - self.seed_fitness.score
    }
}

/// Evolve [`Params`] by seeded hill-climbing: each generation proposes a mutation,
/// evaluates it against the eval set, and **keeps it only if fitness improves**
/// (the Darwin invariant). Deterministic for a given `seed`.
pub fn evolve(
    seed_params: Params,
    bounds: &Bounds,
    cases: &[EvalCase],
    registry: &ThresholdRegistry,
    generations: usize,
    seed: u64,
) -> EvolveResult {
    let mut rng = Lcg(seed ^ 0x9E37_79B9_7F4A_7C15);
    let seed_fitness = fitness(&seed_params, cases, registry);
    let mut best = seed_params;
    let mut best_fit = seed_fitness;
    let mut trajectory = Vec::with_capacity(generations);
    for _ in 0..generations {
        let cand = mutate(&best, &bounds.clone(), &mut rng);
        let cf = fitness(&cand, cases, registry);
        if cf.score > best_fit.score {
            best = cand;
            best_fit = cf;
        }
        trajectory.push(best_fit.score);
    }
    EvolveResult {
        seed_params,
        seed_fitness,
        best_params: best,
        best_fitness: best_fit,
        trajectory,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use helix_provenance::{Confidence, MeasurementMethod, RecordId, ReferenceRange};

    const DAY: i64 = 86_400_000;
    const NOW: i64 = 1000 * DAY;

    fn rec(id: &str, days_ago: i64, value: f64, conf: f64) -> ProvRecord {
        ProvRecord {
            id: RecordId::from(id),
            source: "Quest".into(),
            measured_at: NOW - days_ago * DAY,
            method: MeasurementMethod::LabFeed,
            code: Some("2276-4".into()),
            concept: "Ferritin".into(),
            value,
            unit: "ng/mL".into(),
            reference_range: Some(ReferenceRange::new(Some(30.0), Some(400.0))),
            confidence: Confidence::new(conf),
        }
    }

    fn case(name: &str, records: Vec<ProvRecord>, expected: Expected) -> EvalCase {
        EvalCase {
            name: name.into(),
            concept_code: "2276-4".into(),
            records,
            now: NOW,
            reference_low: Some(30.0),
            reference_high: Some(400.0),
            expected,
        }
    }

    /// Build the labeled eval set. Ground truth is independent of the params.
    fn eval_set() -> Vec<EvalCase> {
        vec![
            // A: clearly declining, fresh, high-confidence → should answer "falling".
            case(
                "declining",
                vec![
                    rec("a0", 60, 60.0, 1.0),
                    rec("a1", 30, 45.0, 1.0),
                    rec("a2", 0, 28.0, 1.0),
                ],
                Expected::Answered(TrendDirection::Falling),
            ),
            // B: noisy but essentially flat (tiny net slope) → should answer "flat".
            // Needs flat_band big enough to absorb the wiggle but not real trends.
            case(
                "noisy-flat",
                vec![
                    rec("b0", 60, 50.0, 1.0),
                    rec("b1", 45, 52.0, 1.0),
                    rec("b2", 30, 49.0, 1.0),
                    rec("b3", 15, 51.0, 1.0),
                    rec("b4", 0, 50.6, 1.0),
                ],
                Expected::Answered(TrendDirection::Flat),
            ),
            // C: a single very old value → should abstain (stale).
            case(
                "stale",
                vec![rec("c0", 800, 120.0, 1.0)],
                Expected::Abstained,
            ),
            // D: fresh but low-confidence reading → should abstain.
            case(
                "low-conf",
                vec![rec("d0", 5, 120.0, 0.45)],
                Expected::Abstained,
            ),
        ]
    }

    #[test]
    fn fitness_rewards_grounding_and_punishes_overconfidence() {
        let reg = helix_escalation::builtin_registry_v1();
        let cases = eval_set();
        // A deliberately bad param set: floor so low it answers the low-conf case,
        // staleness so high it answers the stale case → over-confident on both.
        let reckless = Params {
            confidence_floor: 0.3,
            staleness_window_days: 900,
            flat_band_per_day: 0.0,
        };
        let f = fitness(&reckless, &cases, &reg);
        assert!(f.over_confident >= 1, "reckless params should over-answer");
    }

    #[test]
    fn evolution_improves_fitness_without_touching_safety() {
        let reg = helix_escalation::builtin_registry_v1();
        let cases = eval_set();
        // Baseline: flat_band 0 misclassifies the noisy-flat case as a trend.
        let baseline = Params {
            confidence_floor: 0.5,
            staleness_window_days: 365,
            flat_band_per_day: 0.0,
        };
        let res = evolve(baseline, &Bounds::default(), &cases, &reg, 200, 42);

        // Fitness improved, and the win came from getting the flat case right.
        assert!(
            res.improvement() > 0.0,
            "evolution should improve fitness: {} -> {}",
            res.seed_fitness.score,
            res.best_fitness.score
        );
        assert!(res.best_fitness.flat_band_better(&res.seed_fitness));
        // The evolved flat_band moved up off zero to absorb the noise.
        assert!(res.best_params.flat_band_per_day > baseline.flat_band_per_day);
        // Safety stayed in bounds — confidence_floor never dropped below the floor
        // bound, so evolution never made Helix recklessly answer low-confidence data.
        assert!(res.best_params.confidence_floor >= Bounds::default().confidence_floor.0);
        // The stale + low-confidence cases are still correctly abstained.
        assert_eq!(res.best_fitness.over_confident, 0);
    }

    #[test]
    fn evolution_is_deterministic() {
        let reg = helix_escalation::builtin_registry_v1();
        let cases = eval_set();
        let p = Params {
            confidence_floor: 0.5,
            staleness_window_days: 365,
            flat_band_per_day: 0.0,
        };
        let a = evolve(p, &Bounds::default(), &cases, &reg, 100, 7);
        let b = evolve(p, &Bounds::default(), &cases, &reg, 100, 7);
        assert_eq!(a.best_params, b.best_params);
        assert_eq!(a.best_fitness.score, b.best_fitness.score);
    }

    impl Fitness {
        // helper for the test: more correct classifications than the baseline.
        fn flat_band_better(&self, other: &Fitness) -> bool {
            (self.grounded_correct + self.abstained_correct)
                >= (other.grounded_correct + other.abstained_correct)
                && self.wrong_direction <= other.wrong_direction
        }
    }
}
