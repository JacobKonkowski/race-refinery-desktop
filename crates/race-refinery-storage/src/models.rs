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

/// One lap in the explorer tree. Display fields only; the open session still loads full laps.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryLap {
    pub id: i64,
    pub lap_number: i32,
    pub lap_time_ms: Option<f64>,
    pub pace_eligible: bool,
    pub session_type: String,
    pub on_pit_road_start: bool,
    pub on_pit_road_end: bool,
    /// Versus the fastest pace-eligible lap in this lap's own sub-session.
    pub delta_to_best_ms: Option<f64>,
}

/// A session plus the laps shown under it in the explorer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySession {
    pub session: SessionSummary,
    pub laps: Vec<LibraryLap>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LapSetSummary {
    pub id: i64,
    pub name: String,
    pub track: String,
    pub created_at: String,
    pub lap_count: i32,
}

/// A member lap with the outing it came from. `lap.delta_to_best_ms` is versus the
/// fastest pace-eligible lap in the set, not the lap's original sub-session.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetLap {
    pub session_id: i64,
    pub car: String,
    pub session_date: String,
    pub lap: LapSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LapSetDetail {
    pub set: LapSetSummary,
    pub laps: Vec<SetLap>,
}

/// Optional filters, combined with AND. Empty or missing fields are ignored.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryRule {
    pub track: Option<String>,
    pub car: Option<String>,
    pub session_type: Option<String>,
    pub pace_eligible_only: Option<bool>,
    pub exclude_pit: Option<bool>,
    /// Lap time minus that lap's own sub-session best, in milliseconds.
    pub max_delta_to_best_ms: Option<f64>,
    /// Session date on or after this day (`YYYY-MM-DD` compares against RFC3339).
    pub since: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LapCategory {
    pub id: i64,
    pub name: String,
    pub rule: CategoryRule,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryMatch {
    pub session_id: i64,
    pub track: String,
    pub car: String,
    pub session_date: String,
    pub lap: LapSummary,
}

fn filled(value: &Option<String>) -> Option<&str> {
    value.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

/// Whether a cleaned lap satisfies every set field on the rule.
pub fn lap_matches_rule(rule: &CategoryRule, session: &SessionSummary, lap: &LapSummary) -> bool {
    if let Some(track) = filled(&rule.track) {
        if session.track != track {
            return false;
        }
    }
    if let Some(car) = filled(&rule.car) {
        if session.car != car {
            return false;
        }
    }
    if let Some(since) = filled(&rule.since) {
        if session.session_date.as_str() < since {
            return false;
        }
    }
    if let Some(session_type) = filled(&rule.session_type) {
        if lap.session_type != session_type {
            return false;
        }
    }
    if rule.pace_eligible_only == Some(true) && !lap.pace_eligible {
        return false;
    }
    if rule.exclude_pit == Some(true) && (lap.on_pit_road_start || lap.on_pit_road_end) {
        return false;
    }
    if let Some(max) = rule.max_delta_to_best_ms {
        match lap.delta_to_best_ms {
            Some(delta) if delta <= max => {}
            _ => return false,
        }
    }
    true
}

/// Session-level filters only, so a category query can skip loading other outings.
pub fn session_passes_rule(rule: &CategoryRule, session: &SessionSummary) -> bool {
    if let Some(track) = filled(&rule.track) {
        if session.track != track {
            return false;
        }
    }
    if let Some(car) = filled(&rule.car) {
        if session.car != car {
            return false;
        }
    }
    if let Some(since) = filled(&rule.since) {
        if session.session_date.as_str() < since {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(track: &str, car: &str, date: &str) -> SessionSummary {
        SessionSummary {
            id: 1,
            ibt_path: String::new(),
            track: track.into(),
            car: car.into(),
            session_date: date.into(),
            lap_count: 1,
            best_lap_ms: Some(90_000.0),
            imported_at: String::new(),
            session_type: "Race".into(),
        }
    }

    fn lap(session_type: &str, pace: bool, pit: bool, delta: Option<f64>) -> LapSummary {
        LapSummary {
            id: 1,
            session_num: 0,
            session_type: session_type.into(),
            iracing_lap: 1,
            lap_number: 1,
            lap_time_ms: Some(90_000.0),
            delta_best_ok: Some(true),
            delta_session_best_ok: Some(true),
            on_pit_road_start: pit,
            on_pit_road_end: false,
            lap_dist_pct_min: Some(0.0),
            lap_dist_pct_max: Some(1.0),
            pace_eligible: pace,
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
            has_traffic: false,
            sectors: Vec::new(),
            delta_to_best_ms: delta,
        }
    }

    #[test]
    fn empty_rule_matches_any_lap() {
        let rule = CategoryRule::default();
        assert!(lap_matches_rule(
            &rule,
            &session("Spa", "Ferrari", "2026-04-01T12:00:00+00:00"),
            &lap("Race", false, true, None),
        ));
    }

    #[test]
    fn rule_fields_combine() {
        let rule = CategoryRule {
            track: Some("Spa".into()),
            car: Some(" ".into()),
            session_type: Some("Race".into()),
            pace_eligible_only: Some(true),
            exclude_pit: Some(true),
            max_delta_to_best_ms: Some(500.0),
            since: Some("2026-03-01".into()),
        };
        let outing = session("Spa", "Ferrari", "2026-04-01T12:00:00+00:00");
        assert!(lap_matches_rule(
            &rule,
            &outing,
            &lap("Race", true, false, Some(200.0))
        ));
        assert!(!lap_matches_rule(
            &rule,
            &outing,
            &lap("Practice", true, false, Some(200.0))
        ));
        assert!(!lap_matches_rule(
            &rule,
            &outing,
            &lap("Race", false, false, Some(200.0))
        ));
        assert!(!lap_matches_rule(
            &rule,
            &outing,
            &lap("Race", true, true, Some(200.0))
        ));
        assert!(!lap_matches_rule(
            &rule,
            &outing,
            &lap("Race", true, false, Some(800.0))
        ));
        assert!(!lap_matches_rule(
            &rule,
            &outing,
            &lap("Race", true, false, None)
        ));
        let earlier = session("Spa", "Ferrari", "2026-01-01T12:00:00+00:00");
        assert!(!lap_matches_rule(
            &rule,
            &earlier,
            &lap("Race", true, false, Some(0.0))
        ));
        let other = session("Monza", "Ferrari", "2026-04-01T12:00:00+00:00");
        assert!(!session_passes_rule(&rule, &other));
    }
}
