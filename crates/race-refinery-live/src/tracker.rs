use pitwall::SessionInfo;

use race_refinery_telemetry::SectorBoundary;

use super::competitors::{build_roster, RosterEntry};
use super::lap_signals::{is_flying_lap, lap_completed};
use super::player_frame::AnalysisFrame;
use super::sector_state::{
    current_sector_from_pct, display_sector_boundaries, sector_count, SectorSplitState,
};
use super::snapshot::{LiveSectorProgress, LiveSnapshot};

#[derive(Default)]
struct LapAccum {
    frames: u32,
    pit_frames: u32,
    min_dist_pct: f32,
    max_dist_pct: f32,
    start_on_pit: bool,
    end_on_pit: bool,
}

pub struct LiveTracker {
    track: String,
    car: String,
    session_type: String,
    current_lap: i32,
    lap_start_time: f64,
    sector_state: SectorSplitState,
    sector_state_active: bool,
    prev_lap_dist_pct: Option<f32>,
    prev_session_time: Option<f64>,
    /// Sectors from the lap that just ended; exposed once on the first snapshot after a lap change.
    pending_lap_sectors: Option<Vec<(i32, f64)>>,
    roster: Vec<RosterEntry>,
    player_car_idx: i32,
    lap_accum: LapAccum,
    last_finished_flying: Option<bool>,
    last_finished_completed: bool,
}

impl LiveTracker {
    pub fn new() -> Self {
        Self {
            track: String::new(),
            car: String::new(),
            session_type: String::new(),
            current_lap: 0,
            lap_start_time: 0.0,
            sector_state: SectorSplitState::new(&[], 0.0, 0.0),
            sector_state_active: false,
            prev_lap_dist_pct: None,
            prev_session_time: None,
            pending_lap_sectors: None,
            roster: Vec::new(),
            player_car_idx: -1,
            lap_accum: LapAccum::default(),
            last_finished_flying: None,
            last_finished_completed: false,
        }
    }

    pub fn track(&self) -> &str {
        &self.track
    }

    pub fn roster(&self) -> &[RosterEntry] {
        &self.roster
    }

    pub fn player_car_idx(&self) -> i32 {
        self.player_car_idx
    }

    /// Completed sector splits for the current lap (test / diagnostics).
    #[cfg(test)]
    pub fn completed_sectors(&self) -> &[(i32, f64)] {
        self.sector_state.completed_sectors()
    }

    /// Whether the most recently finished lap looked like a flying lap.
    pub fn last_finished_flying(&self) -> Option<bool> {
        self.last_finished_flying
    }

    pub fn last_finished_completed(&self) -> bool {
        self.last_finished_completed
    }

    /// Clear per-session lap and sector state. Used when the track changes
    /// (driver moved to a new session) so deltas and bests don't carry over.
    pub fn reset_session(&mut self) {
        self.current_lap = 0;
        self.lap_start_time = 0.0;
        self.sector_state = SectorSplitState::new(&[], 0.0, 0.0);
        self.sector_state_active = false;
        self.prev_lap_dist_pct = None;
        self.prev_session_time = None;
        self.pending_lap_sectors = None;
        self.lap_accum = LapAccum::default();
        self.last_finished_flying = None;
        self.last_finished_completed = false;
    }

    pub fn set_session_meta(&mut self, session: &SessionInfo) {
        self.roster = build_roster(session);
        if let Some(driver_info) = &session.driver_info {
            if let Some(idx) = driver_info.driver_car_idx {
                self.player_car_idx = idx;
            }
        }
        self.track = if session.weekend_info.track_display_name.is_empty() {
            session.weekend_info.track_name.clone()
        } else {
            session.weekend_info.track_display_name.clone()
        };
        self.car = extract_car_name(session);
        let idx = session.session_info.current_session_num;
        self.session_type = session
            .session_info
            .sessions
            .iter()
            .find(|s| s.session_num == idx)
            .and_then(|s| {
                s.session_name
                    .as_ref()
                    .filter(|n| !n.is_empty())
                    .cloned()
                    .or_else(|| Some(s.session_type.clone()))
            })
            .unwrap_or_else(|| "Session".into());
    }

