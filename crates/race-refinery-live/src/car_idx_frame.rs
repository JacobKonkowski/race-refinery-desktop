use pitwall::{BitField, PitwallFrame};

/// Live multi-car and session-wide telemetry, subscribed separately from the
/// player-only [`AnalysisFrame`](crate::player_frame::AnalysisFrame) so the IBT
/// import path stays lean. All fields are optional or default-on-missing so a
/// connection never fails if iRacing omits a variable.
#[derive(Debug, Clone, PitwallFrame)]
pub struct CarIdxFrame {
    #[field_name = "PlayerCarIdx"]
    pub player_car_idx: i32,

    // Per-car arrays, indexed by car_idx (iRacing reports 64 slots).
    #[field_name = "CarIdxBestLapTime"]
    pub car_idx_best_lap_time: Vec<f32>,
    #[field_name = "CarIdxLastLapTime"]
    pub car_idx_last_lap_time: Vec<f32>,
    #[field_name = "CarIdxPosition"]
    pub position: Vec<i32>,
    #[field_name = "CarIdxClassPosition"]
    pub class_position: Vec<i32>,
    #[field_name = "CarIdxOnPitRoad"]
    pub on_pit_road: Vec<bool>,
    #[field_name = "CarIdxF2Time"]
    pub f2_time: Vec<f32>,
    #[field_name = "CarIdxLapDistPct"]
    pub lap_dist_pct: Vec<f32>,

    // Player lap clock (seconds; negative sentinel when unset).
    #[field_name = "LapCurrentLapTime"]
    pub current_lap_time: f32,
    #[field_name = "LapLastLapTime"]
    pub player_last_lap_time: f32,
    #[field_name = "LapBestLapTime"]
    pub player_best_lap_time: f32,
    #[field_name = "SessionBestLapTime"]
    pub session_best_lap_time: f32,

    // Player session deltas (valid only when the matching `_OK` flag is set).
    #[field_name = "LapDeltaToBestLap"]
    pub delta_best: f32,
    #[field_name = "LapDeltaToBestLap_OK"]
    pub delta_best_ok: bool,
    #[field_name = "LapDeltaToLastLap"]
    pub delta_last: f32,
    #[field_name = "LapDeltaToLastLap_OK"]
    pub delta_last_ok: bool,
    #[field_name = "LapDeltaToSessionBestLap"]
    pub delta_session_best: f32,
    #[field_name = "LapDeltaToSessionBestLap_OK"]
    pub delta_session_best_ok: bool,
    #[field_name = "LapDeltaToSessionOptimalLap"]
    pub delta_session_optimal: f32,
    #[field_name = "LapDeltaToSessionOptimalLap_OK"]
    pub delta_session_optimal_ok: bool,

    // Session-wide state used by the audio coach and HUD.
    #[field_name = "SessionFlags"]
    pub session_flags: Option<BitField>,
    // iRacing reports `CarLeftRight` as an Int32 enum (irsdk_CarLeftRight), not a
    // bitfield. Decoding it as `BitField` silently failed (always None -> Off),
    // breaking spotter pack alerts; read it as the int it actually is.
    #[field_name = "CarLeftRight"]
    pub car_left_right: i32,
    #[field_name = "PlayerCarMyIncidentCount"]
    pub incident_count: i32,
    #[field_name = "SessionTimeRemain"]
    pub session_time_remain: f64,
    #[field_name = "SessionLapsRemain"]
    pub session_laps_remain: i32,
    #[field_name = "PitsOpen"]
    pub pits_open: bool,
    #[field_name = "IsOnTrack"]
    pub on_track: bool,
}

impl CarIdxFrame {
    /// Raw `SessionFlags` bitfield value (0 when unavailable).
    pub fn session_flags_value(&self) -> u32 {
        self.session_flags.map(|b| b.value()).unwrap_or(0)
    }

    /// Raw `CarLeftRight` enum value (0 / Off when unavailable).
    pub fn car_left_right_value(&self) -> u32 {
        self.car_left_right.max(0) as u32
    }

    /// iRacing's assessment of whether the just-completed lap is valid. Any
    /// `_OK` flag flipping false means the lap was invalidated (track limits, etc.).
    pub fn completed_lap_ok(&self) -> bool {
        self.delta_best_ok && self.delta_session_best_ok
    }
}
