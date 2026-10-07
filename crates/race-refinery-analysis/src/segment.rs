//! Frame -> lap segmentation.
//!
//! Laps are split on any change of `(SessionNum, Lap)`. The `_OK` flags for a
//! completed lap are sampled on the next lap's first frame. The official lap
//! time is published a second or two *later*: `LapLastLapTime` still holds the
//! previous lap's value on the transition frame, so we take the first update
//! that follows it.

use std::collections::HashMap;

use super::types::{LapFrames, RawFrame};

/// How long after a lap change to wait for `LapLastLapTime` to update. iRacing
/// typically publishes 1–2 s after the line; no update in this window means the
/// lap has no official time (reset, tow, or session end).
const LAP_TIME_PUBLISH_WINDOW_S: f64 = 5.0;

pub fn segment_laps(
    frames: Vec<RawFrame>,
    session_labels: &HashMap<i32, String>,
) -> Vec<LapFrames> {
    // (transition index, official time, OK flags) for every lap boundary.
    let closes: Vec<_> = (1..frames.len())
        .filter(|&i| {
            frames[i].session_num != frames[i - 1].session_num || frames[i].lap != frames[i - 1].lap
        })
        .map(|i| {
            (
                i,
                published_lap_time_ms(&frames, i),
                frames[i].delta_best_ok,
                frames[i].delta_session_best_ok,
            )
        })
        .collect();

    let mut laps: Vec<LapFrames> = Vec::new();
    let mut closes = closes.into_iter().peekable();
    let mut bucket: Vec<RawFrame> = Vec::new();
    for (i, frame) in frames.into_iter().enumerate() {
        if let Some((_, sdk_ms, ok_best, ok_session)) = closes.next_if(|c| c.0 == i) {
            laps.push(finish_bucket(
                session_labels,
                std::mem::take(&mut bucket),
                sdk_ms,
                ok_best,
                ok_session,
            ));
        }
        bucket.push(frame);
    }
    if !bucket.is_empty() {
        // Final lap: no following frame, so no official time or flags.
        laps.push(finish_bucket(session_labels, bucket, None, None, None));
    }

    assign_lap_numbers(&mut laps);
    laps
}

/// The official time for the lap that ended just before `transition`: the first
/// `LapLastLapTime` value that differs from the one the lap ran with.
fn published_lap_time_ms(frames: &[RawFrame], transition: usize) -> Option<f64> {
    let at = &frames[transition];
    let stale = frames[transition - 1].lap_last_lap_time;
    frames[transition..]
        .iter()
        .take_while(|f| {
            f.session_num == at.session_num
                && f.session_time - at.session_time <= LAP_TIME_PUBLISH_WINDOW_S
        })
        .find(|f| f.lap_last_lap_time != stale)
        .and_then(|f| sdk_lap_time_ms(f.lap_last_lap_time))
}

fn finish_bucket(
    session_labels: &HashMap<i32, String>,
    frames: Vec<RawFrame>,
    sdk_lap_time_ms: Option<f64>,
    delta_best_ok: Option<bool>,
    delta_session_best_ok: Option<bool>,
) -> LapFrames {
    let (session_num, iracing_lap) = (frames[0].session_num, frames[0].lap);
    LapFrames {
        session_num,
        session_type: session_labels
            .get(&session_num)
            .cloned()
            .unwrap_or_else(|| format!("Session {session_num}")),
        iracing_lap,
        lap_number: 0,
        sdk_lap_time_ms,
        delta_best_ok,
        delta_session_best_ok,
        frames,
    }
}

/// Number laps 1..N separately within each sub-session.
fn assign_lap_numbers(laps: &mut [LapFrames]) {
    let mut counters: HashMap<i32, i32> = HashMap::new();
    for lap in laps {
        let counter = counters.entry(lap.session_num).or_insert(0);
        *counter += 1;
        lap.lap_number = *counter;
    }
}

