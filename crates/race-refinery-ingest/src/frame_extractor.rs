//! Pre-resolved variable offsets for fast per-frame extraction (no allocation).

use anyhow::{Context, Result};
use pitwall::{VarData, VariableInfo, VariableSchema};

use race_refinery_analysis::RawFrame;

pub struct FastFrameExtractor {
    session_num: Option<VariableInfo>,
    lap: VariableInfo,
    lap_dist_pct: VariableInfo,
    speed: VariableInfo,
    throttle: VariableInfo,
    brake: VariableInfo,
    steering: VariableInfo,
    gear: VariableInfo,
    fuel_level: VariableInfo,
    on_pit_road: VariableInfo,
    session_time: VariableInfo,
    // Optional: present in modern IBT files, absent in older ones.
    lap_last_lap_time: Option<VariableInfo>,
    delta_best_ok: Option<VariableInfo>,
    delta_session_best_ok: Option<VariableInfo>,
    lat: Option<VariableInfo>,
    lon: Option<VariableInfo>,
    throttle_raw: Option<VariableInfo>,
    brake_raw: Option<VariableInfo>,
    clutch: Option<VariableInfo>,
    clutch_raw: Option<VariableInfo>,
    handbrake_raw: Option<VariableInfo>,
    abs_active: Option<VariableInfo>,
    rpm: Option<VariableInfo>,
    lat_accel: Option<VariableInfo>,
    long_accel: Option<VariableInfo>,
    yaw_rate: Option<VariableInfo>,
    lf_temp: Option<VariableInfo>,
    rf_temp: Option<VariableInfo>,
    lr_temp: Option<VariableInfo>,
    rr_temp: Option<VariableInfo>,
    lf_pressure: Option<VariableInfo>,
    rf_pressure: Option<VariableInfo>,
    lr_pressure: Option<VariableInfo>,
    rr_pressure: Option<VariableInfo>,
}

impl FastFrameExtractor {
    pub fn from_schema(schema: &VariableSchema) -> Result<Self> {
        fn req(schema: &VariableSchema, name: &str) -> Result<VariableInfo> {
            schema
                .get_variable(name)
                .cloned()
                .with_context(|| format!("telemetry variable '{name}' not found in IBT"))
        }

        fn tire_temp(schema: &VariableSchema, mid: &str, carcass: &str) -> Option<VariableInfo> {
            schema
                .get_variable(mid)
                .or_else(|| schema.get_variable(carcass))
                .cloned()
        }

        fn tire_pressure(schema: &VariableSchema, cold: &str, hot: &str) -> Option<VariableInfo> {
            schema
                .get_variable(cold)
                .or_else(|| schema.get_variable(hot))
                .cloned()
        }

        Ok(Self {
            session_num: schema.get_variable("SessionNum").cloned(),
            lap: req(schema, "Lap")?,
            lap_dist_pct: req(schema, "LapDistPct")?,
            speed: req(schema, "Speed")?,
            throttle: req(schema, "Throttle")?,
            brake: req(schema, "Brake")?,
            steering: req(schema, "SteeringWheelAngle")?,
            gear: req(schema, "Gear")?,
            fuel_level: req(schema, "FuelLevel")?,
            on_pit_road: req(schema, "OnPitRoad")?,
            session_time: req(schema, "SessionTime")?,
            lap_last_lap_time: schema.get_variable("LapLastLapTime").cloned(),
            delta_best_ok: schema.get_variable("LapDeltaToBestLap_OK").cloned(),
            delta_session_best_ok: schema.get_variable("LapDeltaToSessionBestLap_OK").cloned(),
            lat: schema.get_variable("Lat").cloned(),
            lon: schema.get_variable("Lon").cloned(),
            throttle_raw: schema.get_variable("ThrottleRaw").cloned(),
            brake_raw: schema.get_variable("BrakeRaw").cloned(),
            clutch: schema.get_variable("Clutch").cloned(),
            clutch_raw: schema.get_variable("ClutchRaw").cloned(),
            handbrake_raw: schema.get_variable("HandbrakeRaw").cloned(),
            abs_active: schema.get_variable("BrakeABSactive").cloned(),
            rpm: schema.get_variable("RPM").cloned(),
            lat_accel: schema.get_variable("LatAccel").cloned(),
            long_accel: schema.get_variable("LongAccel").cloned(),
            yaw_rate: schema.get_variable("YawRate").cloned(),
            lf_temp: tire_temp(schema, "LFtempM", "LFtempCM"),
            rf_temp: tire_temp(schema, "RFtempM", "RFtempCM"),
            lr_temp: tire_temp(schema, "LRtempM", "LRtempCM"),
            rr_temp: tire_temp(schema, "RRtempM", "RRtempCM"),
            // Cold preferred, then hot / carcass-style fallbacks used by some cars.
            lf_pressure: tire_pressure(schema, "LFcoldPressure", "LFpressure"),
            rf_pressure: tire_pressure(schema, "RFcoldPressure", "RFpressure"),
            lr_pressure: tire_pressure(schema, "LRcoldPressure", "LRpressure"),
            rr_pressure: tire_pressure(schema, "RRcoldPressure", "RRpressure"),
        })
    }

