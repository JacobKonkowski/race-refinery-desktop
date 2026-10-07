//! Audio coach: human-recorded voice packs stitched into callouts.
//!
//! - [`phrases`] — registry of every clip key, its prompt, and its Spotter / Engineer tier
//! - [`pack`] — pack folders (`meta.json`, `manifest.json`, `{key}.wav`) and the active-pack resolver
//! - [`pack_io`] — clone, zip import / export, WAV folder import
//! - [`record`] — Voice Studio microphone capture and take saving (clean-up + undo)
//! - [`preview`] — soundboard presets / composer and the Spotter QC playlist
//! - `player` — clips + pauses with crossfades; missing clips are skipped with a warning
//! - [`engine`] — race rules that decide what to say
//!
//! There is no speech synthesis: a callout whose clips are all missing stays silent.
mod coach;
mod dsp;
pub mod engine;
pub mod pack;
pub mod pack_io;
pub mod phrases;
mod phrasing;
mod player;
pub mod preview;
mod queue;
pub mod record;
mod session_mode;
mod speech;

pub use engine::{RaceContext, RaceEngine, RuleSet, SessionMeta};
pub use pack::{PackStatus, PackStore, VoicePack};
pub use speech::SpeechPlan;

use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use parking_lot::Mutex;
use serde::Serialize;
use tokio_util::sync::CancellationToken;

use race_refinery_live::LiveService;
use race_refinery_settings::{load_settings, AppSettings};

use coach::CoachEngine;
use pack::TierCount;
use player::AudioPlayer;
use preview::PreviewPlayer;
use queue::SpeechQueue;
use record::Recorder;
use speech::SpeechUnit;

/// Bundled voice pack folder relative to a resource root.
pub const COACH_CLIPS_REL: &str = "resources/audio/coach/default";

pub struct AudioCoachService {
    cancel: Mutex<Option<CancellationToken>>,
    active: Mutex<bool>,
    last_message: Mutex<String>,
    bundled_dir: Mutex<Option<PathBuf>>,
    user_root: PathBuf,
    /// Held while any clip plays, so the coach and Voice Studio never overlap.
    speak_lock: Arc<Mutex<()>>,
    pub recorder: Recorder,
    pub preview: PreviewPlayer,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AudioCoachStatus {
    pub active: bool,
    pub last_message: String,
    pub pack_id: String,
    pub pack_name: String,
    pub spotter: TierCount,
    pub engineer: TierCount,
}

/// What a coach test or soundboard callout played.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayReport {
    pub text: String,
    /// Keys the pack has no clip for; they were skipped.
    pub missing: Vec<String>,
}

impl AudioCoachService {
    pub fn new() -> Self {
        Self::with_user_root(PackStore::default_user_root())
    }

    pub fn with_user_root(user_root: PathBuf) -> Self {
        Self {
            cancel: Mutex::new(None),
            active: Mutex::new(false),
            last_message: Mutex::new(String::new()),
            bundled_dir: Mutex::new(None),
            user_root,
            speak_lock: Arc::new(Mutex::new(())),
            recorder: Recorder::new(),
            preview: PreviewPlayer::new(),
        }
    }

    /// Bundled pack folder the host resolved; tried before workspace fallbacks.
    pub fn set_clips_dir(&self, dir: PathBuf) {
        *self.bundled_dir.lock() = Some(dir);
    }

    /// Bundled pack plus user packs under the app data folder.
    pub fn pack_store(&self) -> PackStore {
        PackStore::new(bundled_pack_dir(self), self.user_root.clone())
    }

    /// The pack the coach speaks with right now.
    pub fn active_pack(&self, settings: &AppSettings) -> VoicePack {
        self.pack_store()
            .resolve_or_bundled(&settings.audio_coach_pack_id)
    }

    pub fn speak_lock(&self) -> Arc<Mutex<()>> {
        Arc::clone(&self.speak_lock)
    }

    pub fn is_active(&self) -> bool {
        self.cancel.lock().is_some()
    }

    pub fn status(&self, settings: &AppSettings) -> AudioCoachStatus {
        let pack = self.active_pack(settings).status();
        AudioCoachStatus {
            active: *self.active.lock(),
            last_message: self.last_message.lock().clone(),
            pack_id: pack.id,
            pack_name: pack.name,
            spotter: pack.spotter,
            engineer: pack.engineer,
        }
    }

    pub fn stop(&self) {
        if let Some(token) = self.cancel.lock().take() {
            token.cancel();
        }
        *self.active.lock() = false;
    }

    pub fn start(self: &Arc<Self>, live: Arc<LiveService>) {
        if self.is_active() {
            return;
        }
        let token = CancellationToken::new();
        *self.cancel.lock() = Some(token.clone());
        *self.active.lock() = true;

        let service = Arc::clone(self);
        thread::spawn(move || {
            if let Err(e) = run_audio_loop(service.clone(), live, token) {
                tracing::warn!("Audio coach stopped: {e:#}");
            }
            *service.active.lock() = false;
            *service.cancel.lock() = None;
        });
    }

    pub fn last_message(&self) -> String {
        self.last_message.lock().clone()
    }

