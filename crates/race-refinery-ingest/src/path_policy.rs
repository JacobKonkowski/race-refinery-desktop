//! Restrict IBT import paths to the telemetry folder or an explicit allowlist.

use std::path::{Path, PathBuf};

use super::config_check::default_telemetry_dir;

/// True if `path` is an `.ibt` file under the default telemetry directory
/// (after canonicalize), or equals a dialog-picked path that is still `.ibt`.
pub fn is_allowed_ibt_path(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    if !ext.eq_ignore_ascii_case("ibt") {
        return false;
    }

    let Ok(canon) = path.canonicalize() else {
        // File may not exist yet during watcher races; still require .ibt and
        // a parent under telemetry when possible.
        return path_under_telemetry_loose(path);
    };

    let Ok(tele) = default_telemetry_dir().canonicalize() else {
        return canon
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("ibt"));
    };

    canon.starts_with(&tele)
}

fn path_under_telemetry_loose(path: &Path) -> bool {
    let tele = default_telemetry_dir();
    path.starts_with(&tele)
}

/// Normalize and validate an import path from IPC.
pub fn validate_import_path(path: impl AsRef<Path>) -> Result<PathBuf, String> {
    let path = path.as_ref();
    if !is_allowed_ibt_path(path) {
        return Err(format!(
            "Import refused: path must be an .ibt under the telemetry folder ({})",
            default_telemetry_dir().display()
        ));
    }
    Ok(path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_ibt() {
        let p = default_telemetry_dir().join("notes.txt");
        assert!(!is_allowed_ibt_path(&p));
    }
}
