//! Per-lap aggregates computed directly from frames: fuel usage, tire-temp
//! averages, average speed, and downsampled traces for charts/comparison.

use super::types::{RawFrame, TracePoint};

/// Keep every Nth frame for chart traces (60 Hz IBT -> ~10 Hz at 6).
const DOWNSAMPLE_EVERY: usize = 6;

/// `(fuel_start, fuel_used)` in liters from the first/last frame fuel level.
pub fn fuel_stats(frames: &[RawFrame]) -> (Option<f64>, Option<f64>) {
    let first = frames.first().map(|f| f.fuel_level as f64);
    let last = frames.last().map(|f| f.fuel_level as f64);
    match (first, last) {
        (Some(start), Some(end)) => (Some(start), Some((start - end).max(0.0))),
        _ => (None, None),
    }
}

/// `(lf, rf, lr, rr)` mean tire temps, `None` when there are no frames.
pub fn tire_averages(frames: &[RawFrame]) -> (Option<f64>, Option<f64>, Option<f64>, Option<f64>) {
    if frames.is_empty() {
        return (None, None, None, None);
    }
    let n = frames.len() as f64;
    let sum = frames.iter().fold((0.0, 0.0, 0.0, 0.0), |acc, f| {
        (
            acc.0 + f.lf_temp as f64,
            acc.1 + f.rf_temp as f64,
            acc.2 + f.lr_temp as f64,
            acc.3 + f.rr_temp as f64,
        )
    });
    (
        Some(sum.0 / n),
        Some(sum.1 / n),
        Some(sum.2 / n),
        Some(sum.3 / n),
    )
}

/// `(lf, rf, lr, rr)` mean tire pressures (kPa), `None` when no sample carried pressure.
pub fn tire_pressure_averages(
    frames: &[RawFrame],
) -> (Option<f64>, Option<f64>, Option<f64>, Option<f64>) {
    fn mean(values: impl Iterator<Item = f64>) -> Option<f64> {
        let mut sum = 0.0;
        let mut n = 0usize;
        for v in values {
            sum += v;
            n += 1;
        }
        (n > 0).then_some(sum / n as f64)
    }
    (
        mean(frames.iter().filter_map(|f| f.lf_pressure.map(f64::from))),
        mean(frames.iter().filter_map(|f| f.rf_pressure.map(f64::from))),
        mean(frames.iter().filter_map(|f| f.lr_pressure.map(f64::from))),
        mean(frames.iter().filter_map(|f| f.rr_pressure.map(f64::from))),
    )
}

/// Mean speed (m/s), `None` when there are no frames.
pub fn average_speed(frames: &[RawFrame]) -> Option<f64> {
    if frames.is_empty() {
        return None;
    }
    let sum: f64 = frames.iter().map(|f| f.speed as f64).sum();
    Some(sum / frames.len() as f64)
}

/// Downsample frames into chart trace points. Each point takes its first
/// frame's values; `abs_active` is OR-ed over the frames it stands for so short
/// ABS pulses between kept frames aren't lost.
pub fn downsample_traces(frames: &[RawFrame]) -> Vec<TracePoint> {
    let lap_start = frames.first().map(|f| f.session_time);
    frames
        .chunks(DOWNSAMPLE_EVERY)
        .map(|chunk| {
            let f = &chunk[0];
            TracePoint {
                abs_active: f
                    .abs_active
                    .map(|_| chunk.iter().any(|c| c.abs_active == Some(true))),
                elapsed_ms: lap_start.map(|t0| (f.session_time - t0) * 1000.0),
                dist_pct: f.lap_dist_pct as f64,
                speed: f.speed as f64,
                throttle: f.throttle as f64,
                brake: f.brake as f64,
                throttle_raw: f.throttle_raw.map(f64::from),
                brake_raw: f.brake_raw.map(f64::from),
                clutch: f.clutch.map(f64::from),
                clutch_raw: f.clutch_raw.map(f64::from),
                handbrake_raw: f.handbrake_raw.map(f64::from),
                gear: f.gear,
                steering: f.steering as f64,
                lat: f.lat,
                lon: f.lon,
                rpm: f.rpm.map(f64::from),
                lat_accel: f.lat_accel.map(f64::from),
                long_accel: f.long_accel.map(f64::from),
                yaw_rate: f.yaw_rate.map(f64::from),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(fuel: f32, speed: f32, temp: f32) -> RawFrame {
        RawFrame {
            lap: 1,
            speed,
            gear: 3,
            fuel_level: fuel,
            lf_temp: temp,
            rf_temp: temp,
            lr_temp: temp,
            rr_temp: temp,
            ..Default::default()
        }
    }

    #[test]
    fn fuel_used_is_start_minus_end() {
        let frames = vec![frame(50.0, 0.0, 0.0), frame(48.5, 0.0, 0.0)];
        let (start, used) = fuel_stats(&frames);
        assert_eq!(start, Some(50.0));
        assert!((used.unwrap() - 1.5).abs() < 1e-6);
    }

    #[test]
    fn average_speed_and_temps() {
        let frames = vec![frame(50.0, 40.0, 80.0), frame(50.0, 60.0, 90.0)];
        assert_eq!(average_speed(&frames), Some(50.0));
        let (lf, _, _, _) = tire_averages(&frames);
        assert_eq!(lf, Some(85.0));
    }

    #[test]
    fn downsample_copies_raw_pedals_gps_and_elapsed() {
        let mut frames: Vec<RawFrame> = (0..6).map(|_| frame(50.0, 40.0, 80.0)).collect();
        frames[0].session_time = 10.0;
        frames[0].throttle_raw = Some(0.5);
        frames[0].brake_raw = Some(0.25);
        frames[0].clutch = Some(0.125);
        frames[0].clutch_raw = Some(0.0625);
        frames[0].handbrake_raw = Some(0.0);
        frames[0].lat = Some(41.0);
        frames[0].lon = Some(-88.0);
        frames[3].session_time = 10.5;
        let points = downsample_traces(&frames);
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].throttle_raw, Some(0.5));
        assert_eq!(points[0].brake_raw, Some(0.25));
        assert_eq!(points[0].clutch, Some(0.125));
        assert_eq!(points[0].clutch_raw, Some(0.0625));
        assert_eq!(points[0].handbrake_raw, Some(0.0));
        assert_eq!(points[0].lat, Some(41.0));
        assert_eq!(points[0].lon, Some(-88.0));
        assert_eq!(points[0].elapsed_ms, Some(0.0));
    }

    #[test]
    fn abs_pulse_between_kept_frames_survives_downsampling() {
        let mut frames: Vec<RawFrame> = (0..12)
            .map(|_| RawFrame {
                abs_active: Some(false),
                ..frame(50.0, 40.0, 80.0)
            })
            .collect();
        frames[3].abs_active = Some(true);
        let points = downsample_traces(&frames);
        assert_eq!(points.len(), 2);
        assert_eq!(points[0].abs_active, Some(true));
        assert_eq!(points[1].abs_active, Some(false));
        assert_eq!(
            downsample_traces(&[frame(50.0, 40.0, 80.0)])[0].abs_active,
            None
        );
    }
}
