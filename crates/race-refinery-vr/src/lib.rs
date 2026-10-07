//! In-headset HUD for iRacing VR.
//!
//! Two modes share one service:
//! * **Native** (default) — writes the live snapshot into shared memory for the
//!   `race-refinery-openxr-layer` DLL, which composites quads directly in the headset.
//! * **Web** (fallback) — serves the HUD over HTTP for browser / OpenKneeboard preview.
//!
//! Diagnostics are producer-side only: the layer does not write a heartbeat file
//! (no disk I/O in `xrEndFrame`). `compositor_active` is therefore a proxy —
//! fresh SHM publishes while the layer is installed — not proof the compositor
//! drew a frame.

mod hud_server;
mod layer_install;
mod shm;

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use parking_lot::Mutex;
use tokio_util::sync::CancellationToken;

use race_refinery_analysis::TrackOutline;
use race_refinery_live::{LiveService, LiveSnapshot};
use race_refinery_settings::AppSettings;

pub use hud_server::{check_hud_health, hud_url, open_hud_preview, HUD_PORT};
pub use layer_install::{
    install_layer, is_layer_installed, layer_diagnostics, uninstall_layer, VrLayerDiagnostics,
    MANIFEST_FILE,
};

pub struct VrOverlayService {
    cancel: Mutex<Option<CancellationToken>>,
    status: Mutex<VrOverlayStatus>,
    /// Wall-clock ms of the most recent native frame published to shared memory.
    last_frame_ms: Mutex<Option<u64>>,
    /// Overlay slots enabled in the last published block.
    last_overlay_count: Mutex<u32>,
    /// Last native-loop error (cleared on successful start).
    last_error: Mutex<Option<String>>,
    /// Incremented per recenter request and published to the layer.
    recenter_seq: AtomicU32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VrOverlayStatus {
    pub active: bool,
    pub runtime: String,
    pub message: String,
    pub hud_url: String,
    /// "native" or "web".
    pub mode: String,
    /// Whether the OpenXR API layer is registered with the loader.
    pub layer_installed: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NativeVrStatus {
    pub active: bool,
    pub layer_installed: bool,
    /// Proxy: telemetry is publishing freshly and the layer is installed.
    /// The layer no longer writes a disk heartbeat; this is not a compositor ACK.
    pub compositor_active: bool,
    /// True when Race Refinery is publishing telemetry to shared memory.
    pub telemetry_publishing: bool,
    /// Age of the last published telemetry frame in ms (None if nothing published yet).
    pub last_frame_age_ms: Option<u64>,
    /// Same as `last_frame_age_ms` — producer write age into SHM.
    pub write_age_ms: Option<u64>,
    /// Enabled overlay slots in the last published SHM block.
    pub overlay_count: u32,
    /// Last native-loop error, if any.
    pub last_error: Option<String>,
}

impl VrOverlayService {
    pub fn new() -> Self {
        Self {
            cancel: Mutex::new(None),
            status: Mutex::new(VrOverlayStatus {
                hud_url: hud_url(),
                mode: "native".into(),
                ..Default::default()
            }),
            last_frame_ms: Mutex::new(None),
            last_overlay_count: Mutex::new(0),
            last_error: Mutex::new(None),
            recenter_seq: AtomicU32::new(0),
        }
    }

    /// Re-anchor world-locked overlays to the driver's current head pose on the
    /// layer's next frame. Harmless when the HUD is not running.
    pub fn request_recenter(&self) {
        self.recenter_seq.fetch_add(1, Ordering::Relaxed);
        tracing::info!("VR recenter requested");
    }

    pub fn is_active(&self) -> bool {
        self.cancel.lock().is_some()
    }

    pub fn status(&self) -> VrOverlayStatus {
        let mut s = self.status.lock().clone();
        s.hud_url = hud_url();
        s.layer_installed = layer_install::is_layer_installed();
        s
    }

    pub fn native_status(&self) -> NativeVrStatus {
        let now = now_ms();
        let last = *self.last_frame_ms.lock();
        let write_age_ms = last.map(|t| now.saturating_sub(t));
        let telemetry_publishing = write_age_ms.map(|age| age < 2000).unwrap_or(false);
        let layer_installed = layer_install::is_layer_installed();
        // Without a layer-side ACK, treat fresh publishes + installed layer as the
        // best available "compositor likely active" signal for the Live panel.
        let compositor_active = telemetry_publishing && layer_installed;
        NativeVrStatus {
            active: self.is_active(),
            layer_installed,
            compositor_active,
            telemetry_publishing,
            last_frame_age_ms: write_age_ms,
            write_age_ms,
            overlay_count: *self.last_overlay_count.lock(),
            last_error: self.last_error.lock().clone(),
        }
    }

    pub fn stop(&self) {
        if let Some(token) = self.cancel.lock().take() {
            token.cancel();
        }
        *self.last_frame_ms.lock() = None;
        *self.last_overlay_count.lock() = 0;
        let mode = self.status.lock().mode.clone();
        *self.status.lock() = VrOverlayStatus {
            active: false,
            runtime: String::new(),
            message: "In-headset HUD stopped".into(),
            hud_url: hud_url(),
            mode,
            layer_installed: layer_install::is_layer_installed(),
        };
    }

    /// Start the in-headset HUD in the mode selected by `settings.vr_mode`.
    pub fn start(self: &Arc<Self>, live: Arc<LiveService>, settings: AppSettings) {
        if self.is_active() {
            return;
        }

        let native = settings.vr_mode != "web";
        let token = CancellationToken::new();
        *self.cancel.lock() = Some(token.clone());
        *self.last_error.lock() = None;

        if native {
            self.start_native(live, settings, token);
        } else {
            self.start_web(live, token);
        }
    }

    fn start_web(self: &Arc<Self>, live: Arc<LiveService>, token: CancellationToken) {
        *self.status.lock() = VrOverlayStatus {
            active: true,
            runtime: "OpenXR (Web HUD)".into(),
            message: "Starting in-headset HUD server…".into(),
            hud_url: hud_url(),
            mode: "web".into(),
            layer_installed: layer_install::is_layer_installed(),
        };
        let service = Arc::clone(self);
        thread::spawn(move || {
            if let Err(e) = hud_server::run_hud_server(service.clone(), live, token, true) {
                let msg = format!("HUD server error: {e:#}");
                *service.last_error.lock() = Some(msg.clone());
                service.status.lock().message = msg;
                service.status.lock().active = false;
            }
            *service.cancel.lock() = None;
        });
    }

    fn start_native(
        self: &Arc<Self>,
        live: Arc<LiveService>,
        settings: AppSettings,
        token: CancellationToken,
    ) {
        let installed = layer_install::is_layer_installed();
        *self.status.lock() = VrOverlayStatus {
            active: true,
            runtime: "OpenXR (native layer)".into(),
            message: if installed {
                "Publishing telemetry for in-headset layer".into()
            } else {
                "VR layer not installed — install it, then restart iRacing".into()
            },
            hud_url: hud_url(),
            mode: "native".into(),
            layer_installed: installed,
        };

        // Browser preview shares this cancel token; stop HUD tears both down.
        let preview_service = Arc::clone(self);
        let preview_live = Arc::clone(&live);
        let preview_token = token.clone();
        thread::spawn(move || {
            if let Err(e) =
                hud_server::run_hud_server(preview_service, preview_live, preview_token, false)
            {
                tracing::warn!("VR browser preview server: {e:#}");
            }
        });

        let service = Arc::clone(self);
        thread::spawn(move || {
            if let Err(e) = run_native_loop(service.clone(), live, settings, token) {
                let msg = format!("Native VR error: {e:#}");
                *service.last_error.lock() = Some(msg.clone());
                service.status.lock().message = msg;
                service.status.lock().active = false;
            }
            *service.cancel.lock() = None;
        });
    }
}

impl Default for VrOverlayService {
    fn default() -> Self {
        Self::new()
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn field_pace_ordinal(mode: &str) -> u32 {
    match mode {
        "optimal" => shm::FIELD_PACE_OPTIMAL,
        "both" => shm::FIELD_PACE_BOTH,
        _ => shm::FIELD_PACE_BEST,
    }
}

/// Publish the live snapshot to shared memory at ~30 Hz until cancelled.
/// When live data is empty, publishes a test pattern with the coach quad enabled
/// at its configured placement.
fn run_native_loop(
    service: Arc<VrOverlayService>,
    live: Arc<LiveService>,
    initial: AppSettings,
    cancel: CancellationToken,
) -> anyhow::Result<()> {
    let mut writer = shm::ShmWriter::open()?;
    let _ = initial;
    // Outlines are static per circuit, so the cache is only re-read on a track change.
    let mut track_map: Option<TrackOutline> = None;
    let mut track_map_for = String::new();

    while !cancel.is_cancelled() {
        let settings = race_refinery_settings::load_settings();
        let mut snap = live.snapshot.lock().clone();
        let layout = &settings.overlay_layout;
        let mut slots = [shm::SlotPlacement::DISABLED; shm::MAX_OVERLAYS];
        for (i, w) in layout.widgets.iter().enumerate() {
            slots[i] = shm::SlotPlacement::from_widget(w);
        }

        // Test pattern: ensure the coach is visible when idle, at the user's placement.
        if snap.track.is_empty() && snap.lap <= 0 {
            snap = test_pattern_snapshot();
            slots[shm::KIND_COACH as usize].enabled = true;
        }

        if track_map_for != snap.track {
            track_map = race_refinery_storage::load_track_map(&snap.track);
            track_map_for = snap.track.clone();
        }

        let field_pace = field_pace_ordinal(&layout.field_pace_mode);
        let block = shm::build_block(
            &snap,
            &slots,
            field_pace,
            track_map.as_ref(),
            service.recenter_seq.load(Ordering::Relaxed),
        );
        let overlay_count = slots.iter().filter(|s| s.enabled).count() as u32;
        writer.publish(block);
        *service.last_frame_ms.lock() = Some(now_ms());
        *service.last_overlay_count.lock() = overlay_count;

        thread::sleep(Duration::from_millis(33));
    }

    *service.last_frame_ms.lock() = None;
    *service.last_overlay_count.lock() = 0;
    Ok(())
}

fn test_pattern_snapshot() -> LiveSnapshot {
    LiveSnapshot {
        track: "VR Test".into(),
        car: "Test".into(),
        session_type: "Practice".into(),
        lap: 1,
        lap_time_ms: 45_000.0,
        last_lap_ms: Some(89_123.0),
        best_lap_ms: Some(88_500.0),
        delta_to_best_ms: Some(623.0),
        fuel_level: 32.0,
        speed: 55.0,
        lap_dist_pct: 0.45,
        current_sector: 2,
        on_track: true,
        pits_open: true,
        player_position: Some(5),
        player_class_position: Some(3),
        ..Default::default()
    }
}
