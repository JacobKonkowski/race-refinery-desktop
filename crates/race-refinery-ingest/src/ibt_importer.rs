//! IBT file -> [`AnalyzedSession`] parsing and persistence.
//!
//! This is the only place that touches the raw file. It resolves session metadata
//! (track, car, sectors, sub-session labels) from the YAML, streams frames through
//! [`FastFrameExtractor`], and hands them to the pure analysis pipeline.

use anyhow::{Context, Result};
use pitwall::ibt::IbtReader;
use pitwall::schema::SessionInfoParser;
use pitwall::SessionInfo;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Instant;
use tracing::info;

use race_refinery_analysis::{
    analyze_session, events_for_lap, AnalyzedSession, SectorBoundary, SessionMeta,
};

use super::frame_extractor::FastFrameExtractor;
use super::traffic_sampler::TrafficSampler;

pub struct ImportResult {
    pub session_id: i64,
    pub lap_count: usize,
    pub elapsed_ms: u128,
    pub skipped: bool,
}

pub type ProgressCallback = Box<dyn Fn(f64, String) + Send>;

pub fn import_ibt_file(
    db: &race_refinery_storage::Database,
    path: &Path,
    session: AnalyzedSession,
    hash: &str,
    elapsed_ms: u128,
) -> Result<ImportResult> {
    let path_str = path.to_string_lossy().to_string();
    let lap_count = session.laps.len();
    let store_start = Instant::now();
    let session_id = db.insert_session(&path_str, hash, &session)?;
    info!(
        "Stored session {} ({} laps) in {} ms",
        session_id,
        lap_count,
        store_start.elapsed().as_millis()
    );
    Ok(ImportResult {
        session_id,
        lap_count,
        elapsed_ms,
        skipped: false,
    })
}

pub async fn parse_ibt_file(path: &Path) -> Result<(AnalyzedSession, String, u128)> {
    parse_ibt_file_with_progress(path, None).await
}

pub async fn parse_ibt_file_with_progress(
    path: &Path,
    progress: Option<ProgressCallback>,
) -> Result<(AnalyzedSession, String, u128)> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || parse_ibt_file_fast(&path, progress))
        .await
        .context("parse task join")?
}

pub fn parse_ibt_file_fast(
    path: &Path,
    progress: Option<ProgressCallback>,
) -> Result<(AnalyzedSession, String, u128)> {
    let started = Instant::now();
    let hash = file_identity_hash(path)?;

    info!("Importing IBT: {}", path.display());
    report_progress(&progress, 2.0, "Opening IBT file...");

    let open_start = Instant::now();
    let mut reader = IbtReader::open(path).context("open IBT with IbtReader")?;
    let total_frames = reader.total_frames();
    let tick_rate = reader.tick_rate();
    info!(
        "Opened IBT in {} ms — {} frames at {:.0} Hz (~{:.1} min session)",
        open_start.elapsed().as_millis(),
        total_frames,
        tick_rate,
        total_frames as f64 / tick_rate / 60.0
    );

    report_progress(&progress, 5.0, format!("Reading {total_frames} frames..."));

    let session_info = parse_session_info(&reader, path)?;
    let meta = SessionMeta {
        track: session_info.weekend_info.track_display_name.clone(),
        car: extract_car_name(&session_info),
        session_date: extract_session_date(path),
        sector_boundaries: extract_sectors(&session_info),
        session_labels: build_session_labels(&session_info),
    };

    let extractor = FastFrameExtractor::from_schema(reader.variables())?;
    let mut traffic = TrafficSampler::from_schema(reader.variables());

    let read_start = Instant::now();
    let mut frames = Vec::with_capacity(total_frames);
    let progress_interval = (total_frames / 20).max(1000);

    while let Some((frame_data, _, _)) = reader.read_next_frame()? {
        let idx = frames.len();
        if idx > 0 && idx % progress_interval == 0 {
            let pct = 5.0 + (idx as f64 / total_frames as f64) * 60.0;
            report_progress(
                &progress,
                pct,
                format!("Reading frames... {idx}/{total_frames}"),
            );
        }
        traffic.observe(&frame_data);
        frames.push(extractor.extract(&frame_data));
    }
    info!(
        "Read {} telemetry frames in {} ms",
        frames.len(),
        read_start.elapsed().as_millis()
    );

    report_progress(
        &progress,
        70.0,
        format!("Analyzing {} frames...", frames.len()),
    );

    let analyze_start = Instant::now();
    let mut analyzed = analyze_session(frames, &meta);
    let traffic_samples = traffic.into_samples();
    let traffic_count = traffic_samples.len();
    for lap in &mut analyzed.laps {
        lap.traffic_events = events_for_lap(&traffic_samples, lap.session_num, lap.iracing_lap);
    }
    info!(
        "Analyzed {} laps across {} iRacing sub-sessions in {} ms ({} traffic samples)",
        analyzed.laps.len(),
        analyzed
            .laps
            .iter()
            .map(|l| l.session_num)
            .collect::<HashSet<_>>()
            .len(),
        analyze_start.elapsed().as_millis(),
        traffic_count
    );

    report_progress(&progress, 88.0, "Parse complete, preparing save...");

    let elapsed_ms = started.elapsed().as_millis();
    info!(
        "IBT import parse total: {} ms for {}",
        elapsed_ms,
        path.display()
    );

    Ok((analyzed, hash, elapsed_ms))
}

