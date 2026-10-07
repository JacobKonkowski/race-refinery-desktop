//! Post-import lap cleanup for sticky SDK times and phantom reset buckets.
//!
//! iRacing often leaves `LapLastLapTime` unchanged across tow/reset fragments and
//! emits `Lap == 0` buckets with no distance. We drop those phantoms, clear
//! obviously sticky times on incomplete/non-OK laps, and require near-full
//! distance coverage before a lap counts as pace-eligible.

use super::types::AnalyzedLap;

/// Minimum `LapDistPct` max for a lap to count toward best / deltas.
pub const FULL_LAP_PCT: f32 = 0.95;

/// Max coverage below which an `iracing_lap == 0` bucket is treated as phantom.
pub const PHANTOM_MAX_PCT: f32 = 0.01;

/// Times within this many ms are treated as identical (sticky copy).
const STICKY_EPS_MS: f64 = 0.5;

pub fn is_phantom_lap(iracing_lap: i32, lap_dist_pct_max: f32) -> bool {
    iracing_lap == 0 && lap_dist_pct_max < PHANTOM_MAX_PCT
}

pub fn has_full_coverage(lap_dist_pct_max: f32) -> bool {
    lap_dist_pct_max.is_finite() && lap_dist_pct_max >= FULL_LAP_PCT
}

pub fn pace_eligible_from(
    lap_time_ms: Option<f64>,
    delta_best_ok: Option<bool>,
    delta_session_best_ok: Option<bool>,
    lap_dist_pct_max: f32,
) -> bool {
    lap_time_ms.is_some()
        && delta_best_ok == Some(true)
        && delta_session_best_ok == Some(true)
        && has_full_coverage(lap_dist_pct_max)
}

/// Clear `LapLastLapTime` copies that stick onto incomplete or non-OK laps.
///
/// Walks each sub-session in order. When a lap's time matches the last *kept*
/// time and the lap is incomplete or not both-`_OK`, the time is wiped.
pub fn drop_sticky_lap_times(laps: &mut [AnalyzedLap]) {
    use std::collections::HashMap;

    let mut last_kept: HashMap<i32, f64> = HashMap::new();
    for lap in laps.iter_mut() {
        let Some(t) = lap.lap_time_ms else {
            continue;
        };
        let incomplete = !has_full_coverage(lap.lap_dist_pct_max);
        let not_ok = lap.delta_best_ok != Some(true) || lap.delta_session_best_ok != Some(true);
        if let Some(prev) = last_kept.get(&lap.session_num) {
            if (t - prev).abs() < STICKY_EPS_MS && (incomplete || not_ok) {
                lap.lap_time_ms = None;
                continue;
            }
        }
        last_kept.insert(lap.session_num, t);
    }
}

/// Drop phantom reset buckets and renumber laps 1..N within each sub-session.
pub fn remove_phantom_laps(mut laps: Vec<AnalyzedLap>) -> Vec<AnalyzedLap> {
    laps.retain(|l| !is_phantom_lap(l.iracing_lap, l.lap_dist_pct_max));
    renumber_laps(&mut laps);
    laps
}

fn renumber_laps(laps: &mut [AnalyzedLap]) {
    use std::collections::HashMap;
    let mut counters: HashMap<i32, i32> = HashMap::new();
    for lap in laps.iter_mut() {
        let n = counters.entry(lap.session_num).or_insert(0);
        *n += 1;
        lap.lap_number = *n;
    }
}

/// Sticky-time wipe + phantom removal for a freshly analyzed session.
pub fn finalize_analyzed_laps(mut laps: Vec<AnalyzedLap>) -> Vec<AnalyzedLap> {
    drop_sticky_lap_times(&mut laps);
    remove_phantom_laps(laps)
}

