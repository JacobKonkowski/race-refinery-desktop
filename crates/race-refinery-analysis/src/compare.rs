//! Lap comparison: time delta, per-sector deltas, and distance-aligned traces.
//!
//! Pure math over two laps' stored products. The caller (commands layer) loads
//! the traces/sectors/times from storage and hands them in; this module performs
//! no I/O. "Candidate" is the lap being examined, "reference" is what it is
//! measured against (session best or a user pick).

use serde::{Deserialize, Serialize};

use super::corners::{
    analyze_corners, AssistSpan, CornerAnalysis, CornerDelta, DeltaCurve, LapTimeline, TimingSource,
};
use super::types::{TracePoint, TrafficEvent};

/// Number of points on the shared distance grid used to align two laps.
const GRID_POINTS: usize = 200;

/// One lap's stored products, as needed for a comparison.
pub struct CompareInput<'a> {
    pub lap_id: i64,
    pub lap_time_ms: Option<f64>,
    /// `(sector_num, time_ms)` pairs.
    pub sectors: &'a [(i32, f64)],
    /// Trace points sorted ascending by `dist_pct`.
    pub traces: &'a [TracePoint],
    /// Sparse traffic tags for this lap (may be empty).
    pub traffic: &'a [TrafficEvent],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SectorDelta {
    pub sector_num: i32,
    pub candidate_ms: Option<f64>,
    pub reference_ms: Option<f64>,
    pub delta_ms: Option<f64>,
}

/// One point of the aligned overlay. Channels are `None` where a lap has no data
/// covering that part of the track.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlignedPoint {
    pub dist_pct: f64,
    pub candidate_speed: Option<f64>,
    pub reference_speed: Option<f64>,
    pub candidate_throttle: Option<f64>,
    pub reference_throttle: Option<f64>,
    pub candidate_brake: Option<f64>,
    pub reference_brake: Option<f64>,
    pub candidate_gear: Option<f64>,
    pub reference_gear: Option<f64>,
    pub candidate_steering: Option<f64>,
    pub reference_steering: Option<f64>,
    pub candidate_clutch: Option<f64>,
    pub reference_clutch: Option<f64>,
    pub candidate_rpm: Option<f64>,
    pub reference_rpm: Option<f64>,
    pub candidate_lat_accel: Option<f64>,
    pub reference_lat_accel: Option<f64>,
    pub candidate_long_accel: Option<f64>,
    pub reference_long_accel: Option<f64>,
    pub candidate_yaw_rate: Option<f64>,
    pub reference_yaw_rate: Option<f64>,
    /// Running gap (candidate minus reference, ms) from the start of the range
    /// both laps cover. `None` where either lap has no time curve.
    pub cumulative_delta_ms: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LapComparison {
    pub candidate_lap_id: i64,
    pub reference_lap_id: i64,
    pub candidate_time_ms: Option<f64>,
    pub reference_time_ms: Option<f64>,
    pub delta_ms: Option<f64>,
    pub sector_deltas: Vec<SectorDelta>,
    pub series: Vec<AlignedPoint>,
    /// Reference-lap corners in track order, with the candidate's loss in each.
    pub corners: Vec<CornerDelta>,
    /// Where ABS intervened on either lap, for shading the brake chart.
    pub assists: Vec<AssistSpan>,
    /// Traffic hits on the candidate lap (for corner / map tags).
    pub candidate_traffic: Vec<TrafficEvent>,
    /// Traffic hits on the reference lap.
    pub reference_traffic: Vec<TrafficEvent>,
    /// Where the running delta and corner times came from; `None` when either
    /// lap lacks enough trace to build a time curve.
    pub timing: Option<TimingSource>,
    pub track_length_m: Option<f64>,
}

pub fn compare_laps(candidate: &CompareInput, reference: &CompareInput) -> LapComparison {
    let delta_ms = match (candidate.lap_time_ms, reference.lap_time_ms) {
        (Some(c), Some(r)) => Some(c - r),
        _ => None,
    };
    let curve = LapTimeline::from_trace(candidate.traces, candidate.lap_time_ms)
        .zip(LapTimeline::from_trace(
            reference.traces,
            reference.lap_time_ms,
        ))
        .and_then(|(c, r)| DeltaCurve::new(c, r));

    let mut series = aligned_series(candidate.traces, reference.traces);
    if let Some(curve) = &curve {
        for point in &mut series {
            point.cumulative_delta_ms = curve.at(point.dist_pct);
        }
    }

    let CornerAnalysis { corners, assists } = curve
        .as_ref()
        .map(|c| analyze_corners(candidate.traces, reference.traces, c))
        .unwrap_or_default();

    LapComparison {
        candidate_lap_id: candidate.lap_id,
        reference_lap_id: reference.lap_id,
        candidate_time_ms: candidate.lap_time_ms,
        reference_time_ms: reference.lap_time_ms,
        delta_ms,
        sector_deltas: sector_deltas(candidate.sectors, reference.sectors),
        series,
        corners,
        assists,
        candidate_traffic: candidate.traffic.to_vec(),
        reference_traffic: reference.traffic.to_vec(),
        timing: curve.as_ref().map(DeltaCurve::source),
        track_length_m: curve.as_ref().and_then(DeltaCurve::track_length_m),
    }
}

