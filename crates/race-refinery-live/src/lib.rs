//! Live iRacing telemetry: connection loop, snapshot, sectors, competitors.
mod car_idx_frame;
mod coach_meta;
mod competitors;
mod lap_signals;
mod pack;
mod player_frame;
mod sector_state;
mod snapshot;
mod tracker;

pub use coach_meta::{build_coach_session_meta, CoachSessionMeta};
pub use competitors::CompetitorEntry;
pub use pack::PackState;
pub use snapshot::{LiveConnectionState, LiveSnapshot, LiveStatus};

use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use parking_lot::Mutex;
use pitwall::{LiveConnection, Pitwall, SessionInfo, UpdateRate};
use tauri::{AppHandle, Emitter};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use race_refinery_telemetry::SectorBoundary;

use self::car_idx_frame::CarIdxFrame;
use self::competitors::lap_seconds_to_ms;
use self::lap_signals::include_in_stats_live;
use self::player_frame::AnalysisFrame;
use self::sector_state::{extract_sector_boundaries, normalize_sector_boundaries, region_starts};
use self::tracker::LiveTracker;

/// How far back we look for IBT files is owned by ingest post-session.
/// Wait this long after a frame-stream drop before treating it as a real session
/// end. Brief SDK reconnects within this window keep traffic history and skip
/// premature IBT import.
const SESSION_END_DEBOUNCE: Duration = Duration::from_secs(12);

/// Desktop-composed callback: scan/import recent IBTs after a live session ends.
pub type PostSessionImportFn = Arc<dyn Fn(AppHandle) + Send + Sync>;

pub struct LiveService {
    pub status: Mutex<LiveStatus>,
    pub snapshot: Mutex<LiveSnapshot>,
    pub session_meta: Mutex<Option<CoachSessionMeta>>,
    /// Lap numbers completed while side-by-side with another car.
    pub traffic_laps: Mutex<Vec<i32>>,
    cancel: Mutex<Option<CancellationToken>>,
    demo_cancel: Mutex<Option<CancellationToken>>,
    post_session_import: Mutex<Option<PostSessionImportFn>>,
}

impl LiveService {
    pub fn new() -> Self {
        Self {
            status: Mutex::new(LiveStatus::default()),
            snapshot: Mutex::new(LiveSnapshot::default()),
            session_meta: Mutex::new(None),
            traffic_laps: Mutex::new(Vec::new()),
            cancel: Mutex::new(None),
            demo_cancel: Mutex::new(None),
            post_session_import: Mutex::new(None),
        }
    }
}

impl Default for LiveService {
    fn default() -> Self {
        Self::new()
    }
}

impl LiveService {
    /// Wire the desktop-composed post-session IBT import hook (call once at startup).
    pub fn set_post_session_import(&self, hook: PostSessionImportFn) {
        *self.post_session_import.lock() = Some(hook);
    }

    pub fn is_running(&self) -> bool {
        self.cancel.lock().is_some() || self.demo_cancel.lock().is_some()
    }

    pub fn stop(&self) {
        if let Some(token) = self.cancel.lock().take() {
            token.cancel();
        }
        if let Some(token) = self.demo_cancel.lock().take() {
            token.cancel();
        }
        *self.status.lock() = LiveStatus {
            state: LiveConnectionState::Disconnected,
            message: "Live monitor stopped".into(),
        };
    }

    pub fn start(self: &Arc<Self>, app: AppHandle) {
        if self.cancel.lock().is_some() {
            return;
        }
        // Prefer real iRacing over demo when both requested.
        if let Some(token) = self.demo_cancel.lock().take() {
            token.cancel();
        }
        let token = CancellationToken::new();
        *self.cancel.lock() = Some(token.clone());
        *self.status.lock() = LiveStatus {
            state: LiveConnectionState::WaitingForSession,
            message: "Connecting to iRacing...".into(),
        };

        let service = Arc::clone(self);
        tauri::async_runtime::spawn(async move {
            Arc::clone(&service).run_loop(app, token).await;
            *service.cancel.lock() = None;
        });
    }

