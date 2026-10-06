//! Small, dependency-free signal helpers for voice pack clips: format conversion
//! and the take clean-up chain (high-pass -> trim -> gate -> normalize).

use std::io::{Read, Seek};
use std::path::Path;

use anyhow::Context;

/// Pack clip format: mono 16-bit PCM at this rate.
pub const PACK_RATE: u32 = 22_050;

/// Analysis window for trim / gate / loudness.
const FRAME_MS: usize = 10;
/// Speech loudness target (RMS of voiced frames), about -18 dBFS.
const TARGET_RMS: f32 = 0.126;
/// Peak ceiling after normalization, about -1 dBFS.
const PEAK_CEILING: f32 = 0.89;
/// Silence kept before the first and after the last voiced frame.
const LEAD_MS: usize = 40;
const TAIL_MS: usize = 90;

pub fn downmix(interleaved: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return interleaved.to_vec();
    }
    interleaved
        .chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}

/// Linear-interpolation resampler; plenty for speech going down to 22.05 kHz.
pub fn resample(samples: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || samples.is_empty() || from == 0 {
        return samples.to_vec();
    }
    let ratio = f64::from(from) / f64::from(to);
    let len = (samples.len() as u64 * u64::from(to) / u64::from(from)) as usize;
    (0..len)
        .map(|i| {
            let pos = i as f64 * ratio;
            let idx = pos.floor() as usize;
            let frac = (pos - idx as f64) as f32;
            let a = samples[idx.min(samples.len() - 1)];
            let b = samples[(idx + 1).min(samples.len() - 1)];
            a + (b - a) * frac
        })
        .collect()
}

/// One-pole high-pass to drop rumble and DC offset below `cutoff_hz`.
pub fn high_pass(samples: &mut [f32], rate: u32, cutoff_hz: f32) {
    let rc = 1.0 / (cutoff_hz * std::f32::consts::TAU);
    let dt = 1.0 / rate as f32;
    let alpha = rc / (rc + dt);
    let (mut prev_in, mut prev_out) = (0.0_f32, 0.0_f32);
    for s in samples.iter_mut() {
        let out = alpha * (prev_out + *s - prev_in);
        prev_in = *s;
        prev_out = out;
        *s = out;
    }
}

pub fn peak(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0_f32, |m, s| m.max(s.abs()))
}

fn frame_len(rate: u32) -> usize {
    (rate as usize * FRAME_MS / 1000).max(1)
}

fn frame_rms(samples: &[f32], rate: u32) -> Vec<f32> {
    samples
        .chunks(frame_len(rate))
        .map(|f| (f.iter().map(|s| s * s).sum::<f32>() / f.len() as f32).sqrt())
        .collect()
}

/// RMS level splitting speech from background: well above the quietest frames
/// (capped so a take with no pauses still counts as speech) and not far below
/// the loudest.
fn voice_threshold(rms: &[f32]) -> f32 {
    let mut sorted = rms.to_vec();
    sorted.sort_by(f32::total_cmp);
    let floor = sorted.get(sorted.len() / 10).copied().unwrap_or(0.0);
    let loudest = sorted.last().copied().unwrap_or(0.0);
    (floor * 3.0)
        .min(loudest * 0.5)
        .max(loudest * 0.06)
        .max(0.002)
}

/// Drop leading / trailing silence, keeping a short lead-in and tail.
pub fn trim_silence(samples: &[f32], rate: u32) -> Vec<f32> {
    let rms = frame_rms(samples, rate);
    let threshold = voice_threshold(&rms);
    let (Some(first), Some(last)) = (
        rms.iter().position(|r| *r > threshold),
        rms.iter().rposition(|r| *r > threshold),
    ) else {
        return Vec::new();
    };
    let frame = frame_len(rate);
    let start = (first * frame).saturating_sub(rate as usize * LEAD_MS / 1000);
    let end = ((last + 1) * frame + rate as usize * TAIL_MS / 1000).min(samples.len());
    samples[start..end].to_vec()
}

/// Duck background noise between words by ~20 dB with smooth gain changes.
pub fn noise_gate(samples: &mut [f32], rate: u32) {
    let rms = frame_rms(samples, rate);
    if rms.is_empty() {
        return;
    }
    let threshold = voice_threshold(&rms) * 0.7;
    let frame = frame_len(rate);
    let attack = 1.0 - (-1.0 / (rate as f32 * 0.002)).exp();
    let release = 1.0 - (-1.0 / (rate as f32 * 0.060)).exp();
    let mut gain = if rms[0] > threshold { 1.0 } else { 0.1 };
    for (i, s) in samples.iter_mut().enumerate() {
        let target = if rms[(i / frame).min(rms.len() - 1)] > threshold {
            1.0
        } else {
            0.1
        };
        let k = if target > gain { attack } else { release };
        gain += (target - gain) * k;
        *s *= gain;
    }
}

/// Bring voiced frames to the pack loudness target without exceeding the peak ceiling.
pub fn normalize(samples: &mut [f32], rate: u32) {
    let rms = frame_rms(samples, rate);
    let threshold = voice_threshold(&rms);
    let voiced: Vec<f32> = rms.into_iter().filter(|r| *r > threshold).collect();
    let pk = peak(samples);
    if voiced.is_empty() || pk <= 0.0 {
        return;
    }
    let loudness = (voiced.iter().map(|r| r * r).sum::<f32>() / voiced.len() as f32).sqrt();
    let gain = (TARGET_RMS / loudness).min(PEAK_CEILING / pk);
    for s in samples.iter_mut() {
        *s *= gain;
    }
}