fn parse_session_info(reader: &IbtReader, path: &Path) -> Result<SessionInfo> {
    // iRacing writes this block as Windows-1252. Pitwall's readers decode it with
    // `from_utf8_lossy`, which turns ó and é into U+FFFD.
    let data = std::fs::read(path)
        .with_context(|| format!("re-read IBT for session YAML: {}", path.display()))?;
    let header = reader.header();
    if header.session_info_offset < 0 || header.session_info_len <= 0 {
        anyhow::bail!("IBT contains no session info block");
    }
    let start = header.session_info_offset as usize;
    let end = start.saturating_add(header.session_info_len as usize);
    if end > data.len() {
        anyhow::bail!("session info block is outside the IBT");
    }
    let yaml = decode_iracing_text(&data[start..end]);
    if yaml.trim().is_empty() {
        anyhow::bail!("IBT session info block is empty");
    }
    SessionInfoParser::new()
        .parse(&yaml)
        .context("parse session info")
}

/// UTF-8 when the bytes are valid UTF-8; otherwise Windows-1252.
fn decode_iracing_text(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    let bytes = &bytes[..end];
    if let Ok(text) = std::str::from_utf8(bytes) {
        return text.to_string();
    }
    bytes.iter().copied().map(windows_1252_char).collect()
}

fn windows_1252_char(byte: u8) -> char {
    match byte {
        0x80 => '\u{20AC}',
        0x82 => '\u{201A}',
        0x83 => '\u{0192}',
        0x84 => '\u{201E}',
        0x85 => '\u{2026}',
        0x86 => '\u{2020}',
        0x87 => '\u{2021}',
        0x88 => '\u{02C6}',
        0x89 => '\u{2030}',
        0x8A => '\u{0160}',
        0x8B => '\u{2039}',
        0x8C => '\u{0152}',
        0x8E => '\u{017D}',
        0x91 => '\u{2018}',
        0x92 => '\u{2019}',
        0x93 => '\u{201C}',
        0x94 => '\u{201D}',
        0x95 => '\u{2022}',
        0x96 => '\u{2013}',
        0x97 => '\u{2014}',
        0x98 => '\u{02DC}',
        0x99 => '\u{2122}',
        0x9A => '\u{0161}',
        0x9B => '\u{203A}',
        0x9C => '\u{0153}',
        0x9E => '\u{017E}',
        0x9F => '\u{0178}',
        other => other as char,
    }
}

fn report_progress(progress: &Option<ProgressCallback>, pct: f64, message: impl Into<String>) {
    if let Some(cb) = progress {
        cb(pct, message.into());
    }
}

pub fn save_parsed_ibt(
    db: &race_refinery_storage::Database,
    path: &Path,
    session: AnalyzedSession,
    hash: &str,
    elapsed_ms: u128,
) -> Result<ImportResult> {
    let path_str = path.to_string_lossy().to_string();
    cache_track_map(&session);
    if let Some(session_id) = db
        .find_session_id_by_hash(hash)?
        .or(db.find_session_id_by_path(&path_str)?)
    {
        info!(
            "Skipping already-imported IBT: {} (session {session_id})",
            path.display()
        );
        return Ok(ImportResult {
            session_id,
            lap_count: 0,
            elapsed_ms: 0,
            skipped: true,
        });
    }
    import_ibt_file(db, path, session, hash, elapsed_ms)
}

