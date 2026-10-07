//! Race Refinery — Tauri backend library.
//!
//! Composition root: domain crates (`race_refinery_telemetry`, `race_refinery_analysis`,
//! `race_refinery_ingest`, `race_refinery_storage`, `race_refinery_live`, `race_refinery_audio`,
//! `race_refinery_vr`, `race_refinery_settings`, `race_refinery_monitor`) plus [`commands`] IPC.
//! Domains must not depend on `commands`.

pub mod commands;
mod recenter;

pub use race_refinery_analysis as analysis;
pub use race_refinery_audio as audio;
pub use race_refinery_ingest as ingest;
pub use race_refinery_live as live;
pub use race_refinery_monitor as monitor;
pub use race_refinery_settings as settings;
pub use race_refinery_storage as storage;
pub use race_refinery_telemetry as telemetry;
pub use race_refinery_vr as vr;

use std::sync::Arc;

use crate::commands::AppState;
use race_refinery_ingest::start_watcher;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                tracing_subscriber::EnvFilter::new("race_refinery_desktop_lib=info,pitwall=warn")
            }),
        )
        .try_init();

    let state = Arc::new(AppState::new().expect("failed to initialize database"));

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(recenter::hotkey_plugin())
        .manage(state.clone())
        .setup(move |app| {
            #[cfg(feature = "updater")]
            {
                app.handle()
                    .plugin(tauri_plugin_updater::Builder::new().build())?;
            }
            if let Some(dir) = commands::coach_clips_dir(app.handle()) {
                state.audio.set_clips_dir(dir);
            }
            start_watcher(app.handle().clone(), state.import.clone());
            recenter::init(app.handle(), &state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_sessions,
            commands::get_session,
            commands::get_track_map,
            commands::get_lap_traces,
            commands::compare_laps,
            commands::corner_consistency,
            commands::import_ibt,
            commands::import_folder_cmd,
            commands::reimport_session_cmd,
            commands::check_iracing_config_cmd,
            commands::get_import_status,
            commands::pick_ibt_file,
            commands::clear_database_cmd,
            commands::delete_session_cmd,
            commands::start_live_monitor,
            commands::stop_live_monitor,
            commands::get_live_status,
            commands::get_live_snapshot,
            commands::start_demo_clock,
            commands::stop_demo_clock,
            commands::get_settings,
            commands::save_settings_cmd,
            commands::patch_settings_cmd,
            commands::start_audio_coach,
            commands::stop_audio_coach,
            commands::get_audio_coach_status,
            commands::get_audio_coach_message,
            commands::test_audio_coach,
            commands::voice::list_voice_phrases,
            commands::voice::list_voice_packs,
            commands::voice::get_voice_pack_status,
            commands::voice::create_voice_pack,
            commands::voice::clone_voice_pack,
            commands::voice::delete_voice_pack,
            commands::voice::set_active_voice_pack,
            commands::voice::import_voice_pack_zip,
            commands::voice::export_voice_pack_zip,
            commands::voice::link_voice_pack_folder,
            commands::voice::unlink_voice_pack_folder,
            commands::voice::import_voice_pack_wavs,
            commands::voice::list_input_devices,
            commands::voice::start_voice_capture,
            commands::voice::get_voice_capture_level,
            commands::voice::cancel_voice_capture,
            commands::voice::finish_voice_take,
            commands::voice::undo_voice_take,
            commands::voice::delete_voice_clip,
            commands::voice::list_voice_presets,
            commands::voice::play_voice_clip,
            commands::voice::play_voice_preset,
            commands::voice::play_voice_composition,
            commands::voice::play_spotter_playlist,
            commands::voice::get_voice_preview_status,
            commands::voice::control_voice_preview,
            commands::start_monitor_overlay,
            commands::stop_monitor_overlay,
            commands::get_monitor_overlay_status,
            commands::start_vr_overlay,
            commands::stop_vr_overlay,
            commands::get_vr_overlay_status,
            commands::get_native_vr_status,
            commands::is_vr_layer_installed,
            commands::install_vr_layer,
            commands::uninstall_vr_layer,
            commands::get_vr_layer_diagnostics,
            commands::check_vr_hud_health,
            commands::open_vr_hud_preview_cmd,
            commands::vr_recenter_cmd,
            commands::capture_controller_button_cmd,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
