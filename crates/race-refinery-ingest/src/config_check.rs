use std::path::PathBuf;
use std::time::Duration;

use race_refinery_storage::IracingConfigCheck;

pub fn default_telemetry_dir() -> PathBuf {
    dirs::document_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("iRacing")
        .join("telemetry")
}

pub fn default_app_ini_path() -> PathBuf {
    dirs::document_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("iRacing")
        .join("app.ini")
}

pub fn check_iracing_config() -> IracingConfigCheck {
    let app_ini_path = default_app_ini_path().to_string_lossy().to_string();
    let telemetry_dir = default_telemetry_dir().to_string_lossy().to_string();
    let mut warnings = Vec::new();

    let (mem_enabled, disk_enabled) = if std::path::Path::new(&app_ini_path).exists() {
        match std::fs::read_to_string(&app_ini_path) {
            Ok(content) => {
                let mem = ini_flag(&content, "irsdkEnableMem");
                let disk = ini_flag(&content, "irsdkEnableDisk");
                if !mem {
                    warnings.push("Set irsdkEnableMem=1 in Documents\\iRacing\\app.ini for live telemetry (v2).".into());
                }
                if !disk {
                    warnings.push(
                        "Set irsdkEnableDisk=1 in Documents\\iRacing\\app.ini to record IBT files."
                            .into(),
                    );
                }
                (mem, disk)
            }
            Err(e) => {
                warnings.push(format!("Could not read app.ini: {e}"));
                (false, false)
            }
        }
    } else {
        warnings.push(
            "Documents\\iRacing\\app.ini not found. Install iRacing or create the file.".into(),
        );
        (false, false)
    };

    if !std::path::Path::new(&telemetry_dir).exists() {
        warnings.push(format!(
            "Telemetry folder not found at {telemetry_dir}. Record telemetry in iRacing with Alt+L."
        ));
    }

    IracingConfigCheck {
        app_ini_path,
        telemetry_dir,
        mem_enabled,
        disk_enabled,
        warnings,
    }
}

fn ini_flag(content: &str, key: &str) -> bool {
    content.lines().any(|line| {
        let trimmed = line.trim();
        if let Some((k, v)) = trimmed.split_once('=') {
            k.trim().eq_ignore_ascii_case(key) && v.trim() == "1"
        } else {
            false
        }
    })
}

pub fn debounce_duration() -> Duration {
    Duration::from_secs(2)
}
