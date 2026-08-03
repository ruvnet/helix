use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompatibleNight {
    pub night_start_ms: i64,
    pub compatibility_fingerprint: String,
    pub accepted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LongitudinalOperation {
    Baseline,
    Direction,
    Correlation,
    ChangePoint { split_after_nights: usize },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum NightlyAbstention {
    InsufficientCompatibleNights {
        needed: usize,
        got: usize,
    },
    InsufficientChangePointSegment {
        needed_per_side: usize,
        before: usize,
        after: usize,
    },
    IncompatibleMethod,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum LongitudinalDisposition {
    Ready { compatible_valid_nights: usize },
    Abstained(NightlyAbstention),
}

pub fn assess_longitudinal(
    nights: &[CompatibleNight],
    expected_fingerprint: &str,
    operation: LongitudinalOperation,
) -> LongitudinalDisposition {
    if nights
        .iter()
        .any(|night| night.compatibility_fingerprint != expected_fingerprint)
    {
        return LongitudinalDisposition::Abstained(NightlyAbstention::IncompatibleMethod);
    }
    let unique: BTreeSet<i64> = nights
        .iter()
        .filter(|night| night.accepted)
        .map(|night| night.night_start_ms)
        .collect();
    let got = unique.len();
    match operation {
        LongitudinalOperation::Baseline => minimum(got, 7),
        LongitudinalOperation::Direction => minimum(got, 14),
        LongitudinalOperation::Correlation => minimum(got, 20),
        LongitudinalOperation::ChangePoint { split_after_nights } => {
            let before = split_after_nights.min(got);
            let after = got.saturating_sub(before);
            if before < 10 || after < 10 {
                LongitudinalDisposition::Abstained(
                    NightlyAbstention::InsufficientChangePointSegment {
                        needed_per_side: 10,
                        before,
                        after,
                    },
                )
            } else {
                LongitudinalDisposition::Ready {
                    compatible_valid_nights: got,
                }
            }
        }
    }
}

fn minimum(got: usize, needed: usize) -> LongitudinalDisposition {
    if got < needed {
        LongitudinalDisposition::Abstained(NightlyAbstention::InsufficientCompatibleNights {
            needed,
            got,
        })
    } else {
        LongitudinalDisposition::Ready {
            compatible_valid_nights: got,
        }
    }
}

pub fn compatibility_groups(nights: &[CompatibleNight]) -> BTreeMap<&str, Vec<&CompatibleNight>> {
    let mut groups: BTreeMap<&str, Vec<&CompatibleNight>> = BTreeMap::new();
    for night in nights {
        groups
            .entry(&night.compatibility_fingerprint)
            .or_default()
            .push(night);
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nights(count: usize, fingerprint: &str) -> Vec<CompatibleNight> {
        (0..count)
            .map(|n| CompatibleNight {
                night_start_ms: n as i64,
                compatibility_fingerprint: fingerprint.into(),
                accepted: true,
            })
            .collect()
    }

    #[test]
    fn gates_and_compatibility_abstain_exactly() {
        let fp = "44".repeat(32);
        assert_eq!(
            assess_longitudinal(&nights(6, &fp), &fp, LongitudinalOperation::Baseline),
            LongitudinalDisposition::Abstained(NightlyAbstention::InsufficientCompatibleNights {
                needed: 7,
                got: 6
            })
        );
        assert!(matches!(
            assess_longitudinal(&nights(7, &fp), &fp, LongitudinalOperation::Baseline),
            LongitudinalDisposition::Ready { .. }
        ));
        assert_eq!(
            assess_longitudinal(&nights(13, &fp), &fp, LongitudinalOperation::Direction),
            LongitudinalDisposition::Abstained(NightlyAbstention::InsufficientCompatibleNights {
                needed: 14,
                got: 13
            })
        );
        assert_eq!(
            assess_longitudinal(&nights(19, &fp), &fp, LongitudinalOperation::Correlation),
            LongitudinalDisposition::Abstained(NightlyAbstention::InsufficientCompatibleNights {
                needed: 20,
                got: 19
            })
        );
        assert_eq!(
            assess_longitudinal(
                &nights(20, &fp),
                &fp,
                LongitudinalOperation::ChangePoint {
                    split_after_nights: 9
                }
            ),
            LongitudinalDisposition::Abstained(NightlyAbstention::InsufficientChangePointSegment {
                needed_per_side: 10,
                before: 9,
                after: 11
            })
        );
        assert!(matches!(
            assess_longitudinal(
                &nights(20, &fp),
                &fp,
                LongitudinalOperation::ChangePoint {
                    split_after_nights: 10
                }
            ),
            LongitudinalDisposition::Ready { .. }
        ));
        let mut mixed = nights(14, &fp);
        mixed[3].compatibility_fingerprint = "55".repeat(32);
        assert_eq!(
            assess_longitudinal(&mixed, &fp, LongitudinalOperation::Direction),
            LongitudinalDisposition::Abstained(NightlyAbstention::IncompatibleMethod)
        );
        assert_eq!(compatibility_groups(&mixed).len(), 2);
    }
}
