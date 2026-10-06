//! Voice Studio recorder: microphone capture (cpal) on a worker thread, and saving
//! a take into a pack (clean-up chain + one level of undo).

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use parking_lot::Mutex;
use serde::Serialize;

use super::dsp;
use super::pack::{VoicePack, UNDO_DIR};
use super::phrases::phrase;

/// Longest take kept; anything past this is dropped (a stuck key, not a phrase).
const MAX_TAKE_SECS: usize = 30;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InputDevice {
    pub name: String,
    pub is_default: bool,
}

/// Microphones the system reports, default first.
pub fn input_devices() -> Vec<InputDevice> {
    let host = cpal::default_host();
    let default = host.default_input_device().and_then(|d| d.name().ok());
    let mut out: Vec<InputDevice> = host
        .input_devices()
        .map(|devices| {
            devices
                .filter_map(|d| d.name().ok())
                .map(|name| InputDevice {
                    is_default: default.as_deref() == Some(name.as_str()),
                    name,
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort_by_key(|d| !d.is_default);
    out
}

/// Raw mono capture at the device rate.
pub struct Captured {
    pub rate: u32,
    pub samples: Vec<f32>,
}

struct Capture {
    stop: mpsc::Sender<()>,
    done: mpsc::Receiver<Captured>,
}

/// One capture at a time; the cpal stream lives on its own thread because it is
/// not `Send`.
#[derive(Default)]
pub struct Recorder {
    capture: Mutex<Option<Capture>>,
    /// Peak since the last [`Recorder::take_level`], as `f32` bits.
    level: Arc<AtomicU32>,
}

impl Recorder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_capturing(&self) -> bool {
        self.capture.lock().is_some()
    }

    /// Start capturing from `device` (empty = system default), replacing any
    /// capture already running.
    pub fn start(&self, device: &str) -> anyhow::Result<()> {
        self.cancel();
        let (ready_tx, ready_rx) = mpsc::channel::<anyhow::Result<()>>();
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let (done_tx, done_rx) = mpsc::channel::<Captured>();
        let level = Arc::clone(&self.level);
        level.store(0, Ordering::Relaxed);
        let device = device.to_string();
        thread::spawn(move || {
            let buffer = Arc::new(Mutex::new(Vec::<f32>::new()));
            match open_stream(&device, Arc::clone(&buffer), level) {
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                }
                Ok((stream, rate)) => {
                    let _ = ready_tx.send(Ok(()));
                    let _ = stop_rx.recv();
                    drop(stream);
                    let samples = std::mem::take(&mut *buffer.lock());
                    let _ = done_tx.send(Captured { rate, samples });
                }
            }
        });
        ready_rx
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| anyhow::anyhow!("the microphone did not start"))??;
        *self.capture.lock() = Some(Capture {
            stop: stop_tx,
            done: done_rx,
        });
        Ok(())
    }

    /// Peak input level (0-1) since the previous call.
    pub fn take_level(&self) -> f32 {
        f32::from_bits(self.level.swap(0, Ordering::Relaxed))
    }

    /// Stop and return the audio captured so far.
    pub fn stop(&self) -> anyhow::Result<Captured> {
        let capture = self
            .capture
            .lock()
            .take()
            .ok_or_else(|| anyhow::anyhow!("not recording"))?;
        let _ = capture.stop.send(());
        capture
            .done
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| anyhow::anyhow!("the microphone did not stop cleanly"))
    }

    /// Stop and discard.
    pub fn cancel(&self) {
        if let Some(capture) = self.capture.lock().take() {
            let _ = capture.stop.send(());
        }
    }
}

fn find_device(name: &str) -> anyhow::Result<cpal::Device> {
    let host = cpal::default_host();
    if name.is_empty() {
        return host
            .default_input_device()
            .ok_or_else(|| anyhow::anyhow!("no microphone found"));
    }
    host.input_devices()?
        .find(|d| d.name().is_ok_and(|n| n == name))
        .ok_or_else(|| anyhow::anyhow!("microphone '{name}' is not connected"))
}