fn sector_deltas(candidate: &[(i32, f64)], reference: &[(i32, f64)]) -> Vec<SectorDelta> {
    let mut nums: Vec<i32> = candidate
        .iter()
        .chain(reference.iter())
        .map(|(n, _)| *n)
        .collect();
    nums.sort_unstable();
    nums.dedup();

    nums.into_iter()
        .map(|sector_num| {
            let cand = candidate
                .iter()
                .find(|(n, _)| *n == sector_num)
                .map(|(_, t)| *t);
            let refr = reference
                .iter()
                .find(|(n, _)| *n == sector_num)
                .map(|(_, t)| *t);
            let delta_ms = match (cand, refr) {
                (Some(c), Some(r)) => Some(c - r),
                _ => None,
            };
            SectorDelta {
                sector_num,
                candidate_ms: cand,
                reference_ms: refr,
                delta_ms,
            }
        })
        .collect()
}

fn aligned_series(candidate: &[TracePoint], reference: &[TracePoint]) -> Vec<AlignedPoint> {
    (0..GRID_POINTS)
        .map(|i| {
            let dist_pct = i as f64 / (GRID_POINTS - 1) as f64;
            AlignedPoint {
                dist_pct,
                candidate_speed: interp(candidate, dist_pct, |p| p.speed),
                reference_speed: interp(reference, dist_pct, |p| p.speed),
                candidate_throttle: interp(candidate, dist_pct, |p| p.throttle),
                reference_throttle: interp(reference, dist_pct, |p| p.throttle),
                candidate_brake: interp(candidate, dist_pct, |p| p.brake),
                reference_brake: interp(reference, dist_pct, |p| p.brake),
                candidate_gear: interp(candidate, dist_pct, |p| p.gear as f64),
                reference_gear: interp(reference, dist_pct, |p| p.gear as f64),
                candidate_steering: interp(candidate, dist_pct, |p| p.steering),
                reference_steering: interp(reference, dist_pct, |p| p.steering),
                candidate_clutch: interp_opt(candidate, dist_pct, TracePoint::driver_clutch),
                reference_clutch: interp_opt(reference, dist_pct, TracePoint::driver_clutch),
                candidate_rpm: interp_opt(candidate, dist_pct, |p| p.rpm),
                reference_rpm: interp_opt(reference, dist_pct, |p| p.rpm),
                candidate_lat_accel: interp_opt(candidate, dist_pct, |p| p.lat_accel),
                reference_lat_accel: interp_opt(reference, dist_pct, |p| p.lat_accel),
                candidate_long_accel: interp_opt(candidate, dist_pct, |p| p.long_accel),
                reference_long_accel: interp_opt(reference, dist_pct, |p| p.long_accel),
                candidate_yaw_rate: interp_opt(candidate, dist_pct, |p| p.yaw_rate),
                reference_yaw_rate: interp_opt(reference, dist_pct, |p| p.yaw_rate),
                cumulative_delta_ms: None,
            }
        })
        .collect()
}

/// Linear interpolation of a channel at `x` over points sorted by `dist_pct`.
/// Returns `None` when the trace is empty or `x` falls outside its coverage.
pub(crate) fn interp(
    points: &[TracePoint],
    x: f64,
    accessor: impl Fn(&TracePoint) -> f64,
) -> Option<f64> {
    if points.len() < 2 {
        return points.first().map(&accessor);
    }
    let first = points.first().unwrap();
    let last = points.last().unwrap();
    if x < first.dist_pct || x > last.dist_pct {
        return None;
    }
    // Binary search for the bracketing pair.
    let idx = points.partition_point(|p| p.dist_pct <= x);
    if idx == 0 {
        return Some(accessor(first));
    }
    if idx >= points.len() {
        return Some(accessor(last));
    }
    let lo = &points[idx - 1];
    let hi = &points[idx];
    let span = hi.dist_pct - lo.dist_pct;
    if span.abs() < f64::EPSILON {
        return Some(accessor(lo));
    }
    let t = (x - lo.dist_pct) / span;
    Some(accessor(lo) + (accessor(hi) - accessor(lo)) * t)
}