/// Apply sticky-time clearance to display rows (existing DB data).
/// `get_time` / `set_time` operate on one lap; laps must be in session order.
pub fn clear_sticky_times_in_place<T>(
    laps: &mut [T],
    session_num: impl Fn(&T) -> i32,
    get_time: impl Fn(&T) -> Option<f64>,
    set_time: impl Fn(&mut T, Option<f64>),
    max_pct: impl Fn(&T) -> f32,
    delta_best_ok: impl Fn(&T) -> Option<bool>,
    delta_session_best_ok: impl Fn(&T) -> Option<bool>,
) {
    use std::collections::HashMap;
    let mut last_kept: HashMap<i32, f64> = HashMap::new();
    for lap in laps.iter_mut() {
        let Some(t) = get_time(lap) else {
            continue;
        };
        let sn = session_num(lap);
        let incomplete = !has_full_coverage(max_pct(lap));
        let not_ok = delta_best_ok(lap) != Some(true) || delta_session_best_ok(lap) != Some(true);
        if let Some(prev) = last_kept.get(&sn) {
            if (t - prev).abs() < STICKY_EPS_MS && (incomplete || not_ok) {
                set_time(lap, None);
                continue;
            }
        }
        last_kept.insert(sn, t);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lap(
        session_num: i32,
        iracing_lap: i32,
        lap_number: i32,
        time: Option<f64>,
        ok: Option<bool>,
        max_pct: f32,
    ) -> AnalyzedLap {
        AnalyzedLap {
            session_num,
            session_type: "PRACTICE".into(),
            iracing_lap,
            lap_number,
            lap_time_ms: time,
            delta_best_ok: ok,
            delta_session_best_ok: ok,
            on_pit_road_start: false,
            on_pit_road_end: false,
            lap_dist_pct_min: 0.0,
            lap_dist_pct_max: max_pct,
            fuel_start: None,
            fuel_used: None,
            avg_speed: None,
            lf_temp: None,
            rf_temp: None,
            lr_temp: None,
            rr_temp: None,
            lf_pressure: None,
            rf_pressure: None,
            lr_pressure: None,
            rr_pressure: None,
            sectors: vec![],
            traces: vec![],
            traffic_events: vec![],
        }
    }

    #[test]
    fn phantom_zero_lap_detected() {
        assert!(is_phantom_lap(0, 0.0));
        assert!(!is_phantom_lap(0, 0.5));
        assert!(!is_phantom_lap(3, 0.0));
    }

    #[test]
    fn pace_requires_full_coverage() {
        assert!(pace_eligible_from(
            Some(90_000.0),
            Some(true),
            Some(true),
            0.99
        ));
        assert!(!pace_eligible_from(
            Some(90_000.0),
            Some(true),
            Some(true),
            0.7
        ));
    }

    #[test]
    fn sticky_incomplete_cleared_full_kept() {
        let mut laps = vec![
            lap(0, 5, 1, Some(116_180.0), Some(true), 1.0),
            lap(0, 6, 2, Some(118_575.0), Some(false), 0.75),
            lap(0, 0, 3, Some(118_575.0), Some(false), 0.0),
            lap(0, 6, 4, Some(118_575.0), Some(false), 0.4),
        ];
        drop_sticky_lap_times(&mut laps);
        assert_eq!(laps[0].lap_time_ms, Some(116_180.0));
        assert_eq!(laps[1].lap_time_ms, Some(118_575.0)); // first occurrence kept
        assert_eq!(laps[2].lap_time_ms, None); // sticky copy
        assert_eq!(laps[3].lap_time_ms, None); // sticky copy
    }

    #[test]
    fn sticky_after_first_incomplete_same_time() {
        // First incomplete gets a new sticky value from prior flying; subsequent copies clear.
        let mut laps = vec![
            lap(0, 5, 1, Some(116_000.0), Some(true), 1.0),
            lap(0, 6, 2, Some(118_575.0), Some(false), 0.75),
            lap(0, 0, 3, Some(118_575.0), Some(false), 0.0),
        ];
        drop_sticky_lap_times(&mut laps);
        assert_eq!(laps[1].lap_time_ms, Some(118_575.0));
        assert_eq!(laps[2].lap_time_ms, None);
    }

    #[test]
    fn finalize_removes_phantoms_and_renumbers() {
        let laps = vec![
            lap(0, 5, 1, Some(116_180.0), Some(true), 1.0),
            lap(0, 6, 2, Some(118_575.0), Some(false), 0.75),
            lap(0, 0, 3, Some(118_575.0), Some(false), 0.0),
            lap(0, 6, 4, Some(118_575.0), Some(false), 0.4),
        ];
        let out = finalize_analyzed_laps(laps);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].lap_number, 1);
        assert_eq!(out[1].lap_number, 2);
        assert_eq!(out[2].lap_number, 3);
        assert!(out
            .iter()
            .all(|l| l.iracing_lap != 0 || l.lap_dist_pct_max >= PHANTOM_MAX_PCT));
        assert_eq!(out[1].lap_time_ms, Some(118_575.0));
        assert_eq!(out[2].lap_time_ms, None);
    }

    #[test]
    fn identical_full_ok_laps_not_cleared() {
        // Pathologically identical flying laps — both complete + OK — keep both.
        let mut laps = vec![
            lap(0, 1, 1, Some(93_000.0), Some(true), 1.0),
            lap(0, 2, 2, Some(93_000.0), Some(true), 1.0),
        ];
        drop_sticky_lap_times(&mut laps);
        assert_eq!(laps[0].lap_time_ms, Some(93_000.0));
        assert_eq!(laps[1].lap_time_ms, Some(93_000.0));
    }
}
