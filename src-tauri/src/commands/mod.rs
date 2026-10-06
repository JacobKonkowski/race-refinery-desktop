//! Tauri IPC commands and shared [`AppState`].
//!
//! Every `#[tauri::command]` here is registered in [`crate::run`] and wrapped by
//! the frontend API layer. Analyze commands stay available; live / audio / VR
//! are restored for the usable rebuild milestone. Voice pack and Voice Studio
//! commands live in [`voice`].
pub mod voice;

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use parking_lot::Mutex;
use race_refinery_input::InputWatcher;
use tauri::{AppHandle, Emitter, State};

use crate::analysis::{
    compare_laps as run_compare, corner_consistency as run_consistency, CompareInput,
    ConsistencyLap, CornerConsistency, LapComparison, TracePoint, TrackOutline,
};
use crate::audio::AudioCoachService;
use crate::ingest::{
    check_iracing_config, default_telemetry_dir, run_import, run_reimport, spawn_recent_ibt_import,
    validate_import_path, ImportHandles,
};
use crate::live::{LiveService, LiveSnapshot, LiveStatus, PostSessionImportFn};
use crate::monitor::MonitorOverlayService;
use crate::settings::{load_settings, save_settings, AppSettings, ControllerBinding};
use crate::storage::{
    load_track_map, Database, ImportStatus, IracingConfigCheck, LapTrace, SessionDetail,
    SessionSummary,
};
use crate::vr::{NativeVrStatus, VrLayerDiagnostics, VrOverlayService, VrOverlayStatus};

pub struct AppState {
    pub import: ImportHandles,
    pub live: Arc<LiveService>,
    pub audio: Arc<AudioCoachService>,
    pub vr: Arc<VrOverlayService>,
    pub monitor: Arc<MonitorOverlayService>,
    pub settings: Mutex<AppSettings>,
    /// Controller watcher for the recenter button; set once the main window exists.
    pub input: OnceLock<InputWatcher>,
    /// Accelerator currently registered for recenter, if any.
    pub recenter_hotkey: Mutex<Option<String>>,
}

impl AppState {
    pub fn new() -> anyhow::Result<Self> {
        let import = ImportHandles {
            db: Arc::new(Mutex::new(Database::open()?)),
            import_status: Arc::new(Mutex::new(ImportStatus::default())),
            import_gate: Arc::new(tokio::sync::Mutex::new(())),
        };
        let live = Arc::new(LiveService::new());
        let import_for_hook = import.clone();
        let hook: PostSessionImportFn = Arc::new(move |app: AppHandle| {
            spawn_recent_ibt_import(app, import_for_hook.clone());
        });
        live.set_post_session_import(hook);
        Ok(Self {
            import,
            live,
            audio: Arc::new(AudioCoachService::new()),
            vr: Arc::new(VrOverlayService::new()),
            monitor: Arc::new(MonitorOverlayService::new()),
            settings: Mutex::new(load_settings()),
            input: OnceLock::new(),
            recenter_hotkey: Mutex::new(None),
        })
    }
}

