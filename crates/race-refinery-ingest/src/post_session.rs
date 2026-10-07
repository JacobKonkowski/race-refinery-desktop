//! Post-session IBT auto-import (composed into LiveService from desktop).

use std::time::Duration;

use tauri::AppHandle;
use tracing::warn;

use super::{default_telemetry_dir, run_import, scan_ibt_files, ImportHandles};

const POST_SESSION_IBT_MAX_AGE_SECS: u64 = 600;

/// Scan the telemetry folder for recent IBTs and import them.
pub fn spawn_recent_ibt_import(app: AppHandle, import: ImportHandles) {
    tauri::async_runtime::spawn(async move {
        let dir = default_telemetry_dir();
        let files = match scan_ibt_files(&dir) {
            Ok(files) => files,
            Err(e) => {
                warn!("Post-session IBT scan failed: {e:#}");
                return;
            }
        };

        let cutoff = std::time::SystemTime::now()
            .checked_sub(Duration::from_secs(POST_SESSION_IBT_MAX_AGE_SECS));
        for path in files {
            let recent = std::fs::metadata(&path)
                .and_then(|m| m.modified())
                .ok()
                .zip(cutoff)
                .map(|(modified, cutoff)| modified >= cutoff)
                .unwrap_or(false);
            if !recent {
                continue;
            }
            if let Err(e) = run_import(&app, &import, path.clone()).await {
                warn!("Post-session import of {} failed: {e:#}", path.display());
            }
        }
    });
}