/// iRacing reports lap times in seconds (`f32`); a negative value means unset.
/// Convert to integer milliseconds, rounding to the nearest ms to absorb `f32`
/// representation error rather than biasing every time downward.
fn sdk_lap_time_ms(secs: Option<f32>) -> Option<f64> {
    match secs {
        Some(s) if s > 0.0 => Some((s as f64 * 1000.0).round()),
        _ => None,
    }
}

/// Minimum and maximum `LapDistPct` observed across the frames.
pub fn lap_dist_range(frames: &[RawFrame]) -> (f32, f32) {
    frames
        .iter()
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), f| {
            (lo.min(f.lap_dist_pct), hi.max(f.lap_dist_pct))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(
        session_num: i32,
        lap: i32,
        pct: f32,
        last: Option<f32>,
        ok: Option<bool>,
    ) -> RawFrame {
        RawFrame {
            session_num,
            lap,
            lap_dist_pct: pct,
            speed: 50.0,
            gear: 3,
            fuel_level: 50.0,
            lap_last_lap_time: last,
            delta_best_ok: ok,
            delta_session_best_ok: ok,
            ..Default::default()
        }
    }

    #[test]
    fn splits_on_lap_change_and_samples_transition() {
        let labels = HashMap::from([(0, "Practice".to_string())]);
        let frames = vec![
            frame(0, 1, 0.1, None, None),
            frame(0, 1, 0.9, None, None),
            // transition into lap 2 carries lap 1's official time + OK flags
            frame(0, 2, 0.05, Some(82.653), Some(true)),
            frame(0, 2, 0.9, None, None),
        ];
        let laps = segment_laps(frames, &labels);
        assert_eq!(laps.len(), 2);
        assert_eq!(laps[0].lap_number, 1);
        assert_eq!(laps[0].sdk_lap_time_ms, Some(82_653.0));
        assert_eq!(laps[0].delta_best_ok, Some(true));
        // Final lap has no following frame => no time.
        assert_eq!(laps[1].sdk_lap_time_ms, None);
        assert_eq!(laps[1].delta_best_ok, None);
    }

    #[test]
    fn lap_time_published_after_transition() {
        let at = |lap, t, last| RawFrame {
            session_time: t,
            ..frame(0, lap, 0.5, last, Some(true))
        };
        let frames = vec![
            at(1, 0.0, Some(80.0)),
            // Lap 2 starts still showing lap 0's time; lap 1's arrives 1.5 s later.
            at(2, 100.0, Some(80.0)),
            at(2, 101.5, Some(82.5)),
            // Lap 3 starts, but no update follows within the window (reset).
            at(3, 190.0, Some(82.5)),
            at(3, 196.0, Some(79.0)),
        ];
        let laps = segment_laps(frames, &HashMap::new());
        assert_eq!(laps[0].sdk_lap_time_ms, Some(82_500.0));
        assert_eq!(laps[1].sdk_lap_time_ms, None);
    }

    #[test]
    fn negative_last_lap_time_is_unset() {
        let labels = HashMap::new();
        let frames = vec![
            frame(0, 1, 0.1, None, None),
            frame(0, 2, 0.05, Some(-1.0), Some(false)),
        ];
        let laps = segment_laps(frames, &labels);
        assert_eq!(laps[0].sdk_lap_time_ms, None);
        assert_eq!(laps[0].delta_best_ok, Some(false));
    }

    #[test]
    fn per_subsession_lap_numbering() {
        let labels = HashMap::new();
        let frames = vec![
            frame(0, 1, 0.5, None, None),
            frame(0, 2, 0.5, Some(60.0), Some(true)),
            frame(1, 1, 0.5, None, None),
            frame(1, 2, 0.5, Some(61.0), Some(true)),
        ];
        let laps = segment_laps(frames, &labels);
        assert_eq!(laps.len(), 4);
        assert_eq!(laps[0].lap_number, 1);
        assert_eq!(laps[1].lap_number, 2);
        assert_eq!(laps[2].session_num, 1);
        assert_eq!(laps[2].lap_number, 1);
    }
}