#[tauri::command]
pub fn list_sessions(state: State<'_, Arc<AppState>>) -> Result<Vec<SessionSummary>, String> {
    state
        .import
        .db
        .lock()
        .list_sessions()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_session(
    state: State<'_, Arc<AppState>>,
    session_id: i64,
) -> Result<Option<SessionDetail>, String> {
    state
        .import
        .db
        .lock()
        .get_session(session_id)
        .map_err(|e| e.to_string())
}

/// Cached circuit outline for a track, generated from a prior IBT import.
/// None when no import for that track carried GPS channels.
#[tauri::command]
pub fn get_track_map(track: String) -> Option<TrackOutline> {
    load_track_map(&track)
}

#[tauri::command]
pub fn get_lap_traces(
    state: State<'_, Arc<AppState>>,
    lap_ids: Vec<i64>,
) -> Result<Vec<LapTrace>, String> {
    state
        .import
        .db
        .lock()
        .get_lap_traces(&lap_ids)
        .map_err(|e| e.to_string())
}

/// Compare a candidate lap against a reference lap (time, sectors, aligned traces).
#[tauri::command]
pub fn compare_laps(
    state: State<'_, Arc<AppState>>,
    candidate_lap_id: i64,
    reference_lap_id: i64,
) -> Result<LapComparison, String> {
    let db = state.import.db.lock();
    let (cand_time, cand_sectors, cand_traces) = db
        .get_lap_compare_data(candidate_lap_id)
        .map_err(|e| e.to_string())?;
    let (ref_time, ref_sectors, ref_traces) = db
        .get_lap_compare_data(reference_lap_id)
        .map_err(|e| e.to_string())?;

    let candidate = CompareInput {
        lap_id: candidate_lap_id,
        lap_time_ms: cand_time,
        sectors: &cand_sectors,
        traces: &cand_traces,
    };
    let reference = CompareInput {
        lap_id: reference_lap_id,
        lap_time_ms: ref_time,
        sectors: &ref_sectors,
        traces: &ref_traces,
    };
    Ok(run_compare(&candidate, &reference))
}

/// Brake point and corner time for each of `lap_ids` through the reference
/// lap's corners. The caller picks the laps (clean laps of one sub-session).
#[tauri::command]
pub fn corner_consistency(
    state: State<'_, Arc<AppState>>,
    reference_lap_id: i64,
    lap_ids: Vec<i64>,
) -> Result<Vec<CornerConsistency>, String> {
    let db = state.import.db.lock();
    let load = |lap_id: i64| {
        db.get_lap_compare_data(lap_id)
            .map(|(time, _, traces)| (lap_id, time, traces))
            .map_err(|e| e.to_string())
    };
    let reference = load(reference_lap_id)?;
    let others = lap_ids
        .into_iter()
        .filter(|&id| id != reference_lap_id)
        .map(load)
        .collect::<Result<Vec<_>, _>>()?;
    drop(db);

    fn input(lap: &(i64, Option<f64>, Vec<TracePoint>)) -> ConsistencyLap<'_> {
        ConsistencyLap {
            lap_id: lap.0,
            lap_time_ms: lap.1,
            traces: &lap.2,
        }
    }
    let laps: Vec<ConsistencyLap> = std::iter::once(&reference)
        .chain(&others)
        .map(input)
        .collect();
    Ok(run_consistency(&input(&reference), &laps))
}

#[tauri::command]
pub async fn import_ibt(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    path: String,
) -> Result<String, String> {
    let path_buf = validate_import_path(&path)?;
    run_import(&app, &state.import, path_buf)
        .await
        .map_err(|e| {
            let msg = format!("Import failed: {e:#}");
            {
                let mut status = state.import.import_status.lock();
                status.active = false;
                status.message = msg.clone();
            }
            let _ = app.emit("import-status", state.import.import_status.lock().clone());
            msg
        })?;
    Ok(state.import.import_status.lock().message.clone())
}

/// Re-parse a session's source IBT with the current analysis pipeline, replacing
/// the old rows. Returns the new session id.
#[tauri::command]
pub async fn reimport_session_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    session_id: i64,
) -> Result<i64, String> {
    let ibt_path = state
        .import
        .db
        .lock()
        .get_session(session_id)
        .map_err(|e| e.to_string())?
        .ok_or("Session not found")?
        .session
        .ibt_path;
    let path = validate_import_path(&ibt_path)?;
    if !path.is_file() {
        return Err(format!("Source IBT no longer exists: {ibt_path}"));
    }
    run_reimport(&app, &state.import, session_id, path)
        .await
        .map(|r| r.session_id)
        .map_err(|e| format!("Re-import failed: {e:#}"))
}

