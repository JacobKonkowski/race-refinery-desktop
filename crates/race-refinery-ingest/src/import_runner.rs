use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tauri::{AppHandle, Emitter};
use tracing::info;

use crate::ibt_importer::ImportResult;
use crate::{
    file_identity_hash, parse_ibt_file_with_progress, save_parsed_ibt, ImportHandles,
    ProgressCallback,
};
use race_refinery_storage::ImportStatus;

/// Run a single IBT import. Only one import holds the gate at a time so DB writes
/// and progress updates stay predictable (watcher + manual import cannot overlap).
pub async fn run_import(
    app: &AppHandle,
    import: &ImportHandles,
    path: PathBuf,
) -> Result<ImportResult> {
    if let Some(result) = try_skip_import(import, &path)? {
        finish_status(app, import, &result);
        return Ok(result);
    }

    let _gate = import.import_gate.lock().await;

    if let Some(result) = try_skip_import(import, &path)? {
        finish_status(app, import, &result);
        return Ok(result);
    }

    let (analyzed, hash, elapsed_ms) = parse_with_status(app, import, &path).await?;
    let import_save = import.clone();
    let path_save = path.clone();
    let result = tokio::task::spawn_blocking(move || {
        let db = import_save.db.lock();
        save_parsed_ibt(&db, &path_save, analyzed, &hash, elapsed_ms)
    })
    .await
    .context("save task join")??;

    finish_status(app, import, &result);
    // Always emit so the UI can select the session (including skip-of-existing).
    let _ = app.emit("import-complete", result.session_id);
    Ok(result)
}

/// Re-parse an already-imported IBT with the current analysis and replace the
/// stored session. The old rows are deleted only after the file parses, so a
/// missing or unreadable file leaves the session intact.
pub async fn run_reimport(
    app: &AppHandle,
    import: &ImportHandles,
    session_id: i64,
    path: PathBuf,
) -> Result<ImportResult> {
    let _gate = import.import_gate.lock().await;
    let (analyzed, hash, elapsed_ms) = parse_with_status(app, import, &path).await?;
    let import_save = import.clone();
    let path_save = path.clone();
    let result = tokio::task::spawn_blocking(move || {
        let db = import_save.db.lock();
        db.delete_session(session_id)?;
        save_parsed_ibt(&db, &path_save, analyzed, &hash, elapsed_ms)
    })
    .await
    .context("save task join")??;

    finish_status(app, import, &result);
    let _ = app.emit("import-complete", result.session_id);
    Ok(result)
}

async fn parse_with_status(
    app: &AppHandle,
    import: &ImportHandles,
    path: &Path,
) -> Result<(race_refinery_analysis::AnalyzedSession, String, u128)> {
    let path_label = path.to_string_lossy().to_string();
    set_status(
        app,
        import,
        Some(path_label.clone()),
        true,
        1.0,
        "Opening IBT file...",
    );

    let import_progress = import.clone();
    let app_progress = app.clone();
    let progress_path = path_label.clone();
    let progress = Some(Box::new(move |pct: f64, message: String| {
        {
            let mut status = import_progress.import_status.lock();
            status.active = true;
            status.current_file = Some(progress_path.clone());
            status.progress_pct = pct;
            status.message = message;
        }
        let _ = app_progress.emit(
            "import-status",
            import_progress.import_status.lock().clone(),
        );
    }) as ProgressCallback);

    let (analyzed, hash, elapsed_ms) = parse_ibt_file_with_progress(path, progress)
        .await
        .context("parse IBT")?;

    let trace_points: usize = analyzed.laps.iter().map(|l| l.traces.len()).sum();
    let lap_count = analyzed.laps.len();
    info!(
        "Parsed {} — {} laps, {} trace points; saving to database",
        path.display(),
        lap_count,
        trace_points
    );
    set_status(
        app,
        import,
        Some(path_label.clone()),
        true,
        92.0,
        format!("Saving {lap_count} laps to database..."),
    );
    Ok((analyzed, hash, elapsed_ms))
}

fn try_skip_import(import: &ImportHandles, path: &Path) -> Result<Option<ImportResult>> {
    let hash = file_identity_hash(path)?;
    let path_str = path.to_string_lossy().to_string();
    let db = import.db.lock();
    let existing_id = db
        .find_session_id_by_hash(&hash)?
        .or(db.find_session_id_by_path(&path_str)?);
    if let Some(session_id) = existing_id {
        info!(
            "Skipping already-imported IBT: {} (session {session_id})",
            path.display()
        );
        return Ok(Some(ImportResult {
            session_id,
            lap_count: 0,
            elapsed_ms: 0,
            skipped: true,
        }));
    }
    Ok(None)
}

fn set_status(
    app: &AppHandle,
    import: &ImportHandles,
    current_file: Option<String>,
    active: bool,
    pct: f64,
    message: impl Into<String>,
) {
    {
        let mut status = import.import_status.lock();
        status.active = active;
        status.current_file = current_file;
        status.progress_pct = pct;
        status.message = message.into();
    }
    let _ = app.emit("import-status", import.import_status.lock().clone());
}

/// Set the final status after a folder scan, summing laps/time across every
/// file that was actually imported (skipped/already-imported files don't
/// count towards the totals).
pub fn finish_folder_status(
    app: &AppHandle,
    import: &ImportHandles,
    imported_files: usize,
    total_files: usize,
    total_laps: usize,
    total_elapsed_ms: u128,
) {
    let message = if total_files == 0 {
        "No IBT files found".into()
    } else if imported_files == 0 {
        format!("Scanned {total_files} file(s), already imported")
    } else {
        format!(
            "Imported {total_laps} laps across {imported_files}/{total_files} file(s) in {total_elapsed_ms} ms"
        )
    };
    {
        let mut status = import.import_status.lock();
        *status = ImportStatus {
            active: false,
            current_file: None,
            progress_pct: 100.0,
            message,
        };
    }
    let _ = app.emit("import-status", import.import_status.lock().clone());
}

fn finish_status(app: &AppHandle, import: &ImportHandles, result: &ImportResult) {
    let message = if result.skipped {
        "Already imported".into()
    } else {
        format!(
            "Imported {} laps in {} ms",
            result.lap_count, result.elapsed_ms
        )
    };
    {
        let mut status = import.import_status.lock();
        *status = ImportStatus {
            active: false,
            current_file: None,
            progress_pct: 100.0,
            message,
        };
    }
    let _ = app.emit("import-status", import.import_status.lock().clone());
}
