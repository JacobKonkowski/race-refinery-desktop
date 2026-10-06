//! Voice Studio preview: soundboard presets and a composer that build callouts with
//! the same phrasing helpers as the live coach, plus the Spotter QC playlist.

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use super::pack::VoicePack;
use super::phrases::{phrases, Tier, RADIO_BEEP_KEY};
use super::phrasing::{
    push_delta, push_fuel_status, push_gap_seconds, push_incidents, push_lap_time_callout,
    push_position, push_sector_time_callout, push_uint,
};
use super::player::{AudioPlayer, Playback};
use super::speech::{SpeechPlan, SpeechUnit};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetInfo {
    pub id: &'static str,
    pub label: &'static str,
}

const PRESETS: &[(&str, &str)] = &[
    ("lap", "Lap time + delta"),
    ("sector", "Sector time"),
    ("gap", "Gaps ahead / behind"),
    ("position", "Position change"),
    ("fuel", "Fuel status"),
    ("incident", "Incident + limit"),
    ("numbers", "Two-clip numbers (29, 45)"),
    ("flag", "Flag with radio beep"),
];

pub fn presets() -> Vec<PresetInfo> {
    PRESETS
        .iter()
        .map(|(id, label)| PresetInfo { id, label })
        .collect()
}

fn clip(units: &mut Vec<SpeechUnit>, key: &str) {
    units.push(SpeechUnit::Clip(key.into()));
}

pub fn preset_plan(id: &str) -> Option<SpeechPlan> {
    let mut u = Vec::new();
    match id {
        "lap" => {
            clip(&mut u, "lap");
            push_lap_time_callout(&mut u, 12, 89_452.0);
            push_delta(&mut u, -300.0);
        }
        "sector" => {
            clip(&mut u, "sector");
            push_sector_time_callout(&mut u, 2, 31_420.0);
            push_delta(&mut u, 1_240.0);
        }
        "gap" => {
            clip(&mut u, "gap_ahead");
            push_gap_seconds(&mut u, 1.4);
            clip(&mut u, "gap_behind");
            push_gap_seconds(&mut u, 3.0);
        }
        "position" => {
            clip(&mut u, "position_up");
            push_uint(&mut u, 7);
        }
        "fuel" => {
            clip(&mut u, "fuel_low");
            push_fuel_status(&mut u, 12.0, Some(5.6));
        }
        "incident" => push_incidents(&mut u, 15, Some(17)),
        "numbers" => {
            push_uint(&mut u, 29);
            u.push(SpeechUnit::Pause(300));
            push_uint(&mut u, 45);
        }
        "flag" => {
            clip(&mut u, RADIO_BEEP_KEY);
            clip(&mut u, "flag_yellow");
        }
        _ => return None,
    }
    Some(SpeechPlan::sequence(u))
}

/// Free-form soundboard input; every set field adds its callout, in this order.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Composition {
    pub radio_beep: bool,
    pub lap: Option<u32>,
    pub lap_time_ms: Option<f64>,
    pub sector: Option<u32>,
    pub sector_time_ms: Option<f64>,
    pub delta_ms: Option<f64>,
    pub position: Option<u32>,
    pub gap_ahead_s: Option<f32>,
    pub gap_behind_s: Option<f32>,
    pub fuel_liters: Option<f32>,
    pub fuel_laps: Option<f32>,
    pub incidents: Option<u32>,
    pub incident_limit: Option<u32>,
    pub number: Option<u32>,
}

pub fn compose(c: &Composition) -> SpeechPlan {
    let mut u = Vec::new();
    if c.radio_beep {
        clip(&mut u, RADIO_BEEP_KEY);
    }
    if let (Some(lap), Some(ms)) = (c.lap, c.lap_time_ms) {
        clip(&mut u, "lap");
        push_lap_time_callout(&mut u, lap as i32, ms);
    }
    if let (Some(sector), Some(ms)) = (c.sector, c.sector_time_ms) {
        clip(&mut u, "sector");
        push_sector_time_callout(&mut u, sector as i32, ms);
    }
    if let Some(delta) = c.delta_ms {
        push_delta(&mut u, delta);
    }
    if let Some(pos) = c.position {
        push_position(&mut u, pos as i32);
    }
    if let Some(gap) = c.gap_ahead_s {
        clip(&mut u, "gap_ahead");
        push_gap_seconds(&mut u, gap);
    }
    if let Some(gap) = c.gap_behind_s {
        clip(&mut u, "gap_behind");
        push_gap_seconds(&mut u, gap);
    }
    if let Some(liters) = c.fuel_liters {
        push_fuel_status(&mut u, liters, c.fuel_laps);
    }
    if let Some(count) = c.incidents {
        push_incidents(&mut u, count, c.incident_limit);
    }
    if let Some(n) = c.number {
        push_uint(&mut u, n);
    }
    SpeechPlan::sequence(u)
}

/// Spotter-tier keys in registry order: the QC pass before driving.
pub fn spotter_keys() -> Vec<&'static str> {
    phrases()
        .iter()
        .filter(|p| p.tier == Tier::Spotter)
        .map(|p| p.key)
        .collect()
}