    /// Publish a synthetic [`LiveSnapshot`] at 10 Hz without iRacing (HUD / coach smoke test).
    pub fn start_demo(self: &Arc<Self>, app: AppHandle) {
        if self.is_running() {
            return;
        }
        let token = CancellationToken::new();
        *self.demo_cancel.lock() = Some(token.clone());
        *self.status.lock() = LiveStatus {
            state: LiveConnectionState::Connected,
            message: "Demo clock (no iRacing)".into(),
        };

        let service = Arc::clone(self);
        tauri::async_runtime::spawn(async move {
            let mut tick: u64 = 0;
            let mut interval = tokio::time::interval(Duration::from_millis(100));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            while !token.is_cancelled() {
                interval.tick().await;
                tick = tick.wrapping_add(1);
                let t = tick as f64 * 0.1;
                let lap = ((t / 90.0) as i32) + 1;
                let lap_frac = (t % 90.0) / 90.0;
                let snap = LiveSnapshot {
                    track: "Demo Track".into(),
                    car: "Demo Car".into(),
                    session_type: "Practice".into(),
                    lap,
                    lap_time_ms: (t % 90.0) * 1000.0,
                    last_lap_ms: if lap > 1 { Some(89_500.0) } else { None },
                    last_lap_valid: lap > 1,
                    best_lap_ms: if lap > 1 { Some(88_200.0) } else { None },
                    delta_to_best_ms: Some(((t % 90.0) - 88.2) * 1000.0),
                    fuel_level: (40.0 - t * 0.02).max(5.0) as f32,
                    speed: (40.0 + (t * 0.7).sin() * 15.0) as f32,
                    lap_dist_pct: lap_frac as f32,
                    current_sector: ((lap_frac * 3.0) as i32).clamp(1, 3),
                    sector_boundaries: vec![0.0, 0.33, 0.66, 1.0],
                    on_track: true,
                    pits_open: true,
                    player_position: Some(3),
                    player_class_position: Some(2),
                    pack_state: PackState::Clear,
                    ..Default::default()
                };
                *service.snapshot.lock() = snap.clone();
                let status = service.status.lock().clone();
                let _ = app.emit("live-telemetry", &snap);
                let _ = app.emit("live-status", &status);
            }
            *service.demo_cancel.lock() = None;
        });
    }

    pub fn stop_demo(&self) {
        if let Some(token) = self.demo_cancel.lock().take() {
            token.cancel();
        }
        if self.cancel.lock().is_none() {
            *self.status.lock() = LiveStatus {
                state: LiveConnectionState::Disconnected,
                message: "Demo clock stopped".into(),
            };
        }
    }

    fn set_status(&self, state: LiveConnectionState, message: impl Into<String>) {
        *self.status.lock() = LiveStatus {
            state,
            message: message.into(),
        };
    }

    async fn sleep_or_cancel(cancel: &CancellationToken, dur: Duration) -> bool {
        tokio::select! {
            _ = cancel.cancelled() => true,
            _ = tokio::time::sleep(dur) => false,
        }
    }

    async fn run_loop(self: Arc<Self>, app: AppHandle, cancel: CancellationToken) {
        info!("Starting live telemetry monitor");
        let min_backoff = Duration::from_secs(1);
        let max_backoff = Duration::from_secs(5);
        let mut backoff = min_backoff;
        let mut attempted = false;
        let mut pending_session_end: Option<std::time::Instant> = None;
        let mut last_track = String::new();

        while !cancel.is_cancelled() {
            if let Some(ended_at) = pending_session_end {
                if ended_at.elapsed() >= SESSION_END_DEBOUNCE {
                    info!("Session end debounce elapsed — scanning IBT");
                    self.invoke_post_session_import(app.clone());
                    pending_session_end = None;
                    self.traffic_laps.lock().clear();
                }
            }

            if attempted {
                self.set_status(
                    LiveConnectionState::Reconnecting,
                    "Reconnecting to iRacing...",
                );
            }

            let connection = match Pitwall::connect().await {
                Ok(c) => c,
                Err(e) => {
                    warn!("Live connect failed: {e:#}");
                    attempted = true;
                    if Self::sleep_or_cancel(&cancel, backoff).await {
                        break;
                    }
                    backoff = (backoff * 2).min(max_backoff);
                    continue;
                }
            };

            attempted = true;
            let preserve_traffic = pending_session_end
                .map(|t| t.elapsed() < SESSION_END_DEBOUNCE)
                .unwrap_or(false);
            let result = self
                .run_session(&app, &connection, &cancel, preserve_traffic)
                .await;

            if cancel.is_cancelled() {
                if result.had_frames || pending_session_end.is_some() {
                    self.invoke_post_session_import(app.clone());
                }
                break;
            }

            if result.had_frames {
                backoff = min_backoff;
                if preserve_traffic {
                    info!(
                        "Reconnected within debounce — prior stream end treated as transport blip"
                    );
                }

                let track_changed = !last_track.is_empty()
                    && !result.track.is_empty()
                    && last_track != result.track;
                if !result.track.is_empty() {
                    last_track = result.track.clone();
                }

                if track_changed {
                    info!("Track changed across reconnect — finalizing previous session");
                    self.invoke_post_session_import(app.clone());
                    pending_session_end = None;
                    self.traffic_laps.lock().clear();
                } else if result.stream_ended {
                    pending_session_end = Some(std::time::Instant::now());
                    info!(
                        "Frame stream ended; deferring session finalize for {:?}",
                        SESSION_END_DEBOUNCE
                    );
                } else {
                    pending_session_end = None;
                }
            }

            self.set_status(
                LiveConnectionState::Reconnecting,
                "iRacing session ended — waiting to reconnect...",
            );
            if Self::sleep_or_cancel(&cancel, backoff).await {
                if pending_session_end.is_some() {
                    self.invoke_post_session_import(app.clone());
                }
                break;
            }
            backoff = (backoff * 2).min(max_backoff);
        }

        info!("Live monitor stopped");
    }

