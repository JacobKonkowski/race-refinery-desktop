//! Voice pack and Voice Studio commands: pack management, sharing, recording,
//! and soundboard / QC playback. Packs are addressed by the same reference the
//! `audioCoachPackId` setting stores (`default`, a user pack id, or a folder path).
use std::path::PathBuf;
use std::sync::Arc;

use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use super::{persist_settings, AppState};
use crate::audio::pack_io::{self, ImportReport};
use crate::audio::phrases::{self, Phrase};
use crate::audio::preview::{self, Composition, PresetInfo, PreviewControl, PreviewStatus};
use crate::audio::record::{self, InputDevice, TakeResult};
use crate::audio::{PackStatus, PlayReport, SpeechPlan, VoicePack};
use crate::settings::AppSettings;

fn err(e: anyhow::Error) -> String {
    format!("{e:#}")
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
) -> Result<T, String> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
        .map_err(err)
}

fn open_pack(state: &AppState, pack: &str) -> Result<VoicePack, String> {
    state.audio.pack_store().resolve(pack).map_err(err)
}

fn settings(state: &AppState) -> AppSettings {
    state.settings.lock().clone()
}

// --- Packs -------------------------------------------------------------------

/// Every phrase a pack can hold, in recording order.
#[tauri::command]
pub fn list_voice_phrases() -> Vec<Phrase> {
    phrases::phrases().to_vec()
}

/// Bundled pack, user packs, then linked folders.
#[tauri::command]
pub fn list_voice_packs(state: State<'_, Arc<AppState>>) -> Vec<PackStatus> {
    let folders = settings(&state).audio_coach_pack_folders;
    state
        .audio
        .pack_store()
        .list(&folders)
        .iter()
        .map(VoicePack::status)
        .collect()
}

#[tauri::command]
pub fn get_voice_pack_status(
    state: State<'_, Arc<AppState>>,
    pack: String,
) -> Result<PackStatus, String> {
    Ok(open_pack(&state, &pack)?.status())
}

#[tauri::command]
pub fn create_voice_pack(
    state: State<'_, Arc<AppState>>,
    name: String,
) -> Result<PackStatus, String> {
    let pack = state.audio.pack_store().create(&name).map_err(err)?;
    Ok(pack.status())
}

/// Copy `source` into a new editable user pack.
#[tauri::command]
pub async fn clone_voice_pack(
    state: State<'_, Arc<AppState>>,
    source: String,
    name: String,
) -> Result<PackStatus, String> {
    let store = state.audio.pack_store();
    blocking(move || {
        let source = store.resolve(&source)?;
        Ok(pack_io::clone_pack(&store, &source, &name)?.status())
    })
    .await
}

/// Delete a user pack; the coach falls back to the bundled pack if it was active.
#[tauri::command]
pub fn delete_voice_pack(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    pack: String,
) -> Result<(), String> {
    state
        .audio
        .pack_store()
        .delete_user_pack(&pack)
        .map_err(err)?;
    let mut next = settings(&state);
    if next.audio_coach_pack_id == pack {
        next.audio_coach_pack_id = crate::audio::pack::BUNDLED_PACK_ID.into();
        persist_settings(&app, &state, next)?;
    }
    Ok(())
}