    #[inline]
    pub fn extract(&self, data: &[u8]) -> RawFrame {
        RawFrame {
            session_num: self
                .session_num
                .as_ref()
                .map(|v| read_i32(data, v))
                .unwrap_or(0),
            lap: read_i32(data, &self.lap),
            lap_dist_pct: read_f32(data, &self.lap_dist_pct),
            speed: read_f32(data, &self.speed),
            throttle: read_f32(data, &self.throttle),
            brake: read_f32(data, &self.brake),
            throttle_raw: self.throttle_raw.as_ref().map(|v| read_f32(data, v)),
            brake_raw: self.brake_raw.as_ref().map(|v| read_f32(data, v)),
            clutch: self.clutch.as_ref().map(|v| read_f32(data, v)),
            clutch_raw: self.clutch_raw.as_ref().map(|v| read_f32(data, v)),
            handbrake_raw: self.handbrake_raw.as_ref().map(|v| read_f32(data, v)),
            abs_active: self.abs_active.as_ref().map(|v| read_bool(data, v)),
            steering: read_f32(data, &self.steering),
            gear: read_i32(data, &self.gear),
            rpm: self.rpm.as_ref().map(|v| read_f32(data, v)),
            lat_accel: self.lat_accel.as_ref().map(|v| read_f32(data, v)),
            long_accel: self.long_accel.as_ref().map(|v| read_f32(data, v)),
            yaw_rate: self.yaw_rate.as_ref().map(|v| read_f32(data, v)),
            fuel_level: read_f32(data, &self.fuel_level),
            on_pit_road: read_bool(data, &self.on_pit_road),
            session_time: read_f64(data, &self.session_time),
            lap_last_lap_time: self.lap_last_lap_time.as_ref().map(|v| read_f32(data, v)),
            delta_best_ok: self.delta_best_ok.as_ref().map(|v| read_bool(data, v)),
            delta_session_best_ok: self
                .delta_session_best_ok
                .as_ref()
                .map(|v| read_bool(data, v)),
            lat: self.lat.as_ref().map(|v| read_f64(data, v)),
            lon: self.lon.as_ref().map(|v| read_f64(data, v)),
            lf_temp: self
                .lf_temp
                .as_ref()
                .map(|v| read_f32(data, v))
                .unwrap_or(0.0),
            rf_temp: self
                .rf_temp
                .as_ref()
                .map(|v| read_f32(data, v))
                .unwrap_or(0.0),
            lr_temp: self
                .lr_temp
                .as_ref()
                .map(|v| read_f32(data, v))
                .unwrap_or(0.0),
            rr_temp: self
                .rr_temp
                .as_ref()
                .map(|v| read_f32(data, v))
                .unwrap_or(0.0),
            lf_pressure: self.lf_pressure.as_ref().map(|v| read_f32(data, v)),
            rf_pressure: self.rf_pressure.as_ref().map(|v| read_f32(data, v)),
            lr_pressure: self.lr_pressure.as_ref().map(|v| read_f32(data, v)),
            rr_pressure: self.rr_pressure.as_ref().map(|v| read_f32(data, v)),
        }
    }
}

#[inline]
fn read_f32(data: &[u8], info: &VariableInfo) -> f32 {
    f32::from_bytes(data, info).unwrap_or(0.0)
}

#[inline]
fn read_f64(data: &[u8], info: &VariableInfo) -> f64 {
    f64::from_bytes(data, info).unwrap_or(0.0)
}

#[inline]
fn read_i32(data: &[u8], info: &VariableInfo) -> i32 {
    i32::from_bytes(data, info).unwrap_or(0)
}

#[inline]
fn read_bool(data: &[u8], info: &VariableInfo) -> bool {
    bool::from_bytes(data, info).unwrap_or(false)
}