    pub fn update(&mut self, frame: &AnalysisFrame, bounds: &[SectorBoundary]) {
        if self.sector_state_active {
            self.sector_state.sync_bounds(bounds, frame.lap_dist_pct);
        }

        if frame.lap != self.current_lap {
            if self.current_lap > 0 && frame.lap > self.current_lap {
                let finished = self.sector_state.finish_lap(frame.session_time);
                self.pending_lap_sectors = Some(finished);
                let completed =
                    lap_completed(self.lap_accum.min_dist_pct, self.lap_accum.max_dist_pct);
                let pit_ratio = if self.lap_accum.frames > 0 {
                    self.lap_accum.pit_frames as f64 / self.lap_accum.frames as f64
                } else {
                    0.0
                };
                self.last_finished_flying = Some(is_flying_lap(
                    pit_ratio,
                    completed,
                    self.lap_accum.start_on_pit,
                    self.lap_accum.end_on_pit,
                ));
                self.last_finished_completed = completed;
            }
            self.current_lap = frame.lap;
            self.lap_start_time = frame.session_time;
            // Lap counter can increment before LapDistPct wraps; treat high pct as lap start.
            let start_pct = if frame.lap_dist_pct > 0.9 {
                0.0
            } else {
                frame.lap_dist_pct
            };
            self.sector_state
                .reset_lap(bounds, frame.session_time, start_pct);
            self.sector_state_active = true;
            self.lap_accum = LapAccum::default();
            self.prev_lap_dist_pct = Some(frame.lap_dist_pct);
            self.prev_session_time = Some(frame.session_time);
            self.record_lap_frame(frame);
            return;
        }

        self.record_lap_frame(frame);

        if let Some(prev_pct) = self.prev_lap_dist_pct {
            let prev_time = self.prev_session_time.unwrap_or(frame.session_time);
            self.sector_state
                .advance(prev_pct, prev_time, frame.lap_dist_pct, frame.session_time);
        }
        self.prev_lap_dist_pct = Some(frame.lap_dist_pct);
        self.prev_session_time = Some(frame.session_time);
    }

    fn record_lap_frame(&mut self, frame: &AnalysisFrame) {
        self.lap_accum.frames += 1;
        if frame.on_pit_road {
            self.lap_accum.pit_frames += 1;
        }
        if self.lap_accum.frames == 1 {
            self.lap_accum.min_dist_pct = frame.lap_dist_pct;
            self.lap_accum.max_dist_pct = frame.lap_dist_pct;
            self.lap_accum.start_on_pit = frame.on_pit_road;
            self.lap_accum.end_on_pit = frame.on_pit_road;
        } else {
            self.lap_accum.min_dist_pct = self.lap_accum.min_dist_pct.min(frame.lap_dist_pct);
            self.lap_accum.max_dist_pct = self.lap_accum.max_dist_pct.max(frame.lap_dist_pct);
            self.lap_accum.end_on_pit = frame.on_pit_road;
        }
    }

    pub fn snapshot_from_frame(
        &mut self,
        frame: &AnalysisFrame,
        bounds: &[SectorBoundary],
    ) -> LiveSnapshot {
        self.update(frame, bounds);

        let count = sector_count(bounds);
        let current_sector = if count > 0 {
            current_sector_from_pct(frame.lap_dist_pct, bounds)
        } else {
            1
        };

        let sector_source = self
            .pending_lap_sectors
            .take()
            .unwrap_or_else(|| self.sector_state.completed_sectors().to_vec());

        let sectors: Vec<LiveSectorProgress> = if count > 0 {
            (1..=count as i32)
                .map(|n| {
                    let done = sector_source.iter().find(|(s, _)| *s == n);
                    LiveSectorProgress {
                        sector_num: n,
                        time_ms: done.map(|(_, ms)| *ms),
                        completed: done.is_some(),
                    }
                })
                .collect()
        } else {
            Vec::new()
        };

        LiveSnapshot {
            track: self.track.clone(),
            car: self.car.clone(),
            session_type: self.session_type.clone(),
            lap: self.current_lap,
            last_lap_valid: false,
            fuel_level: frame.fuel_level,
            speed: frame.speed,
            lap_dist_pct: frame.lap_dist_pct,
            throttle: frame.throttle,
            brake: frame.brake,
            throttle_raw: frame.throttle_raw,
            brake_raw: frame.brake_raw,
            clutch: frame.clutch,
            clutch_raw: frame.clutch_raw,
            handbrake_raw: frame.handbrake_raw,
            lat: frame.lat,
            lon: frame.lon,
            current_sector,
            sector_boundaries: display_sector_boundaries(bounds),
            sectors,
            lf_temp: frame.lf_temp,
            rf_temp: frame.rf_temp,
            lr_temp: frame.lr_temp,
            rr_temp: frame.rr_temp,
            on_pit_road: frame.on_pit_road,
            // Lap clock and field data merged from CarIdx in the live loop.
            ..Default::default()
        }
    }
}

impl Default for LiveTracker {
    fn default() -> Self {
        Self::new()
    }
}

