use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// How much non-critical radio traffic the coach produces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ChatterLevel {
    Minimal,
    #[default]
    Normal,
    Verbose,
}

/// Overlay widget slots, shared by the **monitor** surface and the **VR** compositor.
/// Enable once (`enabled`); place twice (`desktop*` for monitor windows, `vr*` for
/// in-headset). The index of each widget in [`OverlayLayout::widgets`] equals its
/// VR overlay slot and kind (0 = coach, 1 = standings, 2 = relative, 3 = radar,
/// 4 = track map).
pub const WIDGET_COUNT: usize = 5;
pub const WIDGET_COACH: usize = 0;
pub const WIDGET_STANDINGS: usize = 1;
pub const WIDGET_RELATIVE: usize = 2;
pub const WIDGET_RADAR: usize = 3;
pub const WIDGET_TRACK_MAP: usize = 4;

/// Default VR quad scale and opacity. Scale 1.0 fills too much of a Quest FOV at
/// the base poses, so widgets start smaller and semi-transparent.
pub const DEFAULT_VR_SCALE: f32 = 0.55;
pub const DEFAULT_VR_OPACITY: f32 = 0.75;

/// What a VR widget is anchored to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum VrLock {
    /// Fixed in the cockpit, relative to the recenter anchor.
    #[default]
    World,
    /// Follows the head.
    Head,
}

/// A button on a DirectInput game controller (wheel, button box, ...).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControllerBinding {
    /// DirectInput instance GUID, stable per device on this machine.
    pub device_guid: String,
    /// Product name shown in the UI and used as a fallback match.
    pub device_name: String,
    /// Zero-based button index.
    pub button: u32,
}

/// Per-widget visibility and placement. Desktop fields are screen pixels for the
/// monitor host window (`monitor-<kind>`); VR fields are meters / multipliers on
/// top of the per-kind base pose the compositor uses.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WidgetPlacement {
    pub enabled: bool,
    pub desktop_x: f32,
    pub desktop_y: f32,
    pub desktop_w: f32,
    pub desktop_h: f32,
    pub vr_lock: VrLock,
    /// Sideways nudge applied to the widget's VR base pose, in meters (+ = right).
    pub vr_offset_x: f32,
    /// Vertical nudge applied to the widget's VR base pose, in meters.
    pub vr_offset_y: f32,
    /// Depth nudge applied to the widget's VR base pose, in meters (+ = closer).
    pub vr_offset_z: f32,
    /// Pitch of the VR quad in degrees (+ = top tilted away).
    pub vr_tilt_deg: f32,
    /// VR quad scale multiplier.
    pub vr_scale: f32,
    /// VR quad opacity, 0.0ΓÇô1.0.
    pub vr_opacity: f32,
}

impl Default for WidgetPlacement {
    fn default() -> Self {
        Self {
            enabled: false,
            desktop_x: 24.0,
            desktop_y: 24.0,
            desktop_w: 320.0,
            desktop_h: 180.0,
            vr_lock: VrLock::World,
            vr_offset_x: 0.0,
            vr_offset_y: 0.0,
            vr_offset_z: 0.0,
            vr_tilt_deg: 0.0,
            vr_scale: DEFAULT_VR_SCALE,
            vr_opacity: DEFAULT_VR_OPACITY,
        }
    }
}

/// Shared catalog of overlay widgets plus the coach field-pace preference.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct OverlayLayout {
    pub widgets: [WidgetPlacement; WIDGET_COUNT],
    /// Field pace shown on the coach widget: "best", "optimal", or "both".
    pub field_pace_mode: String,
}

impl Default for OverlayLayout {
    fn default() -> Self {
        // Coach is on by default; the rest start disabled but with non-overlapping
        // monitor placements so enabling them lands somewhere sane.
        let mut widgets = [WidgetPlacement::default(); WIDGET_COUNT];
        widgets[WIDGET_COACH] = WidgetPlacement {
            enabled: true,
            desktop_x: 24.0,
            desktop_y: 24.0,
            desktop_w: 360.0,
            desktop_h: 200.0,
            ..WidgetPlacement::default()
        };
        widgets[WIDGET_STANDINGS] = WidgetPlacement {
            desktop_x: 24.0,
            desktop_y: 244.0,
            desktop_w: 320.0,
            desktop_h: 300.0,
            ..WidgetPlacement::default()
        };
        widgets[WIDGET_RELATIVE] = WidgetPlacement {
            desktop_x: 360.0,
            desktop_y: 244.0,
            desktop_w: 300.0,
            desktop_h: 240.0,
            ..WidgetPlacement::default()
        };
        widgets[WIDGET_RADAR] = WidgetPlacement {
            desktop_x: 404.0,
            desktop_y: 24.0,
            desktop_w: 200.0,
            desktop_h: 200.0,
            ..WidgetPlacement::default()
        };
        widgets[WIDGET_TRACK_MAP] = WidgetPlacement {
            desktop_x: 620.0,
            desktop_y: 24.0,
            desktop_w: 320.0,
            desktop_h: 320.0,
            ..WidgetPlacement::default()
        };
        Self {
            widgets,
            field_pace_mode: "best".into(),
        }
    }
}

