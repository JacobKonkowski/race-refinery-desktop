# API

Frontend IPC lives in `src/shared/api.ts` and `src/shared/types.ts`. TypeDoc: `npm run docs:api` (entry points under `src/shared`).

Backend commands are registered in `src-tauri/src/lib.rs` from `commands/mod.rs` and `commands/voice.rs`.

**69 commands** covering Analyze storage, Live, settings, audio coach, voice packs, monitor overlays, and VR/HUD.

## Analyze / storage

| Command | TS helper | Notes |
|---------|-----------|--------|
| `list_sessions` | `listSessions` | Session summaries (display cleanup applied) |
| `get_session` | `getSession` | Session + laps (display cleanup applied) |
| `get_lap_traces` | `getLapTraces` | Trace points for one lap |
| `get_track_map` | `getTrackMap` | Cached circuit outline for a track; `null` when none generated |
| `compare_laps` | `compareLaps` | Two-lap comparison: sectors, aligned traces, running delta, corners with per-lap technique, ABS spans |
| `corner_consistency` | `cornerConsistency` | Each given lap's brake point and corner time through the reference lap's corners |
| `import_ibt` | `importIbt` | Single file (pipeline cleanup on write) |
| `import_folder_cmd` | `importFolder` | Folder scan |
| `check_iracing_config_cmd` | `checkIracingConfig` | mem/disk flags |
| `get_import_status` | `getImportStatus` | Watcher / import progress |
| `pick_ibt_file` | `pickIbtFile` | Dialog |
| `clear_database_cmd` | `clearDatabase` | Debug wipe |
| `delete_session_cmd` | `deleteSession` | Per-session delete |
| `reimport_session_cmd` | `reimportSession` | Re-parse a session's IBT with the current analysis; returns the new session id |

## Live

| Command | TS helper | Notes |
|---------|-----------|--------|
| `start_live_monitor` | `startLiveMonitor` | |
| `stop_live_monitor` | `stopLiveMonitor` | |
| `get_live_status` | `getLiveStatus` | |
| `get_live_snapshot` | `getLiveSnapshot` | |
| `start_demo_clock` | `startDemoClock` | Offline exercise |
| `stop_demo_clock` | `stopDemoClock` | |

## Settings

| Command | TS helper | Notes |
|---------|-----------|--------|
| `get_settings` | `getSettings` | Full `AppSettings` |
| `save_settings_cmd` | `saveSettings` | Full write; applies recenter bindings; emits `settings-changed` |
| `patch_settings_cmd` | `patchSettings` | Merge top-level camelCase keys; nested objects replaced whole |

## Audio

| Command | TS helper | Notes |
|---------|-----------|--------|
| `start_audio_coach` | `startAudioCoach` | |
| `stop_audio_coach` | `stopAudioCoach` | |
| `get_audio_coach_status` | `getAudioCoachStatus` | Active flag, last message, active pack id / name, Spotter and Engineer counts |
| `get_audio_coach_message` | `getAudioCoachMessage` | |
| `test_audio_coach` | `testAudioCoach` | Sample lap callout with the active pack; returns `{ text, missing }` |

## Voice packs

Commands live in `src-tauri/src/commands/voice.rs`. See [VOICE_PACKS.md](VOICE_PACKS.md).

