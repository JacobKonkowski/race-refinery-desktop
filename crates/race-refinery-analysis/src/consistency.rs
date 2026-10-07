//! Brake-point consistency: every lap's brake point and time through each of
//! the reference lap's corners, so a scatter shows whether braking later or
//! earlier actually paid off.

use serde::{Deserialize, Serialize};

use super::corners::{
    brake_point, corner_windows, grid, min_in, DeltaCurve, LapLanes, LapTimeline,
};
use super::types::TracePoint;

/// One lap's stored products, as needed for consistency.
pub struct ConsistencyLap<'a> {
    pub lap_id: i64,
    pub lap_time_ms: Option<f64>,
    /// Trace points sorted ascending by `dist_pct`.
    pub traces: &'a [TracePoint],
}

/// One lap through one corner, relative to the reference lap.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsistencyPoint {
    pub lap_id: i64,
    /// Metres after the reference brake point (negative = braked earlier).
    /// `None` when either lap didn't brake for the corner.
    pub brake_offset_m: Option<f64>,
    /// Time through the corner minus the reference's.
    pub time_delta_ms: f64,
    /// Slowest speed in the corner, m/s.
    pub min_speed: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CornerConsistency {
    /// Numbered like the reference lap's corners in a comparison; match on
    /// `apex_pct` since a comparison's narrower distance range can shift numbers.
    pub number: u32,
    pub apex_pct: f64,
    /// Sample standard deviation of `brake_offset_m` across laps.
    pub brake_spread_m: Option<f64>,
    pub points: Vec<ConsistencyPoint>,
}

/// Measure every lap through the reference lap's corners. Laps whose time
/// curve can't be built or doesn't overlap the reference are skipped.
pub fn corner_consistency(
    reference: &ConsistencyLap,
    laps: &[ConsistencyLap],
) -> Vec<CornerConsistency> {
    let Some(ref_tl) = LapTimeline::from_trace(reference.traces, reference.lap_time_ms) else {
        return Vec::new();
    };
    let (start, end) = ref_tl.range();
    let grid = grid(start, end);
    let ref_lanes = LapLanes::new(reference.traces, &ref_tl, &grid);
    let windows = corner_windows(&ref_lanes.speed);
    let track_length_m = ref_tl.track_length_m;

    let measured: Vec<(i64, LapLanes, DeltaCurve)> = laps
        .iter()
        .filter_map(|lap| {
            let tl = LapTimeline::from_trace(lap.traces, lap.lap_time_ms)?;
            let lanes = LapLanes::new(lap.traces, &tl, &grid);
            let delta = DeltaCurve::new(tl, ref_tl.clone())?;
            Some((lap.lap_id, lanes, delta))
        })
        .collect();

    windows
        .iter()
        .map(|w| {
            let ref_brake = brake_point(&ref_lanes.brake, w.brake_from, w.apex, w.prev_apex);
            let points: Vec<ConsistencyPoint> = measured
                .iter()
                .filter_map(|(lap_id, lanes, delta)| {
                    let time_delta_ms =
                        delta.at_clamped(grid[w.exit])? - delta.at_clamped(grid[w.entry])?;
                    let brake = brake_point(&lanes.brake, w.brake_from, w.apex, w.prev_apex);
                    Some(ConsistencyPoint {
                        lap_id: *lap_id,
                        brake_offset_m: brake
                            .zip(ref_brake)
                            .zip(track_length_m)
                            .map(|((c, r), l)| (grid[c] - grid[r]) * l),
                        time_delta_ms,
                        min_speed: min_in(&lanes.speed, w.entry, w.exit),
                    })
                })
                .collect();
            CornerConsistency {
                number: w.number,
                apex_pct: grid[w.apex],
                brake_spread_m: sample_std_dev(points.iter().filter_map(|p| p.brake_offset_m)),
                points,
            }
        })
        .collect()
}

fn sample_std_dev(values: impl Iterator<Item = f64>) -> Option<f64> {
    let values: Vec<f64> = values.collect();
    if values.len() < 2 {
        return None;
    }
    let n = values.len() as f64;
    let mean = values.iter().sum::<f64>() / n;
    let var = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0);
    Some(var.sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corners::tests::lap;

    #[test]
    fn brake_offsets_and_spread_across_laps() {
        let reference = lap(&[0.5], 30.0, 0.03, true);
        // Braking 1% of a 5 km lap (50 m) earlier and later than the reference.
        let early = lap(&[0.5], 30.0, 0.04, true);
        let late = lap(&[0.5], 30.0, 0.02, true);
        let input = |id, traces| ConsistencyLap {
            lap_id: id,
            lap_time_ms: None,
            traces,
        };
        let laps = [
            input(1, &reference[..]),
            input(2, &early[..]),
            input(3, &late[..]),
        ];

        let corners = corner_consistency(&laps[0], &laps);
        assert_eq!(corners.len(), 1);
        let c = &corners[0];
        assert!((c.apex_pct - 0.5).abs() < 0.01);
        assert_eq!(c.points.len(), 3);

        let offset = |id| {
            c.points
                .iter()
                .find(|p| p.lap_id == id)
                .and_then(|p| p.brake_offset_m)
                .unwrap()
        };
        assert!(offset(1).abs() < 5.0);
        assert!((offset(2) + 50.0).abs() < 10.0, "early {}", offset(2));
        assert!((offset(3) - 50.0).abs() < 10.0, "late {}", offset(3));
        // Same speed trace on every lap, so no corner time difference.
        assert!(c.points.iter().all(|p| p.time_delta_ms.abs() < 5.0));
        let spread = c.brake_spread_m.unwrap();
        assert!((spread - 50.0).abs() < 10.0, "spread {spread}");
    }

    #[test]
    fn laps_ending_short_of_the_reference_still_count() {
        let reference = lap(&[0.2, 0.9], 30.0, 0.03, true);
        let short: Vec<_> = reference
            .iter()
            .filter(|p| p.dist_pct > 0.01 && p.dist_pct < 0.98)
            .cloned()
            .collect();
        let laps = [
            ConsistencyLap {
                lap_id: 1,
                lap_time_ms: None,
                traces: &reference,
            },
            ConsistencyLap {
                lap_id: 2,
                lap_time_ms: None,
                traces: &short,
            },
        ];
        let corners = corner_consistency(&laps[0], &laps);
        assert_eq!(corners.len(), 2);
        assert!(corners.iter().all(|c| c.points.len() == 2), "{corners:?}");
    }

    #[test]
    fn skips_laps_without_a_timeline() {
        let reference = lap(&[0.5], 30.0, 0.03, true);
        let untimed = lap(&[0.5], 30.0, 0.03, false);
        let laps = [
            ConsistencyLap {
                lap_id: 1,
                lap_time_ms: None,
                traces: &reference,
            },
            ConsistencyLap {
                lap_id: 2,
                lap_time_ms: None,
                traces: &untimed,
            },
        ];
        let corners = corner_consistency(&laps[0], &laps);
        assert_eq!(corners[0].points.len(), 1);
        assert_eq!(corners[0].brake_spread_m, None);
    }
}