/// Make `pack` the coach voice.
#[tauri::command]
pub fn set_active_voice_pack(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    pack: String,
) -> Result<AppSettings, String> {
    let resolved = open_pack(&state, &pack)?;
    let mut next = settings(&state);
    next.audio_coach_pack_id = resolved.id;
    persist_settings(&app, &state, next)
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackImport {
    pub pack: PackStatus,
    pub report: ImportReport,
}

/// Pick a pack zip and import it as a new user pack. `None` when cancelled.
#[tauri::command]
pub async fn import_voice_pack_zip(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<Option<PackImport>, String> {
    let store = state.audio.pack_store();
    blocking(move || {
        let Some(file) = app
            .dialog()
            .file()
            .add_filter("Voice pack", &["zip"])
            .blocking_pick_file()
        else {
            return Ok(None);
        };
        let path = file.into_path()?;
        let (pack, report) = pack_io::import_zip(&store, &path)?;
        Ok(Some(PackImport {
            pack: pack.status(),
            report,
        }))
    })
    .await
}

/// Save `pack` as a zip. Returns the written path, `None` when cancelled.
#[tauri::command]
pub async fn export_voice_pack_zip(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    pack: String,
) -> Result<Option<String>, String> {
    let pack = open_pack(&state, &pack)?;
    blocking(move || {
        let suggested = format!("{}.zip", pack.meta.name.trim());
        let Some(file) = app
            .dialog()
            .file()
            .add_filter("Voice pack", &["zip"])
            .set_file_name(suggested)
            .blocking_save_file()
        else {
            return Ok(None);
        };
        let path = file.into_path()?;
        pack_io::export_zip(&pack, &path)?;
        Ok(Some(path.to_string_lossy().into_owned()))
    })
    .await
}

/// Pick a folder and list it as a pack (clips stay where they are).
#[tauri::command]
pub async fn link_voice_pack_folder(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<Option<PackStatus>, String> {
    let picker = app.clone();
    let folder: Option<PathBuf> = blocking(move || {
        picker
            .dialog()
            .file()
            .blocking_pick_folder()
            .map(|f| f.into_path())
            .transpose()
            .map_err(anyhow::Error::from)
    })
    .await?;
    let Some(folder) = folder else {
        return Ok(None);
    };
    let reference = folder.to_string_lossy().into_owned();
    let pack = open_pack(&state, &reference)?;
    let mut next = settings(&state);
    if !next.audio_coach_pack_folders.contains(&reference) {
        next.audio_coach_pack_folders.push(reference);
        persist_settings(&app, &state, next)?;
    }
    Ok(Some(pack.status()))
}

/// Stop listing a linked folder (its files are left alone).
#[tauri::command]
pub fn unlink_voice_pack_folder(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    pack: String,
) -> Result<(), String> {
    let mut next = settings(&state);
    next.audio_coach_pack_folders.retain(|f| f != &pack);
    if next.audio_coach_pack_id == pack {
        next.audio_coach_pack_id = crate::audio::pack::BUNDLED_PACK_ID.into();
    }
    persist_settings(&app, &state, next).map(|_| ())
}

/// Pick a folder of `{key}.wav` files and merge them into `pack`. `None` when cancelled.
#[tauri::command]
pub async fn import_voice_pack_wavs(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    pack: String,
) -> Result<Option<ImportReport>, String> {
    let mut pack = open_pack(&state, &pack)?;
    if pack.read_only() {
        return Err("The bundled pack is read-only. Clone it first.".into());
    }
    blocking(move || {
        let Some(folder) = app.dialog().file().blocking_pick_folder() else {
            return Ok(None);
        };
        let folder = folder.into_path()?;
        Ok(Some(pack_io::import_wav_folder(&mut pack, &folder)?))
    })
    .await
}

// --- Recording ---------------------------------------------------------------

#[tauri::command]
pub async fn list_input_devices() -> Result<Vec<InputDevice>, String> {
    blocking(|| Ok(record::input_devices())).await
}

/// Start capturing from `device` (empty = system default).
#[tauri::command]
pub async fn start_voice_capture(
    state: State<'_, Arc<AppState>>,
    device: String,
) -> Result<(), String> {
    let audio = state.audio.clone();
    blocking(move || audio.recorder.start(&device)).await
}

/// Peak input level (0-1) since the last poll.
#[tauri::command]
pub fn get_voice_capture_level(state: State<'_, Arc<AppState>>) -> f32 {
    state.audio.recorder.take_level()
}

#[tauri::command]
pub fn cancel_voice_capture(state: State<'_, Arc<AppState>>) {
    state.audio.recorder.cancel();
}

/// Stop capturing and save the take as `key` (trim, gate, normalize; keeps undo).
#[tauri::command]
pub async fn finish_voice_take(
    state: State<'_, Arc<AppState>>,
    pack: String,
    key: String,
) -> Result<TakeResult, String> {
    let audio = state.audio.clone();
    let mut pack = open_pack(&state, &pack)?;
    blocking(move || {
        let captured = audio.recorder.stop()?;
        record::save_take(&mut pack, &key, &captured)
    })
    .await
}

/// Restore the clip from before the last take of `key`. `false` if nothing to undo.
#[tauri::command]
pub fn undo_voice_take(
    state: State<'_, Arc<AppState>>,
    pack: String,
    key: String,
) -> Result<bool, String> {
    let mut pack = open_pack(&state, &pack)?;
    record::undo_take(&mut pack, &key).map_err(err)
}

#[tauri::command]
pub fn delete_voice_clip(
    state: State<'_, Arc<AppState>>,
    pack: String,
    key: String,
) -> Result<(), String> {
    let mut pack = open_pack(&state, &pack)?;
    pack.remove_clip(&key).map_err(err)
}

// --- Preview -----------------------------------------------------------------

#[tauri::command]
pub fn list_voice_presets() -> Vec<PresetInfo> {
    preview::presets()
}

fn play_items(state: &AppState, pack: VoicePack, items: Vec<SpeechPlan>) {
    let volume = state.settings.lock().audio_coach_volume;
    state
        .audio
        .preview
        .play(pack, items, volume, state.audio.speak_lock());
}

fn play_one(state: &AppState, pack: &str, plan: SpeechPlan) -> Result<PlayReport, String> {
    let pack = open_pack(state, pack)?;
    let report = PlayReport {
        text: plan.display_text(),
        missing: preview::missing_keys(&pack, &plan),
    };
    play_items(state, pack, vec![plan]);
    Ok(report)
}

#[tauri::command]
pub fn play_voice_clip(
    state: State<'_, Arc<AppState>>,
    pack: String,
    key: String,
) -> Result<PlayReport, String> {
    play_one(&state, &pack, SpeechPlan::clip(key))
}

#[tauri::command]
pub fn play_voice_preset(
    state: State<'_, Arc<AppState>>,
    pack: String,
    preset: String,
) -> Result<PlayReport, String> {
    let plan = preview::preset_plan(&preset).ok_or_else(|| format!("unknown preset '{preset}'"))?;
    play_one(&state, &pack, plan)
}

#[tauri::command]
pub fn play_voice_composition(
    state: State<'_, Arc<AppState>>,
    pack: String,
    composition: Composition,
) -> Result<PlayReport, String> {
    play_one(&state, &pack, preview::compose(&composition))
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub keys: Vec<String>,
    /// Spotter keys the pack has no clip for (left out of the playlist).
    pub missing: Vec<String>,
}

/// Play every recorded Spotter line in registry order.
#[tauri::command]
pub fn play_spotter_playlist(
    state: State<'_, Arc<AppState>>,
    pack: String,
) -> Result<Playlist, String> {
    let pack = open_pack(&state, &pack)?;
    let (keys, missing): (Vec<String>, Vec<String>) = preview::spotter_keys()
        .into_iter()
        .map(String::from)
        .partition(|k| pack.has(k));
    let items = keys.iter().map(|k| SpeechPlan::clip(k.clone())).collect();
    play_items(&state, pack, items);
    Ok(Playlist { keys, missing })
}

#[tauri::command]
pub fn get_voice_preview_status(state: State<'_, Arc<AppState>>) -> PreviewStatus {
    state.audio.preview.status()
}

#[tauri::command]
pub fn control_voice_preview(state: State<'_, Arc<AppState>>, action: PreviewControl) {
    state.audio.preview.control(action);
}
