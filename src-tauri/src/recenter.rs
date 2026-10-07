//! VR recenter inputs: a global keyboard hotkey and a wheel / button-box button.
//! Both call [`VrOverlayService::request_recenter`](crate::vr::VrOverlayService).

use std::sync::Arc;

use tauri::plugin::TauriPlugin;
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::commands::AppState;
use crate::settings::AppSettings;

/// Global-shortcut plugin. Only the recenter accelerator is ever registered, so
/// any press is a recenter.
pub fn hotkey_plugin<R: Runtime>() -> TauriPlugin<R> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                if let Some(state) = app.try_state::<Arc<AppState>>() {
                    state.vr.request_recenter();
                }
            }
        })
        .build()
}

fn parse(accelerator: &str) -> Result<Shortcut, String> {
    accelerator
        .parse::<Shortcut>()
        .map_err(|e| format!("Invalid recenter hotkey \"{accelerator}\": {e}"))
}

/// Replace the registered recenter hotkey with `accelerator` (empty = none).
/// On failure the previous hotkey stays registered.
pub fn apply_hotkey(app: &AppHandle, state: &AppState, accelerator: &str) -> Result<(), String> {
    let mut current = state.recenter_hotkey.lock();
    if current.as_deref().unwrap_or("") == accelerator {
        return Ok(());
    }
    let gs = app.global_shortcut();
    let previous = current.take();
    if let Some(prev) = previous.as_deref() {
        if let Ok(sc) = parse(prev) {
            let _ = gs.unregister(sc);
        }
    }
    if accelerator.is_empty() {
        return Ok(());
    }
    let registered = parse(accelerator).and_then(|sc| {
        gs.register(sc).map_err(|e| {
            format!("Could not register \"{accelerator}\" (another app may own it): {e}")
        })
    });
    match registered {
        Ok(()) => {
            *current = Some(accelerator.to_string());
            Ok(())
        }
        Err(e) => {
            if let Some(prev) = previous {
                if parse(&prev)
                    .and_then(|sc| gs.register(sc).map_err(|e| e.to_string()))
                    .is_ok()
                {
                    *current = Some(prev);
                }
            }
            Err(e)
        }
    }
}

/// Start the controller watcher on the main window and apply saved bindings.
pub fn init(app: &AppHandle, state: &Arc<AppState>) {
    #[cfg(windows)]
    let hwnd = app
        .get_webview_window("main")
        .and_then(|w| w.hwnd().ok())
        .map(|h| h.0 as isize)
        .unwrap_or(0);
    #[cfg(not(windows))]
    let hwnd = 0;

    let vr = state.vr.clone();
    let watcher = race_refinery_input::InputWatcher::start(hwnd, move || vr.request_recenter());
    let _ = state.input.set(watcher);

    let settings = state.settings.lock().clone();
    apply_controller(state, &settings);
    if let Err(e) = apply_hotkey(app, state, &settings.vr_recenter_hotkey) {
        tracing::warn!("{e}");
    }
}

pub fn apply_controller(state: &AppState, settings: &AppSettings) {
    if let Some(input) = state.input.get() {
        input.set_binding(settings.vr_recenter_button.clone());
    }
}
