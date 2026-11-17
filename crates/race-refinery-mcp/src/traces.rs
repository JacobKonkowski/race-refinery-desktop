//! Downsampled lap channel traces for MCP (`get_lap_traces`).

use race_refinery_analysis::TracePoint;
use serde::Serialize;

/// Default sample count per channel when the client omits `maxPoints`.
pub const DEFAULT_MAX_POINTS: u32 = 200;
/// Hard ceiling for `maxPoints`.
pub const HARD_MAX_POINTS: u32 = 500;
/// Reject requests that would emit more than this many scalar samples.
pub const HARD_MAX_SAMPLES: usize = 8_000;

/// Channels stored on [`TracePoint`] that MCP may expose.
pub const ALLOWED_CHANNELS: &[&str] = &[
    "brake",
    "throttle",
    "brakeRaw",
    "throttleRaw",
    "speed",
    "steering",
    "gear",
    "clutch",
    "clutchRaw",
    "rpm",
    "latAccel",
    "longAccel",
    "yawRate",
    "absActive",
    "elapsedMs",
];

const DEFAULT_CHANNELS: &[&str] = &["brake", "throttle"];

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LapTracesResult {
    pub lap_id: i64,
    pub session_id: Option<i64>,
    pub lap_number: Option<i32>,
    pub channels: Vec<String>,
    pub units: Vec<ChannelUnit>,
    pub start_dist_pct: f64,
    pub end_dist_pct: f64,
    pub source_point_count: usize,
    pub point_count: usize,
    pub max_points: u32,
    pub downsample: String,
    /// Shared distance axis (0–1 lap fraction) for every channel series.
    pub dist_pct: Vec<f64>,
    /// Parallel to `channels`; each series is the same length as `dist_pct`.
    pub series: Vec<Vec<Option<f64>>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelUnit {
    pub channel: String,
    pub unit: String,
}

pub fn normalize_channels(requested: Option<Vec<String>>) -> Result<Vec<String>, String> {
    let raw = match requested {
        None => DEFAULT_CHANNELS
            .iter()
            .map(|s| (*s).to_string())
            .collect::<Vec<_>>(),
        Some(list) if list.is_empty() => DEFAULT_CHANNELS
            .iter()
            .map(|s| (*s).to_string())
            .collect::<Vec<_>>(),
        Some(list) => list,
    };

    let mut out = Vec::new();
    for ch in raw {
        let key = normalize_channel_name(&ch)?;
        if !out.iter().any(|e| e == &key) {
            out.push(key);
        }
    }
    if out.is_empty() {
        return Err("channels must include at least one allowlisted name".into());
    }
    Ok(out)
}

pub fn normalize_max_points(max_points: Option<u32>) -> Result<u32, String> {
    let n = max_points.unwrap_or(DEFAULT_MAX_POINTS);
    if n == 0 {
        return Err("maxPoints must be >= 1".into());
    }
    if n > HARD_MAX_POINTS {
        return Err(format!("maxPoints cannot exceed {HARD_MAX_POINTS}"));
    }
    Ok(n)
}

pub fn build_lap_traces(
    lap_id: i64,
    session_id: Option<i64>,
    lap_number: Option<i32>,
    points: &[TracePoint],
    channels: &[String],
    max_points: u32,
    start_dist_pct: Option<f64>,
    end_dist_pct: Option<f64>,
) -> Result<LapTracesResult, String> {
    let (start, end) = resolve_window(start_dist_pct, end_dist_pct)?;
    let windowed: Vec<&TracePoint> = points
        .iter()
        .filter(|p| p.dist_pct >= start && p.dist_pct <= end)
        .collect();
    let source_point_count = windowed.len();
    let sampled = downsample(&windowed, max_points as usize);
    let point_count = sampled.len();

    let sample_budget = point_count.saturating_mul(channels.len());
    if sample_budget > HARD_MAX_SAMPLES {
        return Err(format!(
            "request would emit {sample_budget} samples (max {HARD_MAX_SAMPLES}); reduce channels or maxPoints"
        ));
    }

    let units: Vec<ChannelUnit> = channels
        .iter()
        .map(|ch| ChannelUnit {
            channel: ch.clone(),
            unit: channel_unit(ch).into(),
        })
        .collect();

    let dist_pct: Vec<f64> = sampled.iter().map(|p| p.dist_pct).collect();
    let series: Vec<Vec<Option<f64>>> = channels
        .iter()
        .map(|ch| sampled.iter().map(|p| channel_value(p, ch)).collect())
        .collect();

    let downsample = if source_point_count <= point_count {
        "none".into()
    } else {
        format!("uniform({source_point_count}->{point_count})")
    };

    Ok(LapTracesResult {
        lap_id,
        session_id,
        lap_number,
        channels: channels.to_vec(),
        units,
        start_dist_pct: start,
        end_dist_pct: end,
        source_point_count,
        point_count,
        max_points,
        downsample,
        dist_pct,
        series,
    })
}

fn resolve_window(start: Option<f64>, end: Option<f64>) -> Result<(f64, f64), String> {
    let start = start.unwrap_or(0.0);
    let end = end.unwrap_or(1.0);
    if !(0.0..=1.0).contains(&start) || !(0.0..=1.0).contains(&end) {
        return Err("startDistPct/endDistPct must be in [0, 1]".into());
    }
    if end < start {
        return Err("endDistPct must be >= startDistPct".into());
    }
    Ok((start, end))
}

fn downsample<'a>(points: &[&'a TracePoint], max_points: usize) -> Vec<&'a TracePoint> {
    if points.is_empty() || max_points == 0 {
        return Vec::new();
    }
    if points.len() <= max_points {
        return points.to_vec();
    }
    if max_points == 1 {
        return vec![points[0]];
    }
    let mut out = Vec::with_capacity(max_points);
    let last = points.len() - 1;
    for i in 0..max_points {
        let idx = i * last / (max_points - 1);
        out.push(points[idx]);
    }
    out
}

