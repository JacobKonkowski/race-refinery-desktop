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
    // IbtReader::session_yaml uses strict UTF-8; iRacing session info is often Latin-1.
    if let Ok(Some(yaml)) = reader.session_yaml() {
        if let Ok(session) = SessionInfo::parse(&yaml) {
            return Ok(session);
        }
    }

    let data = std::fs::read(path)
        .with_context(|| format!("re-read IBT for session YAML: {}", path.display()))?;
    let header = reader.header();
    if header.session_info_len <= 0 {
        anyhow::bail!("IBT contains no session info block");
    }

    let mut parser = SessionInfoParser::new();
    parser
        .parse_from_memory(
            &data,
            header.session_info_offset,
            header.session_info_len,
            header.session_info_update.max(0) as u32,
        )
        .context("parse session info (Latin-1 tolerant)")
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