fn open_stream(
    device_name: &str,
    buffer: Arc<Mutex<Vec<f32>>>,
    level: Arc<AtomicU32>,
) -> anyhow::Result<(cpal::Stream, u32)> {
    let device = find_device(device_name)?;
    let supported = device.default_input_config()?;
    let format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    let rate = config.sample_rate.0;
    let channels = usize::from(config.channels.max(1));
    let max_len = rate as usize * MAX_TAKE_SECS;

    let push = move |mono: &mut dyn Iterator<Item = f32>| {
        let mut buf = buffer.lock();
        let mut pk = 0.0_f32;
        for s in mono {
            pk = pk.max(s.abs());
            if buf.len() < max_len {
                buf.push(s);
            }
        }
        let _ = level.try_update(Ordering::Relaxed, Ordering::Relaxed, |cur| {
            Some(f32::from_bits(cur).max(pk).to_bits())
        });
    };
    let on_error = |e: cpal::StreamError| tracing::warn!("microphone stream error: {e}");

    macro_rules! build {
        ($t:ty, $to_f32:expr) => {{
            let push = push.clone();
            device.build_input_stream(
                &config,
                move |data: &[$t], _: &cpal::InputCallbackInfo| {
                    let mut frames = data.chunks(channels).map(|frame| {
                        frame.iter().map(|s| $to_f32(*s)).sum::<f32>() / frame.len() as f32
                    });
                    push(&mut frames);
                },
                on_error,
                None,
            )?
        }};
    }

    let stream = match format {
        cpal::SampleFormat::F32 => build!(f32, |s: f32| s),
        cpal::SampleFormat::I16 => build!(i16, |s: i16| f32::from(s) / 32_768.0),
        cpal::SampleFormat::U16 => build!(u16, |s: u16| (f32::from(s) - 32_768.0) / 32_768.0),
        cpal::SampleFormat::I32 => build!(i32, |s: i32| s as f32 / 2_147_483_648.0),
        other => anyhow::bail!("unsupported microphone sample format {other:?}"),
    };
    stream.play()?;
    Ok((stream, rate))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TakeResult {
    pub key: String,
    pub duration_ms: u32,
    /// Raw input peak (0-1) before normalization.
    pub input_peak: f32,
    pub warnings: Vec<String>,
}

/// Clean up a capture and save it as `key` in `pack`, keeping the previous clip
/// for [`undo_take`].
pub fn save_take(
    pack: &mut VoicePack,
    key: &str,
    captured: &Captured,
) -> anyhow::Result<TakeResult> {
    let dest = pack.clip_file_for(key)?;
    let input_peak = dsp::peak(&captured.samples);
    let cleaned = dsp::clean_take(&captured.samples, captured.rate);
    if cleaned.is_empty() {
        anyhow::bail!("no speech detected; check the microphone and its level");
    }
    stash_previous(pack, key)?;
    dsp::write_pack_wav(&dest, &cleaned)?;
    pack.register_clip(key)?;
    let duration_ms = (cleaned.len() as u64 * 1000 / u64::from(dsp::PACK_RATE)) as u32;
    Ok(TakeResult {
        key: key.to_string(),
        duration_ms,
        input_peak,
        warnings: take_warnings(key, duration_ms, input_peak),
    })
}

fn take_warnings(key: &str, duration_ms: u32, input_peak: f32) -> Vec<String> {
    let mut out = Vec::new();
    if input_peak >= 0.99 {
        out.push("The input clipped. Lower the mic gain or move back a little.".into());
    } else if input_peak < 0.05 {
        out.push("Very quiet input. Raise the mic gain or move closer.".into());
    }
    let words = phrase(key)
        .map(|p| p.prompt.split_whitespace().count())
        .unwrap_or(1) as u32;
    let expected_ms = 250 + words * 380;
    if duration_ms > expected_ms * 5 / 2 + 500 {
        out.push("Longer than expected for this phrase. Check for extra words or noise.".into());
    } else if duration_ms < 150 {
        out.push("Very short take. Was the whole phrase captured?".into());
    }
    out
}

fn undo_paths(pack: &VoicePack, key: &str) -> (PathBuf, PathBuf) {
    let dir = pack.dir.join(UNDO_DIR);
    (
        dir.join(format!("{key}.wav")),
        dir.join(format!("{key}.none")),
    )
}

/// Keep the clip being replaced (or a marker that there was none).
fn stash_previous(pack: &VoicePack, key: &str) -> anyhow::Result<()> {
    let (wav, none) = undo_paths(pack, key);
    if let Some(parent) = wav.parent() {
        fs::create_dir_all(parent)?;
    }
    let _ = fs::remove_file(&wav);
    let _ = fs::remove_file(&none);
    match pack.clip_path(key) {
        Some(current) => {
            fs::copy(current, &wav)?;
        }
        None => fs::write(&none, b"")?,
    }
    Ok(())
}

pub fn has_undo(pack: &VoicePack, key: &str) -> bool {
    let (wav, none) = undo_paths(pack, key);
    wav.is_file() || none.is_file()
}

/// Restore the clip `key` had before the last take. `false` when there is nothing
/// to undo.
pub fn undo_take(pack: &mut VoicePack, key: &str) -> anyhow::Result<bool> {
    let dest = pack.clip_file_for(key)?;
    let (wav, none) = undo_paths(pack, key);
    if wav.is_file() {
        fs::rename(&wav, &dest)?;
        pack.register_clip(key)?;
        Ok(true)
    } else if none.is_file() {
        fs::remove_file(&none)?;
        pack.remove_clip(key)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::super::pack::test_support::temp_dir;
    use super::super::pack::PackStore;
    use super::*;

    fn speech(rate: u32) -> Captured {
        let mut samples = vec![0.0; rate as usize / 4];
        samples.extend(
            (0..rate as usize / 2)
                .map(|i| (i as f32 * 200.0 * std::f32::consts::TAU / rate as f32).sin() * 0.3),
        );
        samples.extend(vec![0.0; rate as usize / 4]);
        Captured { rate, samples }
    }

    fn user_pack(name: &str) -> VoicePack {
        let root = temp_dir(name);
        let store = PackStore::new(root.join("bundled"), root.join("user"));
        store.create("Takes").unwrap()
    }

    #[test]
    fn save_then_undo_restores_previous_state() {
        let mut pack = user_pack("takes");
        assert!(!has_undo(&pack, "flag_green"));

        let first = save_take(&mut pack, "flag_green", &speech(48_000)).unwrap();
        assert!(first.duration_ms > 400 && first.duration_ms < 800);
        assert!(pack.has("flag_green"));
        assert!(has_undo(&pack, "flag_green"));
        let first_bytes = fs::read(pack.clip_path("flag_green").unwrap()).unwrap();

        let mut quieter = speech(44_100);
        quieter.samples.iter_mut().for_each(|s| *s *= 0.5);
        save_take(&mut pack, "flag_green", &quieter).unwrap();
        assert_ne!(
            fs::read(pack.clip_path("flag_green").unwrap()).unwrap(),
            first_bytes
        );

        assert!(undo_take(&mut pack, "flag_green").unwrap());
        assert_eq!(
            fs::read(pack.clip_path("flag_green").unwrap()).unwrap(),
            first_bytes
        );
        assert!(!undo_take(&mut pack, "flag_green").unwrap());
    }

    #[test]
    fn undoing_the_first_take_removes_the_clip() {
        let mut pack = user_pack("first-take");
        save_take(&mut pack, "n7", &speech(48_000)).unwrap();
        assert!(undo_take(&mut pack, "n7").unwrap());
        assert!(!pack.has("n7"));
        assert!(!pack.clips.contains_key("n7"));
    }

    #[test]
    fn silent_takes_are_rejected() {
        let mut pack = user_pack("silent");
        let silent = Captured {
            rate: 48_000,
            samples: vec![0.0; 48_000],
        };
        assert!(save_take(&mut pack, "n7", &silent).is_err());
        assert!(!pack.has("n7"));
    }

    #[test]
    fn warnings_flag_clipping_and_long_takes() {
        assert!(take_warnings("n7", 600, 1.0)[0].contains("clipped"));
        assert!(take_warnings("n7", 600, 0.01)[0].contains("quiet"));
        assert!(take_warnings("n7", 5_000, 0.5)[0].contains("Longer"));
        assert!(take_warnings("n7", 600, 0.5).is_empty());
    }
}
