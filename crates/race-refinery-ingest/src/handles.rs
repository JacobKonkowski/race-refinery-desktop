//! Shared handles for IBT import without depending on Tauri `AppState`.

use std::sync::Arc;

use parking_lot::Mutex;

use race_refinery_storage::{Database, ImportStatus};

/// Import-facing subset of app state (database + progress). Composed in desktop.
#[derive(Clone)]
pub struct ImportHandles {
    pub db: Arc<Mutex<Database>>,
    pub import_status: Arc<Mutex<ImportStatus>>,
    pub import_gate: Arc<tokio::sync::Mutex<()>>,
}
