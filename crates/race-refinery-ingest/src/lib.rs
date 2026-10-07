//! IBT ingest: file discovery, parsing, and import orchestration.

pub mod config_check;
pub mod frame_extractor;
pub mod handles;
pub mod ibt_importer;
pub mod import_runner;
pub mod path_policy;
pub mod post_session;
pub mod traffic_sampler;
pub mod watcher;

pub use config_check::{check_iracing_config, default_telemetry_dir};
pub use frame_extractor::FastFrameExtractor;
pub use handles::ImportHandles;
pub use ibt_importer::{
    file_identity_hash, hash_file, parse_ibt_file, parse_ibt_file_with_progress, save_parsed_ibt,
    scan_ibt_files, ImportResult, ProgressCallback,
};
pub use import_runner::{finish_folder_status, run_import, run_reimport};
pub use path_policy::{is_allowed_ibt_path, validate_import_path};
pub use post_session::spawn_recent_ibt_import;
pub use watcher::{import_folder, start_watcher};
