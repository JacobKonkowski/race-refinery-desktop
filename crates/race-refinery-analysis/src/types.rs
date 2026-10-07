//! Analysis-owned domain types.
//!
//! An [`AnalyzedLap`] carries only iRacing-native facts plus values computed by
//! math on those facts (sectors, trace samples, aggregates). There is no
//! invented lap taxonomy and no heuristic validity flag.

use serde::{Deserialize, Serialize};

pub use race_refinery_telemetry::{RawFrame, SectorBoundary, SessionMeta};

/// A downsampled telemetry point kept for charts and comparison.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TracePoint {
    pub dist_pct: f64,
    pub speed: f64,
    pub throttle: f64,
    pub brake: f64,
    pub gear: i32,
    pub steering: f64,
    /// `Lat` at this sample. `None` for sessions imported before GPS was kept
    /// on traces, or from IBTs without the channel — the racing line is then
    /// unavailable and consumers fall back to lap-distance placement.
    pub lat: Option<f64>,
    /// `Lon` at this sample.
    pub lon: Option<f64>,
    /// Driver pedal inputs (`ThrottleRaw`, `BrakeRaw`, …). `None` for sessions
    /// imported before schema v5 or sources without the channel; driver-intent
    /// consumers then fall back to the applied `throttle` / `brake`.
    #[serde(default)]
    pub throttle_raw: Option<f64>,
    #[serde(default)]
    pub brake_raw: Option<f64>,
    /// Applied clutch (`Clutch`).
    #[serde(default)]
    pub clutch: Option<f64>,
    #[serde(default)]
    pub clutch_raw: Option<f64>,
    #[serde(default)]
    pub handbrake_raw: Option<f64>,
    /// `BrakeABSactive` at any frame folded into this sample. `None` before
    /// schema v6 or when the IBT lacks the channel.
    #[serde(default)]
    pub abs_active: Option<bool>,
    /// Milliseconds since the lap's first frame (`SessionTime` delta). `None`
    /// for sessions imported before the time channel was kept; comparisons then
    /// estimate time from speed.
    #[serde(default)]
    pub elapsed_ms: Option<f64>,
    /// Engine RPM. `None` before schema v7 or when the IBT lacks the channel.
    #[serde(default)]
    pub rpm: Option<f64>,
    /// Lateral / longitudinal acceleration (m/s²) and yaw rate (rad/s).
    #[serde(default)]
    pub lat_accel: Option<f64>,
    #[serde(default)]
    pub long_accel: Option<f64>,
    #[serde(default)]
    pub yaw_rate: Option<f64>,
}

impl TracePoint {
    /// Throttle the driver asked for: raw when stored, else applied.
    pub fn driver_throttle(&self) -> f64 {
        self.throttle_raw.unwrap_or(self.throttle)
    }

    /// Brake the driver asked for: raw when stored, else applied.
    pub fn driver_brake(&self) -> f64 {
        self.brake_raw.unwrap_or(self.brake)
    }

    /// Clutch the driver asked for: raw when stored, else applied.
    pub fn driver_clutch(&self) -> Option<f64> {
        self.clutch_raw.or(self.clutch)
    }
}

/// Frames grouped into a single lap, with the SDK values sampled at the
/// transition into the following lap.
#[derive(Debug, Clone)]
pub struct LapFrames {
    pub session_num: i32,
    pub session_type: String,
    pub iracing_lap: i32,
    /// 1-based index within this sub-session, assigned after segmentation.
    pub lap_number: i32,
    /// `LapLastLapTime` (ms) as published shortly after the lap ended.
    pub sdk_lap_time_ms: Option<f64>,
    /// `LapDeltaToBestLap_OK` sampled at that same transition.
    pub delta_best_ok: Option<bool>,
    /// `LapDeltaToSessionBestLap_OK` at the transition.
    pub delta_session_best_ok: Option<bool>,
    pub frames: Vec<RawFrame>,
}

/// A fully analyzed lap: SDK facts + computed products. No `valid`/`lap_kind`.
#[derive(Debug, Clone)]
pub struct AnalyzedLap {
    pub session_num: i32,
    pub session_type: String,
    pub iracing_lap: i32,
    pub lap_number: i32,
    /// Official lap time (`LapLastLapTime`); `None` when the sim did not report one
    /// (e.g. the final, unfinished lap).
    pub lap_time_ms: Option<f64>,
    pub delta_best_ok: Option<bool>,
    pub delta_session_best_ok: Option<bool>,
    /// `OnPitRoad` on the first frame of the lap.
    pub on_pit_road_start: bool,
    /// `OnPitRoad` on the last frame of the lap.
    pub on_pit_road_end: bool,
    pub lap_dist_pct_min: f32,
    pub lap_dist_pct_max: f32,
    pub fuel_start: Option<f64>,
    pub fuel_used: Option<f64>,
    pub avg_speed: Option<f64>,
    pub lf_temp: Option<f64>,
    pub rf_temp: Option<f64>,
    pub lr_temp: Option<f64>,
    pub rr_temp: Option<f64>,
    /// Mean tire pressures (kPa) when the IBT carried pressure channels.
    pub lf_pressure: Option<f64>,
    pub rf_pressure: Option<f64>,
    pub lr_pressure: Option<f64>,
    pub rr_pressure: Option<f64>,
    /// `(sector_num, time_ms)` pairs.
    pub sectors: Vec<(i32, f64)>,
    pub traces: Vec<TracePoint>,
    /// Sparse distance samples where another car was nearby (not on pit road).
    pub traffic_events: Vec<TrafficEvent>,
}

/// A stretch of the lap where traffic was close enough to matter for coaching.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TrafficEvent {
    pub dist_pct: f64,
    /// `"nearby"` — another car within the proximity threshold.
    pub kind: String,
}

impl AnalyzedLap {
    /// Whether this lap counts for the session best / default compare reference.
    ///
    /// Requires an official time, both sim `_OK` flags, and near-full distance
    /// coverage so incomplete/reset fragments never become the "best" lap.
    pub fn pace_eligible(&self) -> bool {
        super::cleanup::pace_eligible_from(
            self.lap_time_ms,
            self.delta_best_ok,
            self.delta_session_best_ok,
            self.lap_dist_pct_max,
        )
    }

    pub fn is_phantom(&self) -> bool {
        super::cleanup::is_phantom_lap(self.iracing_lap, self.lap_dist_pct_max)
    }
}

/// Result of analyzing a whole IBT file.
#[derive(Debug, Clone)]
pub struct AnalyzedSession {
    pub track: String,
    pub car: String,
    pub session_date: String,
    pub laps: Vec<AnalyzedLap>,
    /// Circuit outline generated from this session's GPS samples, when the
    /// source carried `Lat` / `Lon` and one lap covered the track.
    pub track_map: Option<super::track_map::TrackOutline>,
}

impl AnalyzedSession {
    /// Fastest pace-eligible lap time across all sub-sessions, if any.
    pub fn best_lap_ms(&self) -> Option<f64> {
        self.laps
            .iter()
            .filter(|l| l.pace_eligible())
            .filter_map(|l| l.lap_time_ms)
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
    }
}
