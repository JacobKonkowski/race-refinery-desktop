//! Desktop monitor overlay host.
//!
//! Opens one always-on-top transparent Tauri window per enabled overlay widget
//! (`monitor-coach`, `monitor-standings`, `monitor-relative`, `monitor-radar`).
//! Placement comes from [`AppSettings::overlay_layout`] desktop fields; the
//! shared `enabled` flag also drives the VR surface (enable once, place twice).

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

use race_refinery_settings::{
    AppSettings, WIDGET_COACH, WIDGET_COUNT, WIDGET_RADAR, WIDGET_RELATIVE, WIDGET_STANDINGS,
    WIDGET_TRACK_MAP,
};

/// Stable widget kinds in overlay-slot order.
pub const WIDGET_KINDS: [&str; WIDGET_COUNT] =
    ["coach", "standings", "relative", "radar", "trackmap"];

/// Window label prefix (`monitor-coach`, …).
pub const WINDOW_LABEL_PREFIX: &str = "monitor-";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MonitorOverlayStatus {
    pub active: bool,
    pub message: String,
    /// Open window labels (e.g. `monitor-coach`).
    #[serde(default)]
    pub windows: Vec<String>,
}

/// Controller for per-widget desktop monitor windows.
pub struct MonitorOverlayService {
    status: Mutex<MonitorOverlayStatus>,
}

impl MonitorOverlayService {
    pub fn new() -> Self {
        Self {
            status: Mutex::new(MonitorOverlayStatus {
                active: false,
                message: "Monitor overlay stopped".into(),
                windows: Vec::new(),
            }),
        }
    }

    pub fn is_running(&self) -> bool {
        self.status.lock().active
    }

    pub fn status(&self) -> MonitorOverlayStatus {
        self.status.lock().clone()
    }

    /// Create always-on-top transparent windows for each enabled widget.
    pub fn start(&self, app: &AppHandle, settings: &AppSettings) -> Result<(), String> {
        self.stop(app);

        let mut opened = Vec::new();
        for (idx, kind) in WIDGET_KINDS.iter().enumerate() {
            let placement = settings
                .overlay_layout
                .widgets
                .get(idx)
                .copied()
                .unwrap_or_default();
            if !placement.enabled {
                continue;
            }

            let label = window_label(kind);
            let title = format!("Race Refinery {}", kind_title(kind));
            let w = placement.desktop_w.max(120.0) as f64;
            let h = placement.desktop_h.max(80.0) as f64;
            let x = placement.desktop_x as f64;
            let y = placement.desktop_y as f64;

            // Same HTML entry for every slot; the frontend reads the window label.
            WebviewWindowBuilder::new(app, &label, WebviewUrl::App("monitor.html".into()))
                .title(title)
                .inner_size(w, h)
                .position(x, y)
                .decorations(false)
                .transparent(true)
                .always_on_top(true)
                .skip_taskbar(true)
                .resizable(true)
                .visible(true)
                .build()
                .map_err(|e| format!("Failed to open {label}: {e}"))?;

            opened.push(label);
        }

        if opened.is_empty() {
            *self.status.lock() = MonitorOverlayStatus {
                active: false,
                message: "No widgets enabled — turn on slots under Overlay widgets".into(),
                windows: Vec::new(),
            };
            return Err(
                "No widgets enabled in overlay layout. Enable at least one widget first.".into(),
            );
        }

        let count = opened.len();
        *self.status.lock() = MonitorOverlayStatus {
            active: true,
            message: format!("Monitor overlay active ({count} window(s))"),
            windows: opened,
        };
        tracing::info!(count, "monitor overlay started");
        Ok(())
    }

    /// Close all monitor windows and clear status.
    pub fn stop(&self, app: &AppHandle) {
        let labels: Vec<String> = WIDGET_KINDS.iter().map(|k| window_label(k)).collect();
        for label in &labels {
            if let Some(win) = app.get_webview_window(label) {
                let _ = win.close();
            }
        }
        *self.status.lock() = MonitorOverlayStatus {
            active: false,
            message: "Monitor overlay stopped".into(),
            windows: Vec::new(),
        };
    }
}

impl Default for MonitorOverlayService {
    fn default() -> Self {
        Self::new()
    }
}

/// `monitor-coach`, `monitor-standings`, …
pub fn window_label(kind: &str) -> String {
    format!("{WINDOW_LABEL_PREFIX}{kind}")
}

/// Parse `monitor-coach` → `Some("coach")`.
pub fn kind_from_window_label(label: &str) -> Option<&str> {
    let kind = label.strip_prefix(WINDOW_LABEL_PREFIX)?;
    if WIDGET_KINDS.contains(&kind) {
        Some(kind)
    } else {
        None
    }
}

fn kind_title(kind: &str) -> &'static str {
    match kind {
        "coach" => "Coach HUD",
        "standings" => "Standings",
        "relative" => "Relative",
        "radar" => "Radar",
        "trackmap" => "Track Map",
        _ => "Widget",
    }
}

/// Slot index for a widget kind name.
pub fn kind_index(kind: &str) -> Option<usize> {
    match kind {
        "coach" => Some(WIDGET_COACH),
        "standings" => Some(WIDGET_STANDINGS),
        "relative" => Some(WIDGET_RELATIVE),
        "radar" => Some(WIDGET_RADAR),
        "trackmap" => Some(WIDGET_TRACK_MAP),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_round_trip() {
        for kind in WIDGET_KINDS {
            let label = window_label(kind);
            assert_eq!(kind_from_window_label(&label), Some(kind));
            assert!(label.starts_with(WINDOW_LABEL_PREFIX));
        }
    }

    #[test]
    fn rejects_unknown_label() {
        assert_eq!(kind_from_window_label("main"), None);
        assert_eq!(kind_from_window_label("monitor-foo"), None);
        assert_eq!(kind_from_window_label("monitor-"), None);
    }

    #[test]
    fn kind_indices_match_settings_slots() {
        assert_eq!(kind_index("coach"), Some(WIDGET_COACH));
        assert_eq!(kind_index("standings"), Some(WIDGET_STANDINGS));
        assert_eq!(kind_index("relative"), Some(WIDGET_RELATIVE));
        assert_eq!(kind_index("radar"), Some(WIDGET_RADAR));
        assert_eq!(kind_index("trackmap"), Some(WIDGET_TRACK_MAP));
        assert_eq!(kind_index("other"), None);
    }
}