impl OverlayLayout {
    /// Seed the layout from the legacy single-overlay `vr_*` settings so users
    /// upgrading from the coach-only build keep their HUD placement.
    fn from_legacy(settings: &AppSettings) -> Self {
        let mut layout = OverlayLayout::default();
        let coach = &mut layout.widgets[WIDGET_COACH];
        coach.enabled = true;
        coach.vr_offset_y = settings.vr_hud_offset;
        coach.vr_scale = settings.vr_overlay_scale;
        coach.vr_opacity = settings.vr_hud_opacity;
        layout.field_pace_mode = settings.vr_field_pace_mode.clone();
        layout
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    /// Legacy single-panel geometry (kept for settings migration; prefer overlay_layout).
    pub overlay_x: i32,
    pub overlay_y: i32,
    pub overlay_width: u32,
    pub overlay_height: u32,
    pub vr_overlay_enabled: bool,
    /// Overlay quad scale for the native VR HUD (also the legacy web scale).
    pub vr_overlay_scale: f32,
    /// "native" (in-headset OpenXR layer) or "web" (browser / kneeboard preview).
    pub vr_mode: String,
    /// Vertical placement of the native HUD in meters (positive = higher).
    pub vr_hud_offset: f32,
    /// Native HUD opacity, 0.0-1.0.
    pub vr_hud_opacity: f32,
    /// Optional global recenter hotkey (e.g. "Ctrl+F10"); empty = disabled.
    pub vr_recenter_hotkey: String,
    /// Optional wheel / button-box button that recenters the VR anchor.
    pub vr_recenter_button: Option<ControllerBinding>,
    /// Field pace shown on the HUD: "best", "optimal", or "both".
    pub vr_field_pace_mode: String,
    /// Shared widget catalog for monitor windows and the VR compositor.
    pub overlay_layout: OverlayLayout,
    pub audio_coach_enabled: bool,
    /// Active voice pack: `default` (bundled), a user pack id, or an absolute
    /// folder path. Replaces the retired `audioCoachVoice` / `audioCoachRate`,
    /// which older configs still carry and deserialization ignores.
    pub audio_coach_pack_id: String,
    /// Folders linked as voice packs, so the picker lists them.
    pub audio_coach_pack_folders: Vec<String>,
    /// Voice Studio microphone (cpal input device name); empty = system default.
    pub audio_coach_mic_device: String,
    /// Voice Studio selects the next missing phrase after each saved take.
    #[serde(default = "default_true")]
    pub audio_studio_auto_advance: bool,
    /// Speech volume (0.0-1.0).
    pub audio_coach_volume: f32,
    pub audio_coach_fuel_threshold: f32,
    pub audio_pack_alerts_enabled: bool,
    pub audio_flags_enabled: bool,
    pub audio_incidents_enabled: bool,
    pub audio_fuel_race_enabled: bool,
    pub audio_gap_alerts_enabled: bool,
    pub audio_pace_enabled: bool,
    pub audio_strategy_enabled: bool,
    pub audio_race_clock_enabled: bool,
    pub audio_pits_open_enabled: bool,
    #[serde(default)]
    pub audio_coach_chatter_level: ChatterLevel,
    #[serde(default = "default_true")]
    pub audio_session_intro_enabled: bool,
    #[serde(default = "default_true")]
    pub audio_position_callouts_enabled: bool,
    #[serde(default = "default_true")]
    pub audio_tyre_alerts_enabled: bool,
    #[serde(default = "default_true")]
    pub audio_invalid_lap_enabled: bool,
    #[serde(default)]
    pub audio_radio_effects_enabled: bool,
    #[serde(default = "default_true")]
    pub audio_pack_precursors_enabled: bool,
    #[serde(default = "default_fuel_sensitivity")]
    pub audio_fuel_strategy_sensitivity: String,
    #[serde(default)]
    pub audio_inter_message_gap_ms: i32,
    #[serde(default)]
    pub audio_voice_commands_enabled: bool,
    #[serde(default)]
    pub audio_voice_push_to_talk_key: String,
}

fn default_true() -> bool {
    true
}

fn default_fuel_sensitivity() -> String {
    "normal".into()
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            overlay_x: 100,
            overlay_y: 100,
            overlay_width: 720,
            overlay_height: 520,
            vr_overlay_enabled: false,
            vr_overlay_scale: DEFAULT_VR_SCALE,
            vr_mode: "native".into(),
            vr_hud_offset: 0.0,
            vr_hud_opacity: DEFAULT_VR_OPACITY,
            vr_recenter_hotkey: String::new(),
            vr_recenter_button: None,
            vr_field_pace_mode: "best".into(),
            overlay_layout: OverlayLayout::default(),
            audio_coach_enabled: true,
            audio_coach_pack_id: "default".into(),
            audio_coach_pack_folders: Vec::new(),
            audio_coach_mic_device: String::new(),
            audio_studio_auto_advance: true,
            audio_coach_volume: 1.0,
            audio_coach_fuel_threshold: 5.0,
            audio_pack_alerts_enabled: true,
            audio_flags_enabled: true,
            audio_incidents_enabled: true,
            audio_fuel_race_enabled: true,
            audio_gap_alerts_enabled: true,
            audio_pace_enabled: true,
            audio_strategy_enabled: true,
            audio_race_clock_enabled: true,
            audio_pits_open_enabled: true,
            audio_coach_chatter_level: ChatterLevel::Normal,
            audio_session_intro_enabled: true,
            audio_position_callouts_enabled: true,
            audio_tyre_alerts_enabled: true,
            audio_invalid_lap_enabled: true,
            audio_radio_effects_enabled: false,
            audio_pack_precursors_enabled: true,
            audio_fuel_strategy_sensitivity: "normal".into(),
            audio_inter_message_gap_ms: 0,
            audio_voice_commands_enabled: false,
            audio_voice_push_to_talk_key: String::new(),
        }
    }
}