/// Persist the session's generated outline for the track, keeping whichever
/// outline came from the better source lap. Failures are logged, never fatal:
/// a missing map only costs the track-map widget.
fn cache_track_map(session: &AnalyzedSession) {
    let Some(outline) = &session.track_map else {
        return;
    };
    match race_refinery_storage::save_track_map(outline) {
        Ok(true) => info!(
            "Cached track map for {} ({} points, {:.0}% lap coverage)",
            outline.track,
            outline.points.len(),
            outline.coverage * 100.0
        ),
        Ok(false) => {}
        Err(e) => info!("Track map not cached for {}: {e}", outline.track),
    }
}

/// Fast dedup key from path + size + mtime (not a content SHA-256 of the IBT bytes).
/// Avoids reading the entire IBT twice (`IbtReader` already loads it).
pub fn file_identity_hash(path: &Path) -> Result<String> {
    let meta = std::fs::metadata(path).with_context(|| format!("stat {}", path.display()))?;
    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let digest = Sha256::digest(format!(
        "{}:{}:{}",
        path.to_string_lossy(),
        meta.len(),
        modified
    ));
    Ok(format!("{:x}", digest))
}

pub fn hash_file(path: &Path) -> Result<String> {
    file_identity_hash(path)
}

fn build_session_labels(session: &SessionInfo) -> HashMap<i32, String> {
    session
        .session_info
        .sessions
        .iter()
        .map(|s| {
            let label = s
                .session_name
                .as_ref()
                .filter(|name| !name.is_empty())
                .cloned()
                .unwrap_or_else(|| s.session_type.clone());
            (s.session_num, label)
        })
        .collect()
}

fn extract_car_name(session: &SessionInfo) -> String {
    if let Some(driver_info) = &session.driver_info {
        let car_idx = driver_info.driver_car_idx.unwrap_or(-1);
        if let Some(drivers) = &driver_info.drivers {
            for driver in drivers {
                if driver.car_idx == car_idx {
                    return driver
                        .car_screen_name
                        .clone()
                        .or_else(|| driver.car_path.clone())
                        .unwrap_or_else(|| "Unknown Car".into());
                }
            }
        }
    }
    "Unknown Car".into()
}

/// Sector split lines from the session YAML. Empty when the file provides none —
/// we do not invent equal-thirds boundaries.
fn extract_sectors(session: &SessionInfo) -> Vec<SectorBoundary> {
    let Some(split) = &session.split_time_info else {
        return Vec::new();
    };
    let Some(sectors) = &split.sectors else {
        return Vec::new();
    };

    sectors
        .iter()
        .filter_map(|s| {
            Some(SectorBoundary {
                sector_num: s.sector_num?,
                start_pct: s.sector_start_pct?,
            })
        })
        .collect()
}

fn extract_session_date(path: &Path) -> String {
    if let Ok(meta) = std::fs::metadata(path) {
        if let Ok(modified) = meta.modified() {
            if let Ok(datetime) = modified.duration_since(std::time::UNIX_EPOCH) {
                let secs = datetime.as_secs() as i64;
                if let Some(dt) = chrono::DateTime::from_timestamp(secs, 0) {
                    return dt.to_rfc3339();
                }
            }
        }
    }
    chrono::Utc::now().to_rfc3339()
}

pub fn scan_ibt_files(dir: &Path) -> Result<Vec<PathBuf>> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("ibt"))
            == Some(true)
        {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::decode_iracing_text;

    const INTERLAGOS: &str = "Autódromo José Carlos Pace";

    #[test]
    fn windows_1252_track_name_keeps_accents() {
        // ó is 0xF3 and é is 0xE9 in the sim's session block.
        let mut bytes = Vec::new();
        for ch in INTERLAGOS.chars() {
            match ch {
                'ó' => bytes.push(0xF3),
                'é' => bytes.push(0xE9),
                other => bytes.push(u8::try_from(u32::from(other)).unwrap()),
            }
        }
        bytes.push(0);
        assert_eq!(decode_iracing_text(&bytes), INTERLAGOS);
    }

    #[test]
    fn utf8_track_name_is_left_unchanged() {
        let mut bytes = INTERLAGOS.as_bytes().to_vec();
        bytes.push(0);
        assert_eq!(decode_iracing_text(&bytes), INTERLAGOS);
    }
}