#[tauri::command]
pub async fn import_folder_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<usize, String> {
    let dir = default_telemetry_dir();
    crate::ingest::import_folder(&app, &state.import, dir)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn check_iracing_config_cmd() -> IracingConfigCheck {
    check_iracing_config()
}

#[tauri::command]
pub fn get_import_status(state: State<'_, Arc<AppState>>) -> ImportStatus {
    state.import.import_status.lock().clone()
}

#[tauri::command]
pub fn pick_ibt_file(app: AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let file = app
        .dialog()
        .file()
        .add_filter("iRacing Telemetry", &["ibt"])
        .blocking_pick_file();
    Ok(file.map(|f| f.to_string()))
}

#[tauri::command]
pub fn clear_database_cmd(state: State<'_, Arc<AppState>>) -> Result<usize, String> {
    #[cfg(not(debug_assertions))]
    {
        let _ = state;
        Err("Clear database is only available in development builds".into())
    }
    #[cfg(debug_assertions)]
    {
        let removed = state
            .import
            .db
            .lock()
            .clear_all()
            .map_err(|e| e.to_string())?;
        let mut status = state.import.import_status.lock();
        *status = ImportStatus {
            active: false,
            current_file: None,
            progress_pct: 0.0,
            message: if removed > 0 {
                format!("Cleared {removed} session(s) from database")
            } else {
                "Database already empty".into()
            },
        };
        Ok(removed)
    }
}

#[tauri::command]
pub fn delete_session_cmd(
    state: State<'_, Arc<AppState>>,
    session_id: i64,
) -> Result<bool, String> {
    state
        .import
        .db
        .lock()
        .delete_session(session_id)
        .map_err(|e| e.to_string())
}

// --- Live -------------------------------------------------------------------

#[tauri::command]
pub fn start_live_monitor(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    state.live.start(app.clone());
    let settings = state.settings.lock().clone();
    if settings.vr_overlay_enabled {
        state.vr.start(state.live.clone(), settings.clone());
    }
    if settings.audio_coach_enabled {
        state.audio.start(state.live.clone());
    }
    Ok(())
}

#[tauri::command]
pub fn stop_live_monitor(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    state.live.stop();
    state.vr.stop();
    state.audio.stop();
    state.monitor.stop(&app);
    Ok(())
}

#[tauri::command]
pub fn get_live_status(state: State<'_, Arc<AppState>>) -> LiveStatus {
    state.live.status.lock().clone()
}

#[tauri::command]
pub fn get_live_snapshot(state: State<'_, Arc<AppState>>) -> LiveSnapshot {
    state.live.snapshot.lock().clone()
}

#[tauri::command]
pub fn start_demo_clock(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    state.live.start_demo(app);
    Ok(())
}

#[tauri::command]
pub fn stop_demo_clock(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    state.live.stop_demo();
    Ok(())
}

// --- Settings ---------------------------------------------------------------

#[tauri::command]
pub fn get_settings(state: State<'_, Arc<AppState>>) -> AppSettings {
    state.settings.lock().clone()
}

/// Persist `settings`, apply side effects (recenter bindings), update state and
/// notify listeners. A hotkey that cannot be registered rejects the whole save.
pub(crate) fn persist_settings(
    app: &AppHandle,
    state: &AppState,
    settings: AppSettings,
) -> Result<AppSettings, String> {
    let hotkey_changed = state.settings.lock().vr_recenter_hotkey != settings.vr_recenter_hotkey;
    if hotkey_changed {
        crate::recenter::apply_hotkey(app, state, &settings.vr_recenter_hotkey)?;
    }
    save_settings(&settings).map_err(|e| e.to_string())?;
    crate::recenter::apply_controller(state, &settings);
    *state.settings.lock() = settings.clone();
    let _ = app.emit("settings-changed", settings.clone());
    Ok(settings)
}

#[tauri::command]
pub fn save_settings_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    settings: AppSettings,
) -> Result<(), String> {
    persist_settings(&app, &state, settings).map(|_| ())
}

