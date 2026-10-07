//! Serde DTOs for the storage layer and the Tauri IPC boundary (`camelCase`).
//!
//! These are read/transport models. Lap facts mirror iRacing exactly: the sim's
//! `_OK` flags, pit-road samples, and distance coverage — no invented lap kind or
//! opaque "valid" flag. `paceEligible` requires both `_OK` flags and near-full
//! distance coverage (see analysis cleanup).

use serde::{Deserialize, Serialize};

pub use race_refinery_analysis::TracePoint;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub id: i64,
    pub ibt_path: String,
    pub track: String,
    pub car: String,
    pub session_date: String,
    pub lap_count: i32,
    /// Fastest pace-eligible lap, if any.
    pub best_lap_ms: Option<f64>,
    pub imported_at: String,
    /// Session type of the latest sub-session (by `session_num`), e.g. "Race". Derived, not stored.
    pub session_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SectorTime {
    pub sector_num: i32,
    pub time_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LapSummary {
    pub id: i64,
    pub session_num: i32,
    pub session_type: String,
    pub iracing_lap: i32,
    pub lap_number: i32,
    pub lap_time_ms: Option<f64>,
    /// `LapDeltaToBestLap_OK` from the sim, or `None` if the channel was absent.
    pub delta_best_ok: Option<bool>,
    pub delta_session_best_ok: Option<bool>,
    pub on_pit_road_start: bool,
    pub on_pit_road_end: bool,
    pub lap_dist_pct_min: Option<f64>,
    pub lap_dist_pct_max: Option<f64>,
    /// Derived: reported time, both `_OK` flags, and near-full coverage.
    pub pace_eligible: bool,
    pub fuel_start: Option<f64>,
    pub fuel_used: Option<f64>,
    pub avg_speed: Option<f64>,
    pub lf_temp: Option<f64>,
    pub rf_temp: Option<f64>,
    pub lr_temp: Option<f64>,
    pub rr_temp: Option<f64>,
    /// Mean tire pressures (kPa); `null` before schema v9 / without channel.
    pub lf_pressure: Option<f64>,
    pub rf_pressure: Option<f64>,
    pub lr_pressure: Option<f64>,
    pub rr_pressure: Option<f64>,
    /// True when the lap has at least one sparse traffic event.
    pub has_traffic: bool,
    pub sectors: Vec<SectorTime>,
    /// Delta to the fastest pace-eligible lap in this sub-session.
    pub delta_to_best_ms: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDetail {
    pub session: SessionSummary,
    pub laps: Vec<LapSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LapTrace {
    pub lap_id: i64,
    pub lap_number: i32,
    pub points: Vec<TracePoint>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportStatus {
    pub active: bool,
    pub current_file: Option<String>,
    pub progress_pct: f64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IracingConfigCheck {
    pub app_ini_path: String,
    pub telemetry_dir: String,
    pub mem_enabled: bool,
    pub disk_enabled: bool,
    pub warnings: Vec<String>,
}