/// Per-user app data folder (`%LOCALAPPDATA%\race-refinery`).
pub fn data_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("race-refinery")
}

pub fn settings_path() -> PathBuf {
    data_dir().join("settings.json")
}

pub fn load_settings() -> AppSettings {
    let path = settings_path();
    let Ok(content) = fs::read_to_string(&path) else {
        return AppSettings::default();
    };
    let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&content) else {
        return AppSettings::default();
    };
    let had_layout = value.get("overlayLayout").is_some();
    pad_overlay_widgets(&mut value);
    let Ok(mut settings) = serde_json::from_value::<AppSettings>(value) else {
        return AppSettings::default();
    };
    // Upgrade a coach-only config to the shared widget layout once.
    if !had_layout {
        settings.overlay_layout = OverlayLayout::from_legacy(&settings);
    }
    settings
}

/// Resize a stored `overlayLayout.widgets` array to the current slot count.
///
/// The array is fixed-length, so a config written before a slot was added would
/// otherwise fail to deserialize and reset every setting. Missing slots take
/// their default placement.
fn pad_overlay_widgets(value: &mut serde_json::Value) {
    let Some(widgets) = value
        .get_mut("overlayLayout")
        .and_then(|layout| layout.get_mut("widgets"))
        .and_then(|widgets| widgets.as_array_mut())
    else {
        return;
    };
    let defaults = OverlayLayout::default();
    while widgets.len() < WIDGET_COUNT {
        let Ok(placement) = serde_json::to_value(defaults.widgets[widgets.len()]) else {
            return;
        };
        widgets.push(placement);
    }
    widgets.truncate(WIDGET_COUNT);
}