fn interp_opt(
    points: &[TracePoint],
    x: f64,
    accessor: impl Fn(&TracePoint) -> Option<f64>,
) -> Option<f64> {
    if points.is_empty() {
        return None;
    }
    let first = points.first().unwrap();
    let last = points.last().unwrap();
    if x < first.dist_pct || x > last.dist_pct {
        return None;
    }
    let idx = points.partition_point(|p| p.dist_pct <= x);
    let lo = if idx == 0 {
        first
    } else {
        &points[idx - 1]
    };
    let hi = if idx >= points.len() {
        last
    } else {
        &points[idx]
    };
    match (accessor(lo), accessor(hi)) {
        (Some(a), Some(b)) => {
            let span = hi.dist_pct - lo.dist_pct;
            if span.abs() < f64::EPSILON {
                return Some(a);
            }
            let t = (x - lo.dist_pct) / span;
            Some(a + (b - a) * t)
        }
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tp(dist: f64, speed: f64, gear: i32, steering: f64) -> TracePoint {
        TracePoint {
            dist_pct: dist,
            speed,
            gear,
            steering,
            ..Default::default()
        }
    }

    #[test]
    fn time_and_sector_deltas() {
        let cand = CompareInput {
            lap_id: 1,
            lap_time_ms: Some(91_000.0),
            sectors: &[(1, 30_000.0), (2, 31_000.0), (3, 30_000.0)],
            traces: &[],
            traffic: &[],
        };
        let refr = CompareInput {
            lap_id: 2,
            lap_time_ms: Some(90_000.0),
            sectors: &[(1, 29_500.0), (2, 31_000.0), (3, 29_500.0)],
            traces: &[],
            traffic: &[],
        };
        let cmp = compare_laps(&cand, &refr);
        assert_eq!(cmp.delta_ms, Some(1_000.0));
        assert_eq!(cmp.sector_deltas.len(), 3);
        assert_eq!(cmp.sector_deltas[0].delta_ms, Some(500.0));
        assert_eq!(cmp.sector_deltas[1].delta_ms, Some(0.0));
    }

    #[test]
    fn aligned_series_interpolates_on_grid() {
        let cand = CompareInput {
            lap_id: 1,
            lap_time_ms: None,
            sectors: &[],
            traces: &[tp(0.0, 100.0, 3, 0.0), tp(1.0, 200.0, 5, 1.0)],
            traffic: &[],
        };
        let refr = CompareInput {
            lap_id: 2,
            lap_time_ms: None,
            sectors: &[],
            traces: &[tp(0.0, 50.0, 2, -0.5), tp(1.0, 150.0, 4, 0.5)],
            traffic: &[],
        };
        let cmp = compare_laps(&cand, &refr);
        assert_eq!(cmp.series.len(), GRID_POINTS);
        let mid = &cmp.series[GRID_POINTS / 2];
        assert!((mid.candidate_speed.unwrap() - 150.0).abs() < 2.0);
        assert!((mid.reference_speed.unwrap() - 100.0).abs() < 2.0);
        assert!((mid.candidate_gear.unwrap() - 4.0).abs() < 0.1);
        assert!((mid.reference_gear.unwrap() - 3.0).abs() < 0.1);
        assert!((mid.candidate_steering.unwrap() - 0.5).abs() < 0.05);
        assert!(mid.reference_steering.unwrap().abs() < 0.05);
    }

    #[test]
    fn clutch_prefers_raw_when_present() {
        let mut a = tp(0.0, 50.0, 3, 0.0);
        a.clutch = Some(0.2);
        a.clutch_raw = Some(0.8);
        let mut b = tp(1.0, 50.0, 3, 0.0);
        b.clutch = Some(0.2);
        b.clutch_raw = Some(0.8);
        let cand = CompareInput {
            lap_id: 1,
            lap_time_ms: None,
            sectors: &[],
            traces: &[a, b],
            traffic: &[],
        };
        let refr = CompareInput {
            lap_id: 2,
            lap_time_ms: None,
            sectors: &[],
            traces: &[tp(0.0, 50.0, 3, 0.0), tp(1.0, 50.0, 3, 0.0)],
            traffic: &[],
        };
        let mid = &compare_laps(&cand, &refr).series[GRID_POINTS / 2];
        assert!((mid.candidate_clutch.unwrap() - 0.8).abs() < 1e-6);
        assert!(mid.reference_clutch.is_none());
    }
}
