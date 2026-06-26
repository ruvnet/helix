//! # helix-timeline — ADR-031: longitudinal health-score timeline
//!
//! Plots the ADR-016 composite score **over time**, computed deterministically
//! (ADR-007) from the historical subsystem inputs. Each point is the composite of
//! the data available *at that time*, tagged with the methodology version — a
//! methodology change is a visible break, never a silent retroactive rewrite.
//!
//! It is **trend-first** (slope + change-points via `helix-numeric`) and **honest
//! about uncertainty** (each point carries the confidence from its inputs). A
//! wellness-orientation aid, not a risk diagnosis (ADR-010).

use serde::{Deserialize, Serialize};

use helix_numeric::{change_point, slope_per_day, trend_direction, Point, TrendDirection};
use helix_score::{compose, SubScore};

/// A dated snapshot of the subsystem sub-scores (the inputs available at `at`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub at: i64,
    pub subscores: Vec<SubScore>,
}

/// One computed point on the score timeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScorePoint {
    pub at: i64,
    pub value: f64,
    pub confidence: f64,
    pub methodology_version: String,
}

/// The timeline plus its trend summary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Timeline {
    pub points: Vec<ScorePoint>,
    /// Overall direction across the series (units = score points/day).
    pub direction: TrendDirection,
    pub slope_per_day: Option<f64>,
    /// Most significant change-point timestamp, if any.
    pub change_point_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimelineError {
    Empty,
    Compose(String),
}

impl std::fmt::Display for TimelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TimelineError::Empty => write!(f, "no snapshots provided"),
            TimelineError::Compose(e) => write!(f, "score composition failed: {e}"),
        }
    }
}
impl std::error::Error for TimelineError {}

/// Build a score timeline from dated snapshots. Each snapshot composes to one
/// point (ADR-016); the series gets a deterministic trend + change-point.
/// `flat_band` is the score-points/day dead-band below which the trend is flat.
pub fn build_timeline(
    mut snapshots: Vec<Snapshot>,
    flat_band: f64,
) -> Result<Timeline, TimelineError> {
    if snapshots.is_empty() {
        return Err(TimelineError::Empty);
    }
    snapshots.sort_by_key(|s| s.at);

    let mut points = Vec::with_capacity(snapshots.len());
    for snap in snapshots {
        let score = compose(snap.subscores).map_err(|e| TimelineError::Compose(e.to_string()))?;
        points.push(ScorePoint {
            at: snap.at,
            value: score.value,
            confidence: score.confidence,
            methodology_version: score.methodology_version,
        });
    }

    // Trend + change-point over the value series (deterministic, ADR-007).
    let series: Vec<Point> = points.iter().map(|p| Point::new(p.at, p.value)).collect();
    let (slope, direction) = if series.len() >= 2 {
        match slope_per_day(&series) {
            Ok(s) => (Some(s), trend_direction(s, flat_band)),
            Err(_) => (None, TrendDirection::Flat),
        }
    } else {
        (None, TrendDirection::Flat)
    };
    let change_point_at = if series.len() >= 4 {
        change_point(&series).ok().flatten().map(|cp| cp.at)
    } else {
        None
    };

    Ok(Timeline {
        points,
        direction,
        slope_per_day: slope,
        change_point_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use helix_score::{Driver, Subsystem, Trend};

    const DAY: i64 = 86_400_000;

    fn snap(at_days: i64, value: f64) -> Snapshot {
        Snapshot {
            at: at_days * DAY,
            subscores: vec![SubScore {
                subsystem: Subsystem::Sleep,
                value,
                weight: 1.0,
                confidence: 0.9,
                drivers: vec![Driver {
                    concept: "Deep sleep".into(),
                    points: value,
                    trend: Trend::Stable,
                    source_record: "r".into(),
                }],
                trend: Trend::Stable,
            }],
        }
    }

    #[test]
    fn composes_each_snapshot_and_versions_it() {
        let tl = build_timeline(vec![snap(0, 70.0), snap(30, 80.0)], 0.01).unwrap();
        assert_eq!(tl.points.len(), 2);
        assert!((tl.points[0].value - 70.0).abs() < 1e-6);
        assert!(tl.points[1].methodology_version.starts_with("score-v"));
    }

    #[test]
    fn rising_series_is_rising() {
        let tl = build_timeline(vec![snap(0, 60.0), snap(10, 70.0), snap(20, 80.0)], 0.01).unwrap();
        assert_eq!(tl.direction, TrendDirection::Rising);
        assert!(tl.slope_per_day.unwrap() > 0.0);
    }

    #[test]
    fn detects_change_point() {
        // flat at 60 then jumps to 85
        let tl = build_timeline(
            vec![
                snap(0, 60.0),
                snap(10, 61.0),
                snap(20, 84.0),
                snap(30, 85.0),
            ],
            0.01,
        )
        .unwrap();
        assert!(tl.change_point_at.is_some());
    }

    #[test]
    fn empty_errors() {
        assert_eq!(build_timeline(vec![], 0.01), Err(TimelineError::Empty));
    }

    #[test]
    fn sorts_unordered_snapshots() {
        let tl = build_timeline(vec![snap(20, 80.0), snap(0, 60.0)], 0.01).unwrap();
        assert!(tl.points[0].at < tl.points[1].at);
    }
}