/// Shallow-merge `patch` (top-level camelCase keys) into the current settings
/// and persist. Nested objects such as `overlayLayout` are replaced whole.
#[tauri::command]
pub fn patch_settings_cmd(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    patch: serde_json::Value,
) -> Result<AppSettings, String> {
    let current = state.settings.lock().clone();
    let merged = merge_settings_patch(&current, patch)?;
    persist_settings(&app, &state, merged)
}

fn merge_settings_patch(
    current: &AppSettings,
    patch: serde_json::Value,
) -> Result<AppSettings, String> {
    let serde_json::Value::Object(patch) = patch else {
        return Err("Settings patch must be an object".into());
    };
    let mut value = serde_json::to_value(current).map_err(|e| e.to_string())?;
    if let Some(obj) = value.as_object_mut() {
        obj.extend(patch);
    }
    serde_json::from_value(value).map_err(|e| format!("Invalid settings patch: {e}"))
}

#[cfg(test)]
mod settings_patch_tests {
    use super::*;

    #[test]
    fn patch_changes_only_named_keys() {
        let current = AppSettings {
            audio_coach_volume: 0.4,
            ..AppSettings::default()
        };
        let merged =
            merge_settings_patch(&current, serde_json::json!({ "audioFlagsEnabled": false }))
                .unwrap();
        assert!(!merged.audio_flags_enabled);
        assert_eq!(merged.audio_coach_volume, 0.4);
        assert_eq!(merged.vr_mode, current.vr_mode);
    }

    #[test]
    fn patch_rejects_wrong_types_and_non_objects() {
        let current = AppSettings::default();
        assert!(
            merge_settings_patch(&current, serde_json::json!({ "audioFlagsEnabled": "no" }))
                .is_err()
        );
        assert!(merge_settings_patch(&current, serde_json::json!([1, 2])).is_err());
    }
}

// --- VR recenter -------------------------------------------------------------

#[tauri::command]
pub fn vr_recenter_cmd(state: State<'_, Arc<AppState>>) {
    state.vr.request_recenter();
}

/// Wait up to 10 s for the next wheel / button-box press and return it as a
/// binding (not saved; the UI saves it with the rest of the settings).
#[tauri::command]
pub async fn capture_controller_button_cmd(
    state: State<'_, Arc<AppState>>,
) -> Result<Option<ControllerBinding>, String> {
    let Some(input) = state.input.get().cloned() else {
        return Err("Controller input is not available".into());
    };
    tokio::task::spawn_blocking(move || input.capture_next(Duration::from_secs(10)))
        .await
        .map_err(|e| e.to_string())
}

// --- Audio ------------------------------------------------------------------

#[tauri::command]
pub fn start_audio_coach(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    if !state.live.is_running() {
        return Err("Start live monitor or demo clock first".into());
    }
    state.audio.start(state.live.clone());
    Ok(())
}

#[tauri::command]
pub fn stop_audio_coach(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    state.audio.stop();
    Ok(())
}

#[tauri::command]
pub fn get_audio_coach_status(state: State<'_, Arc<AppState>>) -> crate::audio::AudioCoachStatus {
    let settings = state.settings.lock().clone();
    state.audio.status(&settings)
}

#[tauri::command]
pub fn get_audio_coach_message(state: State<'_, Arc<AppState>>) -> String {
    state.audio.last_message()
}

/// Play a lap callout with the active voice pack; reports clips the pack lacks.
#[tauri::command]
pub async fn test_audio_coach(
    state: State<'_, Arc<AppState>>,
) -> Result<crate::audio::PlayReport, String> {
    let audio = state.audio.clone();
    tokio::task::spawn_blocking(move || audio.speak_test())
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}

// --- Monitor overlays -------------------------------------------------------

#[tauri::command]
pub async fn start_monitor_overlay(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let settings = state.settings.lock().clone();
    state.monitor.start(&app, &settings)
}