| Command | TS helper | Notes |
|---------|-----------|--------|
| `list_voice_phrases` | `listVoicePhrases` | Phrase registry: key, prompt, tier, category |
| `list_voice_packs` | `listVoicePacks` | Bundled, user, and linked folder packs with status |
| `get_voice_pack_status` | `getVoicePackStatus` | Tier counts, missing keys, read-only flag |
| `create_voice_pack` | `createVoicePack` | Empty user pack |
| `clone_voice_pack` | `cloneVoicePack` | Copy a pack into a new user pack |
| `delete_voice_pack` | `deleteVoicePack` | User packs only; resets the active pack to `default` if needed |
| `set_active_voice_pack` | `setActiveVoicePack` | Sets `audioCoachPackId`; returns the saved settings |
| `import_voice_pack_zip` | `importVoicePackZip` | File dialog; new user pack + import report, or `null` when cancelled |
| `export_voice_pack_zip` | `exportVoicePackZip` | Save dialog; returns the written path or `null` |
| `link_voice_pack_folder` | `linkVoicePackFolder` | Folder dialog; adds to `audioCoachPackFolders` |
| `unlink_voice_pack_folder` | `unlinkVoicePackFolder` | Forgets a linked folder (files untouched) |
| `import_voice_pack_wavs` | `importVoicePackWavs` | Folder dialog; adds `{key}.wav` files to a writable pack |
| `list_input_devices` | `listInputDevices` | Microphones |
| `start_voice_capture` | `startVoiceCapture` | Start recording a take |
| `get_voice_capture_level` | `getVoiceCaptureLevel` | Input peak since the last poll (0–1) |
| `cancel_voice_capture` | `cancelVoiceCapture` | Discard the take in progress |
| `finish_voice_take` | `finishVoiceTake` | Stop, clean, and save the take as a clip; returns duration, peak, warnings |
| `undo_voice_take` | `undoVoiceTake` | Restore the clip before the last take |
| `delete_voice_clip` | `deleteVoiceClip` | Remove one clip from a pack |
| `list_voice_presets` | `listVoicePresets` | Preview presets |
| `play_voice_clip` | `playVoiceClip` | Play one clip |
| `play_voice_preset` | `playVoicePreset` | Play a preset; returns `{ text, missing }` |
| `play_voice_composition` | `playVoiceComposition` | Play composer values; returns `{ text, missing }` |
| `play_spotter_playlist` | `playSpotterPlaylist` | Play recorded Spotter lines in order; returns `{ keys, missing }` |
| `get_voice_preview_status` | `getVoicePreviewStatus` | Playing / paused, index, text |
| `control_voice_preview` | `controlVoicePreview` | `pause` / `resume` / `skip` / `stop` |

## Monitor overlays

| Command | TS helper | Notes |
|---------|-----------|--------|
| `start_monitor_overlay` | `startMonitorOverlay` | One always-on-top window per enabled widget |
| `stop_monitor_overlay` | `stopMonitorOverlay` | |
| `get_monitor_overlay_status` | `getMonitorOverlayStatus` | Active flag, message, open labels |

## VR / HUD

| Command | TS helper | Notes |
|---------|-----------|--------|
| `start_vr_overlay` | `startVrOverlay` | Native or web per settings |
| `stop_vr_overlay` | `stopVrOverlay` | |
| `get_vr_overlay_status` | `getVrOverlayStatus` | |
| `get_native_vr_status` | `getNativeVrStatus` | Write age, overlay count, last error |
| `is_vr_layer_installed` | `isVrLayerInstalled` | |
| `install_vr_layer` | `installVrLayer` | |
| `uninstall_vr_layer` | `uninstallVrLayer` | |
| `get_vr_layer_diagnostics` | `getVrLayerDiagnostics` | Ready / DLL / issues |
| `check_vr_hud_health` | `checkVrHudHealth` | Web HUD health |
| `open_vr_hud_preview_cmd` | `openVrHudPreview` | Opens browser preview |
| `vr_recenter_cmd` | `recenterVr` | Bump SHM `recenter_seq`; layer re-anchors world-locked widgets |
| `capture_controller_button_cmd` | `captureControllerButton` | Wait up to 10 s for the next DirectInput button press |

## Events

- Live snapshot / status updates (see Live page listeners)
- `settings-changed` after save

## Notes for contributors

Keep `src/shared/api.ts` in sync with every `#[tauri::command]` registered in `lib.rs`
(`scripts/check-ipc-drift.ps1` scans every file under `src-tauri/src/commands/`).

## Capabilities

Main and monitor windows (`main`, `monitor-*`) use `src-tauri/capabilities/default.json`.