    fn invoke_post_session_import(&self, app: AppHandle) {
        if let Some(hook) = self.post_session_import.lock().clone() {
            hook(app);
        }
    }

    async fn run_session(
        &self,
        app: &AppHandle,
        connection: &LiveConnection,
        cancel: &CancellationToken,
        preserve_traffic: bool,
    ) -> SessionRunResult {
        let mut frame_stream = connection.subscribe::<AnalysisFrame>(UpdateRate::Max(10));
        let mut car_idx_stream = connection.subscribe::<CarIdxFrame>(UpdateRate::Max(4));
        let mut session_stream = Box::pin(connection.session_updates());

        let mut tracker = LiveTracker::new();
        let mut sector_bounds: Vec<SectorBoundary> = Vec::new();
        let mut prev_sector_bounds: Vec<SectorBoundary> = Vec::new();
        let mut got_frame = false;
        let mut ever_got_frame = false;
        let mut latest_car_idx: Option<CarIdxFrame> = None;
        let mut tracked_lap = 0;
        let mut current_lap_in_traffic = false;

        if !preserve_traffic {
            self.traffic_laps.lock().clear();
        }

        if let Some(session) = connection.current_session() {
            sector_bounds = extract_sector_boundaries(&session);
            log_sector_bounds_if_changed(
                &session,
                &sector_bounds,
                &mut prev_sector_bounds,
                "loaded",
            );
            tracker.set_session_meta(&session);
            *self.session_meta.lock() = Some(build_coach_session_meta(&session));
        }

        let mut emit_tick = tokio::time::interval(Duration::from_millis(100));
        emit_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    info!("Live monitor cancelled");
                    return SessionRunResult {
                        had_frames: ever_got_frame,
                        stream_ended: false,
                        track: tracker.track().to_string(),
                    };
                }
                session = session_stream.next() => {
                    if let Some(session) = session {
                        sector_bounds = extract_sector_boundaries(&session);
                        log_sector_bounds_if_changed(
                            &session,
                            &sector_bounds,
                            &mut prev_sector_bounds,
                            "updated",
                        );
                        let prev_track = tracker.track().to_string();
                        tracker.set_session_meta(&session);
                        *self.session_meta.lock() = Some(build_coach_session_meta(&session));
                        if !prev_track.is_empty() && prev_track != tracker.track() {
                            info!("Track changed ({prev_track} -> {}), resetting tracker", tracker.track());
                            self.invoke_post_session_import(app.clone());
                            tracker.reset_session();
                            tracker.set_session_meta(&session);
                            self.traffic_laps.lock().clear();
                            tracked_lap = 0;
                            current_lap_in_traffic = false;
                        }
                    }
                }
                car_idx = car_idx_stream.next() => {
                    match car_idx {
                        Some(c) => latest_car_idx = Some(c),
                        None => latest_car_idx = None,
                    }
                }
                frame = frame_stream.next() => {
                    match frame {
                        Some(f) => {
                            got_frame = true;
                            ever_got_frame = true;
                            if tracker.track().is_empty() {
                                if let Some(session) = connection.current_session() {
                                    sector_bounds = extract_sector_boundaries(&session);
                                    tracker.set_session_meta(&session);
                                }
                            }
                            self.set_status(LiveConnectionState::Connected, "Receiving telemetry");
                            let bounds = &sector_bounds;
                            let mut snap = tracker.snapshot_from_frame(&f, bounds);
                            if let Some(car_idx) = &latest_car_idx {
                                merge_car_idx(&mut snap, &tracker, car_idx);
                            }

                            if snap.pack_state.is_traffic() {
                                current_lap_in_traffic = true;
                            }
                            if snap.lap != tracked_lap {
                                if tracked_lap > 0 && current_lap_in_traffic {
                                    self.traffic_laps.lock().push(tracked_lap);
                                }
                                tracked_lap = snap.lap;
                                current_lap_in_traffic = false;
                            }

                            *self.snapshot.lock() = snap;
                        }
                        None => {
                            warn!("Live frame stream ended");
                            return SessionRunResult {
                                had_frames: ever_got_frame,
                                stream_ended: true,
                                track: tracker.track().to_string(),
                            };
                        }
                    }
                }
                _ = emit_tick.tick() => {
                    if got_frame {
                        let snap = self.snapshot.lock().clone();
                        let status = self.status.lock().clone();
                        let _ = app.emit("live-telemetry", &snap);
                        let _ = app.emit("live-status", &status);
                    }
                }
            }
        }
    }
}

