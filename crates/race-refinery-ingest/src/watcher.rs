use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use parking_lot::Mutex;
use tauri::AppHandle;

use crate::{
    default_telemetry_dir, finish_folder_status, run_import, scan_ibt_files, ImportHandles,
};

pub fn start_watcher(app: AppHandle, import: ImportHandles) {
    let telemetry_dir = default_telemetry_dir();
    if !telemetry_dir.exists() {
        let _ = std::fs::create_dir_all(&telemetry_dir);
    }

    let app_clone = app.clone();
    let import_clone = import.clone();
    let watch_path = telemetry_dir.clone();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("watcher runtime");

        let pending: Arc<Mutex<HashSet<PathBuf>>> = Arc::new(Mutex::new(HashSet::new()));
        let pending_timer = pending.clone();
        let app_timer = app_clone.clone();
        let import_timer = import_clone.clone();

        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(3));
            let batch: Vec<PathBuf> = {
                let mut guard = pending_timer.lock();
                if guard.is_empty() {
                    continue;
                }
                guard.drain().collect()
            };
            for path in batch {
                let _ = rt.block_on(run_import(&app_timer, &import_timer, path));
            }
        });

        let mut watcher = RecommendedWatcher::new(
            move |res: notify::Result<notify::Event>| {
                if let Ok(event) = res {
                    // Only new files — Modify fires when OneDrive syncs or files are read,
                    // which would queue dozens of parallel imports and block the DB.
                    if matches!(event.kind, EventKind::Create(_)) {
                        for path in event.paths {
                            if path
                                .extension()
                                .and_then(|e| e.to_str())
                                .map(|e| e.eq_ignore_ascii_case("ibt"))
                                == Some(true)
                            {
                                pending.lock().insert(path);
                            }
                        }
                    }
                }
            },
            notify::Config::default(),
        )
        .expect("create file watcher");

        if watcher
            .watch(&watch_path, RecursiveMode::NonRecursive)
            .is_err()
        {
            return;
        }

        loop {
            std::thread::sleep(Duration::from_secs(3600));
        }
    });
}

pub async fn import_folder(
    app: &AppHandle,
    import: &ImportHandles,
    dir: PathBuf,
) -> anyhow::Result<usize> {
    let files = scan_ibt_files(&dir)?;
    let count = files.len();

    let mut imported_files = 0usize;
    let mut total_laps = 0usize;
    let mut total_elapsed_ms = 0u128;
    for path in files {
        let result = run_import(app, import, path).await?;
        if !result.skipped {
            imported_files += 1;
            total_laps += result.lap_count;
            total_elapsed_ms += result.elapsed_ms;
        }
    }

    finish_folder_status(
        app,
        import,
        imported_files,
        count,
        total_laps,
        total_elapsed_ms,
    );
    Ok(count)
}