    /// Play a lap callout with the active pack, as on track. Blocks until done.
    pub fn speak_test(&self) -> anyhow::Result<PlayReport> {
        let settings = load_settings();
        let mut units = Vec::new();
        if settings.audio_radio_effects_enabled {
            units.push(SpeechUnit::Clip(phrases::RADIO_BEEP_KEY.into()));
        }
        units.push(SpeechUnit::Clip("lap".into()));
        phrasing::push_lap_time_callout(&mut units, 12, 89_452.0);
        phrasing::push_delta(&mut units, -300.0);
        let plan = SpeechPlan::sequence(units);
        let player = AudioPlayer::new(self.active_pack(&settings), settings.audio_coach_volume)?;
        let missing = play(&player, self, &plan)?;
        Ok(PlayReport {
            text: plan.display_text(),
            missing,
        })
    }
}

impl Default for AudioCoachService {
    fn default() -> Self {
        Self::new()
    }
}

/// `rel` inside the repo's `src-tauri` tree, which `tauri dev` and tests run against.
fn workspace_resource_dir(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../src-tauri")
        .join(rel)
}

/// The bundled pack committed in the repo.
fn workspace_clips_dir() -> PathBuf {
    workspace_resource_dir(COACH_CLIPS_REL)
}

/// Bundled pack folders in lookup order: host override, repo `src-tauri`, then
/// beside the running executable.
fn clip_dir_candidates(override_dir: Option<PathBuf>) -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = override_dir.into_iter().collect();
    candidates.push(workspace_clips_dir());
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(PathBuf::from))
    {
        candidates.push(dir.join(COACH_CLIPS_REL));
    }
    candidates
}

fn bundled_pack_dir(service: &AudioCoachService) -> PathBuf {
    let override_dir = service.bundled_dir.lock().clone();
    clip_dir_candidates(override_dir)
        .into_iter()
        .find(|dir| dir.join("manifest.json").is_file())
        .unwrap_or_else(workspace_clips_dir)
}

fn run_audio_loop(
    service: Arc<AudioCoachService>,
    live: Arc<LiveService>,
    cancel: CancellationToken,
) -> anyhow::Result<()> {
    let settings = load_settings();
    let mut player = AudioPlayer::new(service.active_pack(&settings), settings.audio_coach_volume)?;
    let mut engine = CoachEngine::new();
    let mut queue = SpeechQueue::new(3);

    while !cancel.is_cancelled() {
        let settings = load_settings();

        if let Some(meta) = live.session_meta.lock().clone() {
            engine.set_session_meta(meta);
        }

        let snap = live.snapshot.lock().clone();
        if let Some(plan) = engine.poll(&snap, &settings) {
            queue.push(plan.0, plan.1);
        }

        if let Some(plan) = queue.pop() {
            if cancel.is_cancelled() {
                break;
            }
            // Re-read the pack per callout so new recordings and pack switches apply live.
            player.set_pack(service.active_pack(&settings));
            player.set_volume(settings.audio_coach_volume);
            play(&player, &service, &plan)?;
            if let Some(plan) = engine.poll(&snap, &settings) {
                queue.push(plan.0, plan.1);
            }
            let gap_ms = settings.audio_inter_message_gap_ms.max(0) as u64;
            thread::sleep(Duration::from_millis(200 + gap_ms));
            continue;
        }

        thread::sleep(Duration::from_millis(250));
    }
    Ok(())
}

fn play(
    player: &AudioPlayer,
    service: &AudioCoachService,
    plan: &SpeechPlan,
) -> anyhow::Result<Vec<String>> {
    let line = plan.display_text();
    tracing::info!("Audio coach: {line}");
    *service.last_message.lock() = line;
    let _speaking = service.speak_lock.lock();
    player.play_plan(plan)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::speech::SpeechPlan;
    use super::{bundled_pack_dir, clip_dir_candidates, workspace_clips_dir, AudioCoachService};

    #[test]
    fn display_text_sequence() {
        let plan = SpeechPlan::sequence(vec![]);
        assert_eq!(plan.display_text(), "");
    }

    #[test]
    fn bundled_pack_records_every_phrase() {
        let dir = workspace_clips_dir();
        assert!(dir.join("manifest.json").is_file());
        assert!(dir.join("meta.json").is_file());
        let service = AudioCoachService::with_user_root(PathBuf::from("no-user-packs"));
        let pack = service.pack_store().bundled().unwrap();
        assert!(pack.read_only());
        assert_eq!(pack.status().missing, Vec::<String>::new());
    }

    #[test]
    fn resolves_workspace_clips_without_override() {
        let service = AudioCoachService::new();
        assert_eq!(bundled_pack_dir(&service), workspace_clips_dir());
    }

    #[test]
    fn override_is_tried_first() {
        let custom = PathBuf::from("custom-clips");
        let candidates = clip_dir_candidates(Some(custom.clone()));
        assert_eq!(candidates[0], custom);
        assert_eq!(candidates[1], workspace_clips_dir());
    }

    #[test]
    fn missing_override_falls_through() {
        let service = AudioCoachService::new();
        service.set_clips_dir(PathBuf::from("does-not-exist"));
        assert_eq!(bundled_pack_dir(&service), workspace_clips_dir());
    }
}
