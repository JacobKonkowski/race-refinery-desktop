//! Sparse CarIdx proximity sampling during IBT import (not stored on every frame).

use pitwall::{VarData, VariableInfo, VariableSchema};
use race_refinery_analysis::{field_nearby, TrafficSample};

/// Sample every N frames (~0.5 s at 60 Hz) so CarIdx arrays stay cheap.
const SAMPLE_EVERY: usize = 30;

pub struct TrafficSampler {
    player_idx: Option<VariableInfo>,
    lap: Option<VariableInfo>,
    session_num: Option<VariableInfo>,
    lap_dist_pct: Option<VariableInfo>,
    car_idx_lap_dist: Option<VariableInfo>,
    car_idx_on_pit: Option<VariableInfo>,
    frame_i: usize,
    samples: Vec<TrafficSample>,
}

impl TrafficSampler {
    pub fn from_schema(schema: &VariableSchema) -> Self {
        Self {
            player_idx: schema.get_variable("PlayerCarIdx").cloned(),
            lap: schema.get_variable("Lap").cloned(),
            session_num: schema.get_variable("SessionNum").cloned(),
            lap_dist_pct: schema.get_variable("LapDistPct").cloned(),
            car_idx_lap_dist: schema.get_variable("CarIdxLapDistPct").cloned(),
            car_idx_on_pit: schema.get_variable("CarIdxOnPitRoad").cloned(),
            frame_i: 0,
            samples: Vec::new(),
        }
    }

    pub fn available(&self) -> bool {
        self.player_idx.is_some() && self.car_idx_lap_dist.is_some() && self.lap.is_some()
    }

    pub fn observe(&mut self, data: &[u8]) {
        self.frame_i += 1;
        if !self.available() || self.frame_i % SAMPLE_EVERY != 1 {
            return;
        }
        let Some(player_info) = &self.player_idx else {
            return;
        };
        let Some(dist_info) = &self.car_idx_lap_dist else {
            return;
        };
        let Some(lap_info) = &self.lap else {
            return;
        };
        let player_idx = i32::from_bytes(data, player_info).unwrap_or(-1);
        if player_idx < 0 {
            return;
        }
        let Ok(field_pct) = Vec::<f32>::from_bytes(data, dist_info) else {
            return;
        };
        let on_pit = self
            .car_idx_on_pit
            .as_ref()
            .and_then(|v| Vec::<bool>::from_bytes(data, v).ok())
            .unwrap_or_else(|| vec![false; field_pct.len()]);
        let player_pct = field_pct
            .get(player_idx as usize)
            .copied()
            .or_else(|| {
                self.lap_dist_pct
                    .as_ref()
                    .and_then(|v| f32::from_bytes(data, v).ok())
            })
            .unwrap_or(-1.0);
        let nearby = field_nearby(player_idx, player_pct, &field_pct, &on_pit);
        if !nearby {
            return;
        }
        self.samples.push(TrafficSample {
            session_num: self
                .session_num
                .as_ref()
                .and_then(|v| i32::from_bytes(data, v).ok())
                .unwrap_or(0),
            lap: i32::from_bytes(data, lap_info).unwrap_or(0),
            dist_pct: player_pct.clamp(0.0, 0.999),
            nearby: true,
        });
    }

    pub fn into_samples(self) -> Vec<TrafficSample> {
        self.samples
    }
}