/// Short fades so trimmed edges never click.
fn fade_edges(samples: &mut [f32], rate: u32) {
    let n = (rate as usize * 5 / 1000).min(samples.len() / 2);
    let len = samples.len();
    for i in 0..n {
        let g = i as f32 / n as f32;
        samples[i] *= g;
        samples[len - 1 - i] *= g;
    }
}

/// Recorder clean-up: resample to the pack rate, then high-pass, trim, gate,
/// normalize. Empty when no speech stands out from the background.
pub fn clean_take(samples: &[f32], rate: u32) -> Vec<f32> {
    let mut out = resample(samples, rate, PACK_RATE);
    high_pass(&mut out, PACK_RATE, 80.0);
    let mut out = trim_silence(&out, PACK_RATE);
    if out.is_empty() {
        return out;
    }
    noise_gate(&mut out, PACK_RATE);
    normalize(&mut out, PACK_RATE);
    fade_edges(&mut out, PACK_RATE);
    out
}

/// Decode any PCM / float WAV to mono samples at the pack rate.
pub fn decode_wav<R: Read + Seek>(reader: R) -> anyhow::Result<Vec<f32>> {
    let mut wav = hound::WavReader::new(reader)?;
    let spec = wav.spec();
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => wav.samples::<f32>().collect::<Result<_, _>>()?,
        hound::SampleFormat::Int => {
            let scale = (1_i64 << (spec.bits_per_sample.max(1) - 1)) as f32;
            wav.samples::<i32>()
                .map(|s| s.map(|v| v as f32 / scale))
                .collect::<Result<_, _>>()?
        }
    };
    let mono = downmix(&samples, usize::from(spec.channels));
    Ok(resample(&mono, spec.sample_rate, PACK_RATE))
}

pub fn read_wav_file(path: &Path) -> anyhow::Result<Vec<f32>> {
    let file = std::fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    decode_wav(std::io::BufReader::new(file)).with_context(|| format!("decode {}", path.display()))
}

/// Write mono 16-bit PCM at the pack rate.
pub fn write_pack_wav(path: &Path, samples: &[f32]) -> anyhow::Result<()> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: PACK_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec)
        .with_context(|| format!("write {}", path.display()))?;
    for s in samples {
        writer.write_sample((s.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16)?;
    }
    writer.finalize()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(rate: u32, ms: usize, amp: f32) -> Vec<f32> {
        (0..rate as usize * ms / 1000)
            .map(|i| (i as f32 * 220.0 * std::f32::consts::TAU / rate as f32).sin() * amp)
            .collect()
    }

    fn silence(rate: u32, ms: usize, noise: f32) -> Vec<f32> {
        (0..rate as usize * ms / 1000)
            .map(|i| if i % 2 == 0 { noise } else { -noise })
            .collect()
    }

    #[test]
    fn resample_scales_length() {
        let out = resample(&vec![0.0; 48_000], 48_000, PACK_RATE);
        assert_eq!(out.len(), PACK_RATE as usize);
        assert_eq!(resample(&[0.5; 10], PACK_RATE, PACK_RATE), vec![0.5; 10]);
    }

    #[test]
    fn downmix_averages_channels() {
        assert_eq!(downmix(&[1.0, 0.0, 0.5, 0.5], 2), vec![0.5, 0.5]);
    }

    #[test]
    fn trim_keeps_speech_and_short_padding() {
        let rate = PACK_RATE;
        let mut take = silence(rate, 500, 0.001);
        take.extend(tone(rate, 400, 0.3));
        take.extend(silence(rate, 700, 0.001));
        let trimmed = trim_silence(&take, rate);
        let ms = trimmed.len() * 1000 / rate as usize;
        assert!((400..=560).contains(&ms), "{ms} ms");
    }

    #[test]
    fn silence_only_trims_to_nothing() {
        assert!(clean_take(&vec![0.0; 22_050], PACK_RATE).is_empty());
    }

    #[test]
    fn gate_ducks_background_between_words() {
        let rate = PACK_RATE;
        let mut take = tone(rate, 300, 0.3);
        take.extend(silence(rate, 300, 0.01));
        take.extend(tone(rate, 300, 0.3));
        noise_gate(&mut take, rate);
        let mid = rate as usize * 450 / 1000;
        assert!(take[mid].abs() < 0.003, "{}", take[mid]);
    }

    #[test]
    fn normalize_reaches_target_without_clipping() {
        let rate = PACK_RATE;
        let mut quiet = tone(rate, 500, 0.02);
        normalize(&mut quiet, rate);
        let rms = (quiet.iter().map(|s| s * s).sum::<f32>() / quiet.len() as f32).sqrt();
        assert!((rms - TARGET_RMS).abs() < 0.01, "{rms}");
        assert!(peak(&quiet) <= PEAK_CEILING + 1e-4);
    }

    #[test]
    fn wav_round_trip_converts_to_pack_format() {
        let dir = std::env::temp_dir().join(format!("rr-dsp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("stereo.wav");
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 44_100,
            bits_per_sample: 24,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(&path, spec).unwrap();
        for _ in 0..44_100 {
            w.write_sample(4_000_000_i32).unwrap();
            w.write_sample(0_i32).unwrap();
        }
        w.finalize().unwrap();

        let mono = read_wav_file(&path).unwrap();
        assert_eq!(mono.len(), PACK_RATE as usize);
        assert!((mono[100] - 4_000_000.0 / 8_388_608.0 / 2.0).abs() < 1e-3);

        let out = dir.join("pack.wav");
        write_pack_wav(&out, &mono).unwrap();
        let spec = hound::WavReader::open(&out).unwrap().spec();
        assert_eq!((spec.channels, spec.sample_rate), (1, PACK_RATE));
        let _ = std::fs::remove_dir_all(dir);
    }
}
