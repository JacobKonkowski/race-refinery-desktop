/**
 * Tauri IPC wrappers for the Race Refinery backend.
 *
 * Commands use `invoke()`; live/import updates use `listen()` helpers below.
 * Analyze, live, audio, monitor, and VR handlers are registered in
 * `src-tauri/src/commands/mod.rs`.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ask } from "@tauri-apps/plugin-dialog";
import type {
  AppSettings,
  AudioCoachStatus,
  ControllerBinding,
  CornerConsistency,
  ImportStatus,
  InputDevice,
  IracingConfigCheck,
  LapComparison,
  LapTrace,
  LiveSnapshot,
  LiveStatus,
  MonitorOverlayStatus,
  NativeVrStatus,
  SessionDetail,
  SessionSummary,
  TrackOutline,
  VoiceComposition,
  VoiceImportReport,
  VoicePackImport,
  VoicePackStatus,
  VoicePhrase,
  VoicePlaylist,
  VoicePlayReport,
  VoicePreset,
  VoicePreviewControl,
  VoicePreviewStatus,
  VoiceTakeResult,
  VrLayerDiagnostics,
  VrOverlayStatus,
} from "./types";

/* --- Analyze / sessions --- */

export async function listSessions(): Promise<SessionSummary[]> {
  return invoke("list_sessions");
}

export async function getSession(sessionId: number): Promise<SessionDetail | null> {
  return invoke("get_session", { sessionId });
}

export async function getLapTraces(lapIds: number[]): Promise<LapTrace[]> {
  return invoke("get_lap_traces", { lapIds });
}

/** Cached circuit outline for a track; `null` until an IBT with GPS is imported. */
export async function getTrackMap(track: string): Promise<TrackOutline | null> {
  return invoke("get_track_map", { track });
}

export async function compareLaps(
  candidateLapId: number,
  referenceLapId: number,
): Promise<LapComparison> {
  return invoke("compare_laps", { candidateLapId, referenceLapId });
}

/** Each lap's brake point and corner time through the reference lap's corners. */
export async function cornerConsistency(
  referenceLapId: number,
  lapIds: number[],
): Promise<CornerConsistency[]> {
  return invoke("corner_consistency", { referenceLapId, lapIds });
}

export async function importIbt(path: string): Promise<string> {
  return invoke("import_ibt", { path });
}

export async function importFolder(): Promise<number> {
  return invoke("import_folder_cmd");
}

export async function checkIracingConfig(): Promise<IracingConfigCheck> {
  return invoke("check_iracing_config_cmd");
}

export async function getImportStatus(): Promise<ImportStatus> {
  return invoke("get_import_status");
}

export async function pickIbtFile(): Promise<string | null> {
  return invoke("pick_ibt_file");
}

export async function clearDatabase(): Promise<number> {
  return invoke("clear_database_cmd");
}

export async function deleteSession(sessionId: number): Promise<boolean> {
  return invoke("delete_session_cmd", { sessionId });
}

/** Re-parse a session's source IBT with the current analysis; resolves to the new session id. */
export async function reimportSession(sessionId: number): Promise<number> {
  return invoke("reimport_session_cmd", { sessionId });
}

/** Native yes/no dialog (Tauri webview blocks `window.confirm`). */
export async function confirmDialog(
  message: string,
  title = "Confirm",
): Promise<boolean> {
  return ask(message, { title, kind: "warning" });
}

export function onImportComplete(callback: (sessionId: number) => void) {
  return listen<number>("import-complete", (event) => callback(event.payload));
}

export function onImportStatus(callback: (status: ImportStatus) => void) {
  return listen<ImportStatus>("import-status", (event) => callback(event.payload));
}

/* --- Live --- */

export async function startLiveMonitor(): Promise<void> {
  return invoke("start_live_monitor");
}

export async function stopLiveMonitor(): Promise<void> {
  return invoke("stop_live_monitor");
}

export async function startDemoClock(): Promise<void> {
  return invoke("start_demo_clock");
}

export async function stopDemoClock(): Promise<void> {
  return invoke("stop_demo_clock");
}

export async function getLiveStatus(): Promise<LiveStatus> {
  return invoke("get_live_status");
}