/// Keys in `plan` the pack cannot play (the radio beep always can).
pub fn missing_keys(pack: &VoicePack, plan: &SpeechPlan) -> Vec<String> {
    let mut seen = BTreeSet::new();
    plan.clip_keys()
        .into_iter()
        .filter(|k| k != RADIO_BEEP_KEY && !pack.has(k) && seen.insert(k.clone()))
        .collect()
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewStatus {
    pub playing: bool,
    pub paused: bool,
    /// Zero-based position in the current list.
    pub index: usize,
    pub total: usize,
    pub text: String,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PreviewControl {
    Pause,
    Resume,
    Skip,
    Stop,
}

/// Plays soundboard callouts and playlists on a worker thread. Starting a new
/// preview stops the previous one.
#[derive(Default)]
pub struct PreviewPlayer {
    status: Arc<Mutex<PreviewStatus>>,
    generation: Arc<AtomicU64>,
    paused: Arc<AtomicBool>,
    skip: Arc<AtomicBool>,
}

impl PreviewPlayer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn status(&self) -> PreviewStatus {
        self.status.lock().clone()
    }

    /// Play `items` in order. `speak_lock` is shared with the live coach so the
    /// two never talk over each other.
    pub fn play(
        &self,
        pack: VoicePack,
        items: Vec<SpeechPlan>,
        volume: f32,
        speak_lock: Arc<Mutex<()>>,
    ) {
        let gen = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.paused.store(false, Ordering::SeqCst);
        self.skip.store(false, Ordering::SeqCst);
        *self.status.lock() = PreviewStatus {
            playing: true,
            total: items.len(),
            ..PreviewStatus::default()
        };
        let status = Arc::clone(&self.status);
        let generation = Arc::clone(&self.generation);
        let paused = Arc::clone(&self.paused);
        let skip = Arc::clone(&self.skip);
        thread::spawn(move || {
            let current = || generation.load(Ordering::SeqCst) == gen;
            match AudioPlayer::new(pack, volume) {
                Ok(player) => {
                    let mut index = 0;
                    while index < items.len() {
                        // Wait out a pause without holding the speak lock, so the
                        // live coach can still talk while a preview is paused.
                        while paused.load(Ordering::SeqCst) && current() {
                            status.lock().paused = true;
                            thread::sleep(Duration::from_millis(50));
                        }
                        if !current() {
                            return;
                        }
                        let plan = &items[index];
                        {
                            let mut s = status.lock();
                            s.index = index;
                            s.paused = false;
                            s.text = plan.display_text();
                        }
                        let mut interrupted = false;
                        let result = {
                            let _speaking = speak_lock.lock();
                            let mut control = || {
                                if !current() || skip.swap(false, Ordering::SeqCst) {
                                    return Playback::Stop;
                                }
                                if paused.load(Ordering::SeqCst) {
                                    interrupted = true;
                                    return Playback::Stop;
                                }
                                Playback::Play
                            };
                            player.play_plan_with(plan, &mut control)
                        };
                        if let Err(e) = result {
                            tracing::warn!("preview playback failed: {e:#}");
                            break;
                        }
                        // A paused line replays from its start on resume.
                        if !interrupted {
                            index += 1;
                        }
                    }
                }
                Err(e) => tracing::warn!("preview playback unavailable: {e:#}"),
            }
            if current() {
                let mut s = status.lock();
                s.playing = false;
                s.paused = false;
            }
        });
    }

    pub fn control(&self, action: PreviewControl) {
        match action {
            PreviewControl::Pause => self.paused.store(true, Ordering::SeqCst),
            PreviewControl::Resume => self.paused.store(false, Ordering::SeqCst),
            PreviewControl::Skip => self.skip.store(true, Ordering::SeqCst),
            PreviewControl::Stop => {
                self.generation.fetch_add(1, Ordering::SeqCst);
                let mut s = self.status.lock();
                s.playing = false;
                s.paused = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_preset_builds_a_plan_from_registry_keys() {
        for preset in presets() {
            let plan = preset_plan(preset.id).unwrap_or_else(|| panic!("{}", preset.id));
            for key in plan.clip_keys() {
                assert!(
                    crate::phrases::is_pack_key(&key),
                    "{}: {key} is not a pack key",
                    preset.id
                );
            }
        }
        assert!(preset_plan("nope").is_none());
    }

    #[test]
    fn number_preset_uses_two_clip_numbers() {
        let keys = preset_plan("numbers").unwrap().clip_keys();
        assert_eq!(keys, ["n20", "n9", "n40", "n5"]);
    }

    #[test]
    fn composer_chains_fields_in_order() {
        let plan = compose(&Composition {
            position: Some(3),
            incidents: Some(15),
            incident_limit: Some(17),
            ..Composition::default()
        });
        assert_eq!(
            plan.clip_keys(),
            ["position", "n3", "incident_intro", "n15", "limit", "n17"]
        );
        assert!(compose(&Composition::default()).clip_keys().is_empty());
    }

    #[test]
    fn spotter_playlist_is_spotter_tier_only() {
        let keys = spotter_keys();
        assert_eq!(keys.first(), Some(&"intro_online"));
        assert!(!keys.contains(&"n5"));
        assert!(keys.contains(&"flag_green"));
    }
}
