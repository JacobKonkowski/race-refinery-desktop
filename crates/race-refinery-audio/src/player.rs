use std::cell::RefCell;
use std::collections::HashSet;
use std::fs::File;
use std::io::BufReader;
use std::time::Duration;

use rodio::buffer::SamplesBuffer;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};

use super::dsp::PACK_RATE;
use super::pack::VoicePack;
use super::phrases::RADIO_BEEP_KEY;
use super::speech::{SpeechPlan, SpeechUnit};

/// Overlap between consecutive clips so composed numbers blend into one phrase.
const CROSSFADE_MS: usize = 30;

/// Mono samples of consecutive clips and pauses, joined with short crossfades.
struct ClipRun {
    rate: u32,
    samples: Vec<f32>,
    /// Length of the clip at the end of `samples`; 0 after a pause.
    last_clip_len: usize,
}

impl ClipRun {
    fn new(rate: u32) -> Self {
        Self {
            rate,
            samples: Vec::new(),
            last_clip_len: 0,
        }
    }

    fn push_clip(&mut self, clip: &[f32]) {
        let overlap = (self.rate as usize * CROSSFADE_MS / 1000)
            .min(self.last_clip_len / 2)
            .min(clip.len() / 2);
        let start = self.samples.len() - overlap;
        for (i, (out, next)) in self.samples[start..].iter_mut().zip(clip).enumerate() {
            let t = (i + 1) as f32 / (overlap + 1) as f32;
            *out = *out * (1.0 - t) + next * t;
        }
        self.samples.extend_from_slice(&clip[overlap..]);
        self.last_clip_len = clip.len();
    }

    fn push_pause(&mut self, ms: u32) {
        let len = self.rate as usize * ms as usize / 1000;
        self.samples.resize(self.samples.len() + len, 0.0);
        self.last_clip_len = 0;
    }
}

/// A plan decoded against a pack: buffers ready for a sink plus the keys it lacked.
pub struct Rendered {
    /// `(sample rate, mono samples)`; a new buffer starts wherever the rate changes.
    runs: Vec<(u32, Vec<f32>)>,
    pub missing: Vec<String>,
}

impl Rendered {
    /// Nothing to play: every spoken clip was missing.
    pub fn is_silent(&self) -> bool {
        self.runs.is_empty()
    }
}

/// Decode `plan` against `pack`. Missing clips are skipped; when no spoken clip is
/// left the whole plan is dropped (a lone radio beep says nothing).
pub fn render_plan(pack: &VoicePack, plan: &SpeechPlan) -> Rendered {
    let mut runs: Vec<(u32, Vec<f32>)> = Vec::new();
    let mut run: Option<ClipRun> = None;
    let mut missing = Vec::new();
    let mut spoken = 0;
    let mut flush = |run: &mut Option<ClipRun>| {
        if let Some(r) = run.take().filter(|r| !r.samples.is_empty()) {
            runs.push((r.rate, r.samples));
        }
    };
    for unit in &plan.units() {
        match unit {
            SpeechUnit::Clip(key) => {
                let clip = if key == RADIO_BEEP_KEY && !pack.has(key) {
                    Some((PACK_RATE, radio_beep_samples()))
                } else {
                    clip_samples(pack, key)
                };
                let Some((rate, samples)) = clip else {
                    missing.push(key.clone());
                    continue;
                };
                if key != RADIO_BEEP_KEY {
                    spoken += 1;
                }
                if run.as_ref().is_some_and(|r| r.rate != rate) {
                    flush(&mut run);
                }
                run.get_or_insert_with(|| ClipRun::new(rate))
                    .push_clip(&samples);
            }
            SpeechUnit::Pause(ms) => {
                run.get_or_insert_with(|| ClipRun::new(PACK_RATE))
                    .push_pause(*ms);
            }
        }
    }
    flush(&mut run);
    if spoken == 0 {
        runs.clear();
    }
    Rendered { runs, missing }
}

/// Decode a clip to mono samples and its sample rate.
fn clip_samples(pack: &VoicePack, key: &str) -> Option<(u32, Vec<f32>)> {
    let path = pack.clip_path(key)?;
    let decoded = File::open(&path)
        .map_err(anyhow::Error::from)
        .and_then(|file| Ok(Decoder::new(BufReader::new(file))?));
    let decoder = match decoded {
        Ok(decoder) => decoder,
        Err(e) => {
            tracing::warn!("clip {key}: {e:#}");
            return None;
        }
    };
    let channels = usize::from(decoder.channels().max(1));
    let rate = decoder.sample_rate();
    let samples: Vec<f32> = decoder.convert_samples::<f32>().collect();
    let mono = if channels == 1 {
        samples
    } else {
        samples
            .chunks(channels)
            .map(|frame| frame.iter().sum::<f32>() / channels as f32)
            .collect()
    };
    Some((rate, mono))
}

/// Built-in two-tone radio chirp (1.2 kHz then 1.8 kHz, ~150 ms) with a soft
/// envelope so it does not click. A pack's `radio_beep.wav` replaces it.
pub fn radio_beep_samples() -> Vec<f32> {
    let rate = PACK_RATE as f32;
    let tone_len = (rate * 0.07) as usize;
    let fade = (rate * 0.008) as usize;
    let mut out = Vec::new();
    for (freq, gap_after) in [(1200.0_f32, true), (1800.0_f32, false)] {
        for i in 0..tone_len {
            let env = (i.min(tone_len - 1 - i).min(fade) as f32) / fade as f32;
            let t = i as f32 / rate;
            out.push((t * freq * std::f32::consts::TAU).sin() * env * 0.35);
        }
        if gap_after {
            out.resize(out.len() + PACK_RATE as usize / 100, 0.0);
        }
    }
    out
}