fn extract_car_name(session: &SessionInfo) -> String {
    if let Some(driver_info) = &session.driver_info {
        let car_idx = driver_info.driver_car_idx.unwrap_or(-1);
        if let Some(drivers) = &driver_info.drivers {
            for driver in drivers {
                if driver.car_idx == car_idx {
                    return driver
                        .car_screen_name
                        .clone()
                        .or_else(|| driver.car_path.clone())
                        .unwrap_or_else(|| "Unknown Car".into());
                }
            }
        }
    }
    "Unknown Car".into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sector_state::MIN_SECTOR_MS;
    use race_refinery_telemetry::SectorBoundary;

    fn boundary(sector_num: i32, start_pct: f64) -> SectorBoundary {
        SectorBoundary {
            sector_num,
            start_pct,
        }
    }

    fn frame(lap: i32, pct: f32, session_time: f64) -> AnalysisFrame {
        AnalysisFrame {
            session_num: 0,
            lap,
            lap_dist_pct: pct,
            speed: 0.0,
            throttle: 0.0,
            brake: 0.0,
            throttle_raw: None,
            brake_raw: None,
            clutch: None,
            clutch_raw: None,
            handbrake_raw: None,
            steering: 0.0,
            gear: 0,
            fuel_level: 0.0,
            on_pit_road: false,
            session_time,
            lat: None,
            lon: None,
            lf_temp: 0.0,
            rf_temp: 0.0,
            lr_temp: 0.0,
            rr_temp: 0.0,
        }
    }

    fn default_bounds() -> Vec<SectorBoundary> {
        vec![boundary(0, 0.0), boundary(1, 0.34), boundary(2, 0.72)]
    }

    #[test]
    fn sector_crossings_at_splits() {
        let mut tracker = LiveTracker::new();
        let bounds = default_bounds();
        tracker.update(&frame(1, 0.0, 0.0), &bounds);
        tracker.update(&frame(1, 0.35, 34.0), &bounds);
        tracker.update(&frame(1, 0.73, 72.0), &bounds);
        assert_eq!(tracker.completed_sectors().len(), 2);

        let snap = tracker.snapshot_from_frame(&frame(2, 0.01, 100.0), &bounds);
        let completed: Vec<_> = snap
            .sectors
            .iter()
            .filter(|s| s.completed)
            .map(|s| s.sector_num)
            .collect();
        assert_eq!(completed, vec![1, 2, 3]);
        assert_eq!(snap.sectors.len(), 3);
    }

    #[test]
    fn mid_lap_join_skips_passed_splits() {
        let mut tracker = LiveTracker::new();
        let bounds = default_bounds();
        tracker.update(&frame(1, 0.50, 50.0), &bounds);
        assert_eq!(tracker.completed_sectors().len(), 0);
        tracker.update(&frame(1, 0.73, 72.0), &bounds);
        assert_eq!(tracker.completed_sectors().len(), 1);
        assert_eq!(tracker.completed_sectors()[0].0, 2);
    }

    #[test]
    fn no_spurious_sectors_on_single_tick() {
        let mut tracker = LiveTracker::new();
        let bounds = default_bounds();
        tracker.update(&frame(1, 0.0, 0.0), &bounds);
        tracker.update(&frame(1, 0.80, 80.0), &bounds);
        assert!(tracker.completed_sectors().len() <= 2);
        for (_, ms) in tracker.completed_sectors() {
            assert!(*ms >= MIN_SECTOR_MS);
        }
    }

    #[test]
    fn lap_start_desync_high_pct_still_records_sectors() {
        let mut tracker = LiveTracker::new();
        let bounds = default_bounds();
        tracker.update(&frame(1, 0.999, 0.0), &bounds);
        tracker.update(&frame(1, 0.01, 1.0), &bounds);
        tracker.update(&frame(1, 0.35, 35.0), &bounds);
        tracker.update(&frame(1, 0.73, 73.0), &bounds);
        assert_eq!(tracker.completed_sectors().len(), 2);

        let snap = tracker.snapshot_from_frame(&frame(2, 0.01, 100.0), &bounds);
        let completed: Vec<_> = snap
            .sectors
            .iter()
            .filter(|s| s.completed)
            .map(|s| s.sector_num)
            .collect();
        assert_eq!(completed, vec![1, 2, 3]);
    }

    #[test]
    fn four_sector_track_dynamic_sectors() {
        let bounds = vec![
            boundary(0, 0.0),
            boundary(1, 0.26),
            boundary(2, 0.51),
            boundary(3, 0.69),
        ];
        let mut tracker = LiveTracker::new();
        tracker.update(&frame(1, 0.0, 0.0), &bounds);
        tracker.update(&frame(1, 0.27, 20.0), &bounds);
        tracker.update(&frame(1, 0.55, 38.0), &bounds);
        tracker.update(&frame(1, 0.73, 55.0), &bounds);
        assert_eq!(tracker.completed_sectors().len(), 3);

        let snap = tracker.snapshot_from_frame(&frame(1, 0.80, 60.0), &bounds);
        assert_eq!(snap.sectors.len(), 4);
        assert_eq!(snap.current_sector, 4);
    }
}
