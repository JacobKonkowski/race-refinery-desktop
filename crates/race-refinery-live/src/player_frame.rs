use pitwall::PitwallFrame;

/// Player-only live telemetry frame (10 Hz). Extra channels keep the SDK
/// subscription aligned with IBT layout even when unused by the tracker.
#[derive(Debug, Clone, PitwallFrame)]
#[allow(dead_code)]
pub struct AnalysisFrame {
    #[field_name = "SessionNum"]
    pub session_num: i32,
    #[field_name = "Lap"]
    pub lap: i32,
    #[field_name = "LapDistPct"]
    pub lap_dist_pct: f32,
    #[field_name = "Speed"]
    pub speed: f32,
    #[field_name = "Throttle"]
    pub throttle: f32,
    #[field_name = "Brake"]
    pub brake: f32,
    /// Driver pedals before auto-blip / TC / ABS; `None` when the sim omits them.
    #[field_name = "ThrottleRaw"]
    pub throttle_raw: Option<f32>,
    #[field_name = "BrakeRaw"]
    pub brake_raw: Option<f32>,
    #[field_name = "Clutch"]
    pub clutch: Option<f32>,
    #[field_name = "ClutchRaw"]
    pub clutch_raw: Option<f32>,
    #[field_name = "HandbrakeRaw"]
    pub handbrake_raw: Option<f32>,
    #[field_name = "SteeringWheelAngle"]
    pub steering: f32,
    #[field_name = "Gear"]
    pub gear: i32,
    #[field_name = "FuelLevel"]
    pub fuel_level: f32,
    #[field_name = "OnPitRoad"]
    pub on_pit_road: bool,
    #[field_name = "SessionTime"]
    pub session_time: f64,
    /// `None` when the sim build omits GPS; the live trail then falls back to
    /// lap-distance placement on the cached outline.
    #[field_name = "Lat"]
    pub lat: Option<f64>,
    #[field_name = "Lon"]
    pub lon: Option<f64>,
    #[field_name = "LFtempM"]
    pub lf_temp: f32,
    #[field_name = "RFtempM"]
    pub rf_temp: f32,
    #[field_name = "LRtempM"]
    pub lr_temp: f32,
    #[field_name = "RRtempM"]
    pub rr_temp: f32,
}