/// What a caller wants the sink to do while a plan plays.
pub enum Playback {
    Play,
    Stop,
}

pub struct AudioPlayer {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    pack: VoicePack,
    volume: f32,
    /// Keys already warned about, so a missing clip logs once per player.
    warned: RefCell<HashSet<String>>,
}

impl AudioPlayer {
    pub fn new(pack: VoicePack, volume: f32) -> anyhow::Result<Self> {
        let (stream, handle) =
            OutputStream::try_default().map_err(|e| anyhow::anyhow!("audio output: {e}"))?;
        Ok(Self {
            _stream: stream,
            handle,
            pack,
            volume,
            warned: RefCell::new(HashSet::new()),
        })
    }

    /// Swap packs (picks up new recordings too); cheap enough to call per callout.
    pub fn set_pack(&mut self, pack: VoicePack) {
        if pack.id != self.pack.id {
            self.warned.borrow_mut().clear();
        }
        self.pack = pack;
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume;
    }

    /// Play `plan` to the end. Returns the keys the pack was missing.
    pub fn play_plan(&self, plan: &SpeechPlan) -> anyhow::Result<Vec<String>> {
        self.play_plan_with(plan, &mut || Playback::Play)
    }

    /// Play `plan`, polling `control` so callers can stop mid-callout.
    /// Preview pause is handled outside this loop: stop the current plan, release
    /// the speak lock, then replay the same line when resume is pressed.
    pub fn play_plan_with(
        &self,
        plan: &SpeechPlan,
        control: &mut dyn FnMut() -> Playback,
    ) -> anyhow::Result<Vec<String>> {
        let rendered = render_plan(&self.pack, plan);
        for key in &rendered.missing {
            if self.warned.borrow_mut().insert(key.clone()) {
                tracing::warn!("voice pack '{}' has no clip for {key}", self.pack.id);
            }
        }
        if rendered.is_silent() {
            return Ok(rendered.missing);
        }
        let sink = Sink::try_new(&self.handle)?;
        sink.set_volume(self.volume.max(0.0));
        for (rate, samples) in rendered.runs {
            sink.append(SamplesBuffer::new(1, rate, samples));
        }
        while !sink.empty() {
            match control() {
                Playback::Play => sink.play(),
                Playback::Stop => {
                    sink.stop();
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        Ok(rendered.missing)
    }
}

#[cfg(test)]
mod tests {
    use super::super::pack::test_support::{temp_dir, write_wav};
    use super::super::pack::{PackKind, VoicePack};
    use super::super::speech::{SpeechPlan, SpeechUnit};
    use super::{radio_beep_samples, render_plan, ClipRun};

    #[test]
    fn consecutive_clips_overlap_by_the_crossfade() {
        // 1 kHz: 30 ms crossfade = 30 samples.
        let mut run = ClipRun::new(1000);
        run.push_clip(&[1.0; 100]);
        run.push_clip(&[0.0; 100]);
        assert_eq!(run.samples.len(), 170);
        // Blends from the first clip toward the second across the overlap.
        assert!(run.samples[70] < 1.0 && run.samples[70] > 0.9);
        assert!(run.samples[99] < 0.1);
        assert_eq!(run.samples[100], 0.0);
    }

    #[test]
    fn short_clips_cap_the_overlap() {
        let mut run = ClipRun::new(1000);
        run.push_clip(&[1.0; 10]);
        run.push_clip(&[1.0; 100]);
        assert_eq!(run.samples.len(), 105);
    }

    #[test]
    fn pauses_append_silence_without_overlap() {
        let mut run = ClipRun::new(1000);
        run.push_clip(&[1.0; 100]);
        run.push_pause(50);
        run.push_clip(&[1.0; 100]);
        assert_eq!(run.samples.len(), 250);
        assert!(run.samples[100..150].iter().all(|s| *s == 0.0));
        assert_eq!(run.samples[150], 1.0);
    }

    fn pack_with(name: &str, keys: &[&str]) -> VoicePack {
        let dir = temp_dir(name);
        for key in keys {
            write_wav(&dir.join(format!("{key}.wav")), 22_050, &[0.2; 2205]);
        }
        VoicePack::load("test".into(), PackKind::Folder, dir).unwrap()
    }

    fn clips(keys: &[&str]) -> SpeechPlan {
        SpeechPlan::sequence(
            keys.iter()
                .map(|k| SpeechUnit::Clip((*k).to_string()))
                .collect(),
        )
    }

    #[test]
    fn missing_units_are_skipped_and_reported() {
        let pack = pack_with("render-partial", &["incident_intro"]);
        let rendered = render_plan(&pack, &clips(&["incident_intro", "n7"]));
        assert!(!rendered.is_silent());
        assert_eq!(rendered.missing, ["n7"]);
    }

    #[test]
    fn plans_without_any_spoken_clip_are_dropped() {
        let pack = pack_with("render-empty", &[]);
        let rendered = render_plan(&pack, &clips(&["radio_beep", "flag_green"]));
        assert!(rendered.is_silent());
        assert_eq!(rendered.missing, ["flag_green"]);
    }

    #[test]
    fn built_in_beep_fills_in_for_packs_without_one() {
        let pack = pack_with("render-beep", &["flag_green"]);
        let rendered = render_plan(&pack, &clips(&["radio_beep", "flag_green"]));
        assert!(rendered.missing.is_empty());
        let total: usize = rendered.runs.iter().map(|(_, s)| s.len()).sum();
        assert!(total > radio_beep_samples().len());
    }
}