fn normalize_channel_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("empty channel name".into());
    }
    let lower = trimmed.to_ascii_lowercase().replace(['_', '-'], "");
    let canonical = match lower.as_str() {
        "brake" => "brake",
        "throttle" => "throttle",
        "brakeraw" => "brakeRaw",
        "throttleraw" => "throttleRaw",
        "speed" => "speed",
        "steering" | "steer" => "steering",
        "gear" => "gear",
        "clutch" => "clutch",
        "clutchraw" => "clutchRaw",
        "rpm" => "rpm",
        "lataccel" | "latg" => "latAccel",
        "longaccel" | "longg" => "longAccel",
        "yawrate" | "yaw" => "yawRate",
        "absactive" | "abs" => "absActive",
        "elapsedms" | "time" | "elapsed" => "elapsedMs",
        other => {
            return Err(format!(
                "unsupported channel '{other}'; allowed: {}",
                ALLOWED_CHANNELS.join(", ")
            ));
        }
    };
    Ok(canonical.into())
}

fn channel_unit(channel: &str) -> &'static str {
    match channel {
        "brake" | "throttle" | "brakeRaw" | "throttleRaw" | "clutch" | "clutchRaw" => {
            "0-1 fraction"
        }
        "speed" => "m/s",
        "steering" => "rad",
        "gear" => "gear",
        "rpm" => "rpm",
        "latAccel" | "longAccel" => "m/s^2",
        "yawRate" => "rad/s",
        "absActive" => "0|1",
        "elapsedMs" => "ms",
        _ => "unknown",
    }
}

fn channel_value(point: &TracePoint, channel: &str) -> Option<f64> {
    match channel {
        "brake" => Some(point.brake),
        "throttle" => Some(point.throttle),
        "brakeRaw" => point.brake_raw,
        "throttleRaw" => point.throttle_raw,
        "speed" => Some(point.speed),
        "steering" => Some(point.steering),
        "gear" => Some(point.gear as f64),
        "clutch" => point.clutch,
        "clutchRaw" => point.clutch_raw,
        "rpm" => point.rpm,
        "latAccel" => point.lat_accel,
        "longAccel" => point.long_accel,
        "yawRate" => point.yaw_rate,
        "absActive" => point.abs_active.map(|v| if v { 1.0 } else { 0.0 }),
        "elapsedMs" => point.elapsed_ms,
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(dist: f64, brake: f64, throttle: f64) -> TracePoint {
        TracePoint {
            dist_pct: dist,
            brake,
            throttle,
            speed: 50.0,
            ..TracePoint::default()
        }
    }

    #[test]
    fn defaults_brake_throttle_and_200() {
        assert_eq!(
            normalize_channels(None).unwrap(),
            vec!["brake".to_string(), "throttle".to_string()]
        );
        assert_eq!(normalize_max_points(None).unwrap(), 200);
        assert!(normalize_max_points(Some(501)).is_err());
    }

    #[test]
    fn window_and_downsample() {
        let points: Vec<TracePoint> = (0..1000)
            .map(|i| pt(i as f64 / 999.0, i as f64 / 999.0, 0.0))
            .collect();
        let channels = vec!["brake".into()];
        let out = build_lap_traces(
            1,
            Some(2),
            Some(3),
            &points,
            &channels,
            10,
            Some(0.2),
            Some(0.4),
        )
        .unwrap();
        assert_eq!(out.point_count, 10);
        assert!(out.start_dist_pct >= 0.19 && out.end_dist_pct <= 0.41);
        assert!(out.dist_pct.first().copied().unwrap() >= 0.2 - 1e-9);
        assert!(out.dist_pct.last().copied().unwrap() <= 0.4 + 1e-9);
        assert_eq!(out.series[0].len(), 10);
        assert!(out.downsample.starts_with("uniform("));
    }

    #[test]
    fn rejects_unknown_channel() {
        assert!(normalize_channels(Some(vec!["tireTemp".into()])).is_err());
    }
}