#[tauri::command]
pub fn stop_monitor_overlay(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    state.monitor.stop(&app);
    Ok(())
}

#[tauri::command]
pub fn get_monitor_overlay_status(
    state: State<'_, Arc<AppState>>,
) -> crate::monitor::MonitorOverlayStatus {
    state.monitor.status()
}

// --- VR ---------------------------------------------------------------------

#[tauri::command]
pub fn start_vr_overlay(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let settings = state.settings.lock().clone();
    state.vr.start(state.live.clone(), settings);
    Ok(())
}

#[tauri::command]
pub fn stop_vr_overlay(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    state.vr.stop();
    Ok(())
}

#[tauri::command]
pub fn get_vr_overlay_status(state: State<'_, Arc<AppState>>) -> VrOverlayStatus {
    state.vr.status()
}

#[tauri::command]
pub fn get_native_vr_status(state: State<'_, Arc<AppState>>) -> NativeVrStatus {
    state.vr.native_status()
}

fn vr_layer_manifest_path(app: &AppHandle) -> Result<String, String> {
    use tauri::Manager;
    let rel = ["resources", "openxr-layer", crate::vr::MANIFEST_FILE];
    let cargo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(dir) = app.path().resource_dir() {
        candidates.push(rel.iter().fold(dir, |acc, p| acc.join(p)));
    }
    // `tauri dev`: files live under src-tauri/resources (resource_dir is often target/debug).
    candidates.push(
        cargo
            .join("resources")
            .join("openxr-layer")
            .join(crate::vr::MANIFEST_FILE),
    );
    // Repo source manifest (DLL must sit beside it — prefer staged resources above).
    candidates.push(
        cargo
            .join("..")
            .join("openxr-layer")
            .join("manifest")
            .join(crate::vr::MANIFEST_FILE),
    );
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(PathBuf::from))
    {
        candidates.push(rel.iter().fold(dir, |acc, p| acc.join(p)));
    }
    let candidate = candidates
        .into_iter()
        .find(|p| p.is_file())
        .ok_or_else(|| {
            "Could not resolve VR layer manifest path. Stage openxr-layer into \
             src-tauri/resources/openxr-layer (see docs/NATIVE_VR.md)."
                .to_string()
        })?;
    Ok(candidate.to_string_lossy().into_owned())
}

/// Bundled voice pack folder: bundled resources in a packaged build, else the
/// `src-tauri` tree under `tauri dev`. `None` when neither holds a `manifest.json`.
pub(crate) fn coach_clips_dir(app: &AppHandle) -> Option<PathBuf> {
    use tauri::Manager;
    let rel = crate::audio::COACH_CLIPS_REL;
    let bundled = app.path().resource_dir().ok().map(|dir| dir.join(rel));
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    bundled
        .into_iter()
        .chain(std::iter::once(dev))
        .find(|dir| dir.join("manifest.json").is_file())
}

#[tauri::command]
pub fn is_vr_layer_installed() -> bool {
    crate::vr::is_layer_installed()
}

#[tauri::command]
pub fn install_vr_layer(app: AppHandle) -> Result<(), String> {
    let path = vr_layer_manifest_path(&app)?;
    crate::vr::install_layer(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn uninstall_vr_layer(app: AppHandle) -> Result<(), String> {
    let path = vr_layer_manifest_path(&app)?;
    crate::vr::uninstall_layer(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_vr_layer_diagnostics(app: AppHandle) -> Result<VrLayerDiagnostics, String> {
    let path = vr_layer_manifest_path(&app)?;
    Ok(crate::vr::layer_diagnostics(&path))
}

#[tauri::command]
pub fn check_vr_hud_health() -> bool {
    crate::vr::check_hud_health()
}

#[tauri::command]
pub fn open_vr_hud_preview_cmd() -> Result<(), String> {
    crate::vr::open_hud_preview()
}