export async function getLiveSnapshot(): Promise<LiveSnapshot> {
  return invoke("get_live_snapshot");
}

export function onLiveTelemetry(callback: (snap: LiveSnapshot) => void) {
  return listen<LiveSnapshot>("live-telemetry", (event) => callback(event.payload));
}

export function onLiveStatus(callback: (status: LiveStatus) => void) {
  return listen<LiveStatus>("live-status", (event) => callback(event.payload));
}

/* --- Settings --- */

export async function getSettings(): Promise<AppSettings> {
  return invoke("get_settings");
}

export async function saveSettings(settings: AppSettings): Promise<void> {
  return invoke("save_settings_cmd", { settings });
}

export async function patchSettings(patch: Partial<AppSettings>): Promise<AppSettings> {
  return invoke("patch_settings_cmd", { patch });
}

export function onSettingsChanged(callback: (settings: AppSettings) => void) {
  return listen<AppSettings>("settings-changed", (event) => callback(event.payload));
}

/* --- Audio coach --- */

export async function startAudioCoach(): Promise<void> {
  return invoke("start_audio_coach");
}

export async function stopAudioCoach(): Promise<void> {
  return invoke("stop_audio_coach");
}

export async function getAudioCoachStatus(): Promise<AudioCoachStatus> {
  return invoke("get_audio_coach_status");
}

export async function getAudioCoachMessage(): Promise<string> {
  return invoke("get_audio_coach_message");
}

/** Play a lap callout with the active voice pack; reports clips the pack lacks. */
export async function testAudioCoach(): Promise<VoicePlayReport> {
  return invoke("test_audio_coach");
}

/* --- Voice packs / Voice Studio --- */

export async function listVoicePhrases(): Promise<VoicePhrase[]> {
  return invoke("list_voice_phrases");
}

export async function listVoicePacks(): Promise<VoicePackStatus[]> {
  return invoke("list_voice_packs");
}

export async function getVoicePackStatus(pack: string): Promise<VoicePackStatus> {
  return invoke("get_voice_pack_status", { pack });
}

export async function createVoicePack(name: string): Promise<VoicePackStatus> {
  return invoke("create_voice_pack", { name });
}

export async function cloneVoicePack(source: string, name: string): Promise<VoicePackStatus> {
  return invoke("clone_voice_pack", { source, name });
}

export async function deleteVoicePack(pack: string): Promise<void> {
  return invoke("delete_voice_pack", { pack });
}

export async function setActiveVoicePack(pack: string): Promise<AppSettings> {
  return invoke("set_active_voice_pack", { pack });
}

/** Pick a pack zip and import it as a new user pack; null when cancelled. */
export async function importVoicePackZip(): Promise<VoicePackImport | null> {
  return invoke("import_voice_pack_zip");
}

/** Save a pack as a zip; resolves to the written path, null when cancelled. */
export async function exportVoicePackZip(pack: string): Promise<string | null> {
  return invoke("export_voice_pack_zip", { pack });
}

/** Pick a folder and list it as a voice pack; null when cancelled. */
export async function linkVoicePackFolder(): Promise<VoicePackStatus | null> {
  return invoke("link_voice_pack_folder");
}

export async function unlinkVoicePackFolder(pack: string): Promise<void> {
  return invoke("unlink_voice_pack_folder", { pack });
}

/** Pick a folder of `{key}.wav` files and merge them into a pack; null when cancelled. */
export async function importVoicePackWavs(pack: string): Promise<VoiceImportReport | null> {
  return invoke("import_voice_pack_wavs", { pack });
}

export async function listInputDevices(): Promise<InputDevice[]> {
  return invoke("list_input_devices");
}

export async function startVoiceCapture(device: string): Promise<void> {
  return invoke("start_voice_capture", { device });
}

/** Peak mic level (0-1) since the previous poll. */
export async function getVoiceCaptureLevel(): Promise<number> {
  return invoke("get_voice_capture_level");
}

export async function cancelVoiceCapture(): Promise<void> {
  return invoke("cancel_voice_capture");
}

/** Stop capturing and save the take (trim, gate, normalize; keeps one undo). */
export async function finishVoiceTake(pack: string, key: string): Promise<VoiceTakeResult> {
  return invoke("finish_voice_take", { pack, key });
}