struct SessionRunResult {
    had_frames: bool,
    stream_ended: bool,
    track: String,
}

fn log_sector_bounds_if_changed(
    session: &SessionInfo,
    bounds: &[SectorBoundary],
    prev: &mut Vec<SectorBoundary>,
    event: &str,
) {
    if bounds == *prev {
        return;
    }
    if let Some(raw) = session
        .split_time_info
        .as_ref()
        .and_then(|s| s.sectors.as_ref())
    {
        info!(
            raw_sectors = ?raw.iter().map(|s| (s.sector_num, s.sector_start_pct)).collect::<Vec<_>>(),
            region_starts = ?region_starts(bounds),
            splits = ?normalize_sector_boundaries(bounds),
            "Sector boundaries {event} from session YAML"
        );
    } else {
        info!("Sector boundaries suppressed: no SplitTimeInfo.Sectors in session YAML");
    }
    *prev = bounds.to_vec();
}

fn merge_car_idx(snap: &mut LiveSnapshot, tracker: &LiveTracker, frame: &CarIdxFrame) {
    let player_car_idx = if tracker.player_car_idx() >= 0 {
        tracker.player_car_idx()
    } else {
        frame.player_car_idx
    };
    let comp = competitors::build(tracker.roster(), player_car_idx, frame);
    snap.competitors = comp.competitors;
    snap.player_position = comp.player_position;
    snap.player_class_position = comp.player_class_position;
    snap.session_fastest_lap_ms = comp.session_fastest_lap_ms;
    snap.gap_to_car_ahead_s = comp.gap_to_car_ahead_s;
    snap.gap_to_car_behind_s = comp.gap_to_car_behind_s;
    snap.pack_state = PackState::from_car_left_right(frame.car_left_right_value());

    if frame.delta_session_best_ok {
        snap.delta_to_session_best_ms = Some(frame.delta_session_best as f64 * 1000.0);
    }
    if frame.delta_session_optimal_ok {
        snap.delta_to_session_optimal_ms = Some(frame.delta_session_optimal as f64 * 1000.0);
    }

    snap.session_flags = frame.session_flags_value();
    snap.incident_count = frame.incident_count;
    snap.session_laps_remain =
        (frame.session_laps_remain >= 0).then_some(frame.session_laps_remain);
    snap.session_time_remain_s =
        (frame.session_time_remain >= 0.0).then_some(frame.session_time_remain);
    snap.pits_open = frame.pits_open;
    snap.on_track = frame.on_track;

    if let Some(ms) = lap_seconds_to_ms(Some(frame.current_lap_time)) {
        snap.lap_time_ms = ms;
    }
    snap.last_lap_ms = lap_seconds_to_ms(Some(frame.player_last_lap_time));
    snap.best_lap_ms = lap_seconds_to_ms(Some(frame.player_best_lap_time));
    if frame.delta_best_ok {
        snap.delta_to_best_ms = Some(frame.delta_best as f64 * 1000.0);
    }
    if frame.delta_last_ok {
        snap.delta_to_last_ms = Some(frame.delta_last as f64 * 1000.0);
    }

    if let Some(flying) = tracker.last_finished_flying() {
        snap.last_lap_valid = include_in_stats_live(
            flying,
            tracker.last_finished_completed(),
            frame.completed_lap_ok(),
        );
    }
}