pub fn save_settings(settings: &AppSettings) -> anyhow::Result<()> {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(settings)?;
    fs::write(path, json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A config written by the four-slot build, with a customized coach slot.
    fn legacy_four_slot_json() -> serde_json::Value {
        let defaults = OverlayLayout::default();
        let mut coach = serde_json::to_value(defaults.widgets[WIDGET_COACH]).unwrap();
        coach["desktopX"] = serde_json::json!(999.0);
        serde_json::json!({
            "overlayLayout": {
                "widgets": [
                    coach,
                    serde_json::to_value(defaults.widgets[WIDGET_STANDINGS]).unwrap(),
                    serde_json::to_value(defaults.widgets[WIDGET_RELATIVE]).unwrap(),
                    serde_json::to_value(defaults.widgets[WIDGET_RADAR]).unwrap(),
                ],
                "fieldPaceMode": "optimal"
            },
            "audioCoachVolume": 0.25
        })
    }

    #[test]
    fn older_layouts_are_padded_not_discarded() {
        let mut value = legacy_four_slot_json();
        pad_overlay_widgets(&mut value);

        let settings: AppSettings = serde_json::from_value(value).expect("deserialize");
        assert_eq!(settings.overlay_layout.widgets.len(), WIDGET_COUNT);
        // Existing placement and unrelated settings survive the migration.
        assert_eq!(
            settings.overlay_layout.widgets[WIDGET_COACH].desktop_x,
            999.0
        );
        assert_eq!(settings.overlay_layout.field_pace_mode, "optimal");
        assert_eq!(settings.audio_coach_volume, 0.25);
        // The new slot lands on its default placement, disabled.
        assert!(!settings.overlay_layout.widgets[WIDGET_TRACK_MAP].enabled);
        assert_eq!(
            settings.overlay_layout.widgets[WIDGET_TRACK_MAP].desktop_w,
            320.0
        );
    }

    #[test]
    fn longer_layouts_are_truncated() {
        let mut value = legacy_four_slot_json();
        let extra = serde_json::to_value(WidgetPlacement::default()).unwrap();
        for _ in 0..3 {
            value["overlayLayout"]["widgets"]
                .as_array_mut()
                .unwrap()
                .push(extra.clone());
        }
        pad_overlay_widgets(&mut value);

        assert_eq!(
            value["overlayLayout"]["widgets"].as_array().unwrap().len(),
            WIDGET_COUNT
        );
    }

    #[test]
    fn widgets_without_vr_placement_fields_default_to_world_lock() {
        let value = serde_json::json!({
            "overlayLayout": {
                "widgets": [{ "enabled": true, "vrOffsetY": 0.1, "vrScale": 0.8 }],
            },
        });
        let mut value = value;
        pad_overlay_widgets(&mut value);
        let settings: AppSettings = serde_json::from_value(value).expect("deserialize");
        let coach = settings.overlay_layout.widgets[WIDGET_COACH];
        assert_eq!(coach.vr_lock, VrLock::World);
        assert_eq!(coach.vr_offset_x, 0.0);
        assert_eq!(coach.vr_offset_z, 0.0);
        assert_eq!(coach.vr_tilt_deg, 0.0);
        assert_eq!(coach.vr_offset_y, 0.1);
        assert_eq!(coach.vr_scale, 0.8);
        assert!(settings.vr_recenter_button.is_none());
    }

    #[test]
    fn recenter_button_round_trips() {
        let settings = AppSettings {
            vr_recenter_button: Some(ControllerBinding {
                device_guid: "{abc}".into(),
                device_name: "Wheel".into(),
                button: 7,
            }),
            ..AppSettings::default()
        };
        let json = serde_json::to_value(&settings).unwrap();
        assert_eq!(json["vrRecenterButton"]["button"], 7);
        assert_eq!(json["overlayLayout"]["widgets"][0]["vrLock"], "world");
        let back: AppSettings = serde_json::from_value(json).unwrap();
        assert_eq!(back.vr_recenter_button, settings.vr_recenter_button);
    }

    #[test]
    fn retired_voice_settings_are_ignored() {
        let value = serde_json::json!({
            "audioCoachVoice": "Microsoft Guy",
            "audioCoachRate": 1.4,
            "audioCoachVolume": 0.5,
        });
        let settings: AppSettings = serde_json::from_value(value).expect("deserialize");
        assert_eq!(settings.audio_coach_volume, 0.5);
        assert_eq!(settings.audio_coach_pack_id, "default");
        assert!(settings.audio_studio_auto_advance);
        let json = serde_json::to_value(&settings).unwrap();
        assert!(json.get("audioCoachVoice").is_none());
        assert!(json.get("audioCoachRate").is_none());
    }

    #[test]
    fn configs_without_a_layout_are_untouched() {
        let mut value = serde_json::json!({ "audioCoachEnabled": true });
        pad_overlay_widgets(&mut value);
        assert!(value.get("overlayLayout").is_none());
    }
}