export async function undoVoiceTake(pack: string, key: string): Promise<boolean> {
  return invoke("undo_voice_take", { pack, key });
}

export async function deleteVoiceClip(pack: string, key: string): Promise<void> {
  return invoke("delete_voice_clip", { pack, key });
}

export async function listVoicePresets(): Promise<VoicePreset[]> {
  return invoke("list_voice_presets");
}

export async function playVoiceClip(pack: string, key: string): Promise<VoicePlayReport> {
  return invoke("play_voice_clip", { pack, key });
}

export async function playVoicePreset(pack: string, preset: string): Promise<VoicePlayReport> {
  return invoke("play_voice_preset", { pack, preset });
}

export async function playVoiceComposition(
  pack: string,
  composition: VoiceComposition,
): Promise<VoicePlayReport> {
  return invoke("play_voice_composition", { pack, composition });
}

export async function playSpotterPlaylist(pack: string): Promise<VoicePlaylist> {
  return invoke("play_spotter_playlist", { pack });
}

export async function getVoicePreviewStatus(): Promise<VoicePreviewStatus> {
  return invoke("get_voice_preview_status");
}

export async function controlVoicePreview(action: VoicePreviewControl): Promise<void> {
  return invoke("control_voice_preview", { action });
}

/* --- VR / HUD --- */

export async function startVrOverlay(): Promise<void> {
  return invoke("start_vr_overlay");
}

export async function stopVrOverlay(): Promise<void> {
  return invoke("stop_vr_overlay");
}

export async function getVrOverlayStatus(): Promise<VrOverlayStatus> {
  return invoke("get_vr_overlay_status");
}

export async function getNativeVrStatus(): Promise<NativeVrStatus> {
  return invoke("get_native_vr_status");
}

export async function isVrLayerInstalled(): Promise<boolean> {
  return invoke("is_vr_layer_installed");
}

export async function installVrLayer(): Promise<void> {
  return invoke("install_vr_layer");
}

export async function uninstallVrLayer(): Promise<void> {
  return invoke("uninstall_vr_layer");
}

export async function getVrLayerDiagnostics(): Promise<VrLayerDiagnostics> {
  return invoke("get_vr_layer_diagnostics");
}

export async function checkVrHudHealth(): Promise<boolean> {
  return invoke("check_vr_hud_health");
}

export async function openVrHudPreview(): Promise<void> {
  return invoke("open_vr_hud_preview_cmd");
}

/** Re-anchor world-locked VR widgets to the current head pose. */
export async function recenterVr(): Promise<void> {
  return invoke("vr_recenter_cmd");
}

/** Wait (up to 10 s) for the next wheel / button-box press; null on timeout. */
export async function captureControllerButton(): Promise<ControllerBinding | null> {
  return invoke("capture_controller_button_cmd");
}

/* --- Monitor overlays --- */

export async function startMonitorOverlay(): Promise<void> {
  return invoke("start_monitor_overlay");
}

export async function stopMonitorOverlay(): Promise<void> {
  return invoke("stop_monitor_overlay");
}

export async function getMonitorOverlayStatus(): Promise<MonitorOverlayStatus> {
  return invoke("get_monitor_overlay_status");
}

export function buildOpenKneeboardUrl(settings: AppSettings, baseUrl: string): string {
  const layoutMap: Record<string, string> = {
    coach: "ironman",
    standings: "standings",
    relative: "relative",
    radar: "radar",
    trackmap: "trackmap",
  };
  const kinds = ["coach", "standings", "relative", "radar", "trackmap"];
  const enabled = settings.overlayLayout.widgets
    .map((w, i) => ({ w, kind: kinds[i] }))
    .filter(({ w }) => w.enabled);
  const layout = enabled.length > 0 ? layoutMap[enabled[0].kind] ?? "ironman" : "ironman";
  const pace = settings.overlayLayout.fieldPaceMode || "best";
  const url = new URL(baseUrl.endsWith("/vr") ? baseUrl : `${baseUrl.replace(/\/$/, "")}/vr`);
  url.searchParams.set("layout", layout);
  url.searchParams.set("pace", pace);
  return url.toString();
}
