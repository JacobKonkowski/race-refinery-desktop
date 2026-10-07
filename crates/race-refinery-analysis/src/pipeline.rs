//! Post-session pipeline: raw frames -> [`AnalyzedSession`].
//!
//! The pipeline is pure (no I/O, no Tauri). It segments frames into laps,
//! records SDK facts plus computed products, then applies cleanup for sticky
//! `LapLastLapTime` copies and phantom reset buckets.

use rayon::prelude::*;

use super::aggregates::{
    average_speed, downsample_traces, fuel_stats, tire_averages, tire_pressure_averages,
};
use super::cleanup::finalize_analyzed_laps;
use super::sectors::compute_sector_times;
use super::segment::{lap_dist_range, segment_laps};
use super::track_map::outline_from_laps;
use super::types::{AnalyzedLap, AnalyzedSession, LapFrames, SessionMeta};

/// Analyze a full session's frames using the resolved session metadata.
pub fn analyze_session(frames: Vec<super::types::RawFrame>, meta: &SessionMeta) -> AnalyzedSession {
    let groups = segment_laps(frames, &meta.session_labels);
    let track_map = outline_from_laps(&meta.track, &groups);

    let laps: Vec<AnalyzedLap> = groups
        .into_par_iter()
        .map(|group| analyze_lap(group, &meta.sector_boundaries))
        .collect();

    // Parallel map can reorder; restore session/lap order before cleanup.
    let mut laps = laps;
    laps.sort_by(|a, b| {
        a.session_num
            .cmp(&b.session_num)
            .then(a.lap_number.cmp(&b.lap_number))
    });
    let laps = finalize_analyzed_laps(laps);

    AnalyzedSession {
        track: meta.track.clone(),
        car: meta.car.clone(),
        session_date: meta.session_date.clone(),
        laps,
        track_map,
    }
}

fn analyze_lap(group: LapFrames, boundaries: &[super::types::SectorBoundary]) -> AnalyzedLap {
    let frames = &group.frames;
    let (min_pct, max_pct) = lap_dist_range(frames);
    let (fuel_start, fuel_used) = fuel_stats(frames);
    let (lf_temp, rf_temp, lr_temp, rr_temp) = tire_averages(frames);
    let (lf_pressure, rf_pressure, lr_pressure, rr_pressure) = tire_pressure_averages(frames);
    let sectors = compute_sector_times(frames, boundaries);
    let traces = downsample_traces(frames);

    let on_pit_road_start = frames.first().map(|f| f.on_pit_road).unwrap_or(false);
    let on_pit_road_end = frames.last().map(|f| f.on_pit_road).unwrap_or(false);

    AnalyzedLap {
        session_num: group.session_num,
        session_type: group.session_type,
        iracing_lap: group.iracing_lap,
        lap_number: group.lap_number,
        lap_time_ms: group.sdk_lap_time_ms,
        delta_best_ok: group.delta_best_ok,
        delta_session_best_ok: group.delta_session_best_ok,
        on_pit_road_start,
        on_pit_road_end,
        lap_dist_pct_min: if min_pct.is_finite() { min_pct } else { 0.0 },
        lap_dist_pct_max: if max_pct.is_finite() { max_pct } else { 0.0 },
        fuel_start,
        fuel_used,
        avg_speed: average_speed(frames),
        lf_temp,
        rf_temp,
        lr_temp,
        rr_temp,
        lf_pressure,
        rf_pressure,
        lr_pressure,
        rr_pressure,
        sectors,
        traces,
        traffic_events: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::types::{RawFrame, SectorBoundary};
    use super::*;
    use std::collections::HashMap;

    fn frame(
        lap: i32,
        pct: f32,
        t: f64,
        last: Option<f32>,
        ok: Option<bool>,
        pit: bool,
    ) -> RawFrame {
        RawFrame {
            lap,
            lap_dist_pct: pct,
            speed: 55.0,
            throttle: 1.0,
            gear: 4,
            fuel_level: 50.0,
            on_pit_road: pit,
            session_time: t,
            lap_last_lap_time: last,
            delta_best_ok: ok,
            delta_session_best_ok: ok,
            lf_temp: 80.0,
            rf_temp: 80.0,
            lr_temp: 80.0,
            rr_temp: 80.0,
            ..Default::default()
        }
    }

    fn meta() -> SessionMeta {
        SessionMeta {
            track: "Test".into(),
            car: "Car".into(),
            session_date: "2026-01-01".into(),
            sector_boundaries: vec![
                SectorBoundary {
                    sector_num: 1,
                    start_pct: 0.34,
                },
                SectorBoundary {
                    sector_num: 2,
                    start_pct: 0.72,
                },
            ],
            session_labels: HashMap::from([(0, "Practice".to_string())]),
        }
    }

    #[test]
    fn eligible_lap_from_ok_flags() {
        // Lap 1 sweeps the whole lap; lap 2's first frame carries lap 1's time + OK.
        let mut frames = Vec::new();
        for i in 0..100 {
            frames.push(frame(1, i as f32 / 99.0, i as f64, None, None, false));
        }
        frames.push(frame(2, 0.02, 100.0, Some(90.0), Some(true), false));
        frames.push(frame(2, 0.5, 101.0, None, None, false));

        let session = analyze_session(frames, &meta());
        let lap1 = &session.laps[0];
        assert_eq!(lap1.lap_time_ms, Some(90_000.0));
        assert!(lap1.pace_eligible());
        assert_eq!(session.best_lap_ms(), Some(90_000.0));
        assert!(!lap1.sectors.is_empty());
    }

    #[test]
    fn lap_without_ok_flags_is_not_eligible() {
        let mut frames = Vec::new();
        for i in 0..50 {
            frames.push(frame(1, i as f32 / 49.0, i as f64, None, None, false));
        }
        // transition without _OK channels (older IBT)
        frames.push(frame(2, 0.02, 51.0, Some(88.0), None, false));
        let session = analyze_session(frames, &meta());
        assert_eq!(session.laps[0].lap_time_ms, Some(88_000.0));
        assert!(!session.laps[0].pace_eligible());
        assert_eq!(session.best_lap_ms(), None);
    }

    #[test]
    fn pit_flags_recorded() {
        let frames = vec![
            frame(1, 0.0, 0.0, None, None, true),
            frame(1, 0.5, 1.0, None, None, false),
            frame(2, 0.02, 2.0, Some(60.0), Some(false), false),
        ];
        let session = analyze_session(frames, &meta());
        assert!(session.laps[0].on_pit_road_start);
        assert!(!session.laps[0].on_pit_road_end);
    }
}
