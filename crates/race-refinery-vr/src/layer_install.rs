//! Register / unregister the Race Refinery OpenXR API layer with the loader.
//!
//! OpenXR discovers implicit API layers from per-user registry values under
//! `HKCU\Software\Khronos\OpenXR\1\ApiLayers\Implicit`: the value name is the
//! full path to the layer's JSON manifest and the `REG_DWORD` data is `0` when
//! enabled (non-zero disables it). This is exactly how RaceLab VR and
//! OpenKneeboard register their layers.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[cfg(windows)]
const IMPLICIT_LAYERS_KEY: &str = r"Software\Khronos\OpenXR\1\ApiLayers\Implicit";

/// File name of the manifest, used to recognize our value among others.
pub const MANIFEST_FILE: &str = "race_refinery_openxr_layer.json";

/// Manifest name used before the Race Refinery rename. Install and uninstall
/// remove these registrations so the old layer never loads alongside ours.
#[cfg(windows)]
const LEGACY_MANIFEST_FILE: &str = "pitwall_openxr_layer.json";

pub const LAYER_DLL: &str = "race-refinery-openxr-layer.dll";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VrLayerDiagnostics {
    pub registered: bool,
    pub manifest_path: Option<String>,
    pub dll_present: bool,
    pub dll_path: Option<String>,
    /// True when `RACE_REFINERY_VR_DISABLE` is set in the process or user environment.
    pub layer_disabled: bool,
    /// Registered, DLL beside manifest, and not disabled via env.
    pub ready: bool,
    /// iRacing OpenXR renderer VRMode (0 = desktop/monitor, non-zero = VR).
    pub iracing_open_xr_vr_mode: Option<u32>,
    /// Whether OpenXREnabled=1 in rendererDX11OpenXR.ini.
    pub iracing_open_xr_enabled: Option<bool>,
    pub issues: Vec<String>,
}

pub fn dll_path_for_manifest(manifest_path: &str) -> PathBuf {
    Path::new(manifest_path)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(LAYER_DLL)
}

pub fn iracing_documents_dir() -> Option<PathBuf> {
    let profile = std::env::var("USERPROFILE").ok()?;
    for sub in ["OneDrive\\Documents\\iRacing", "Documents\\iRacing"] {
        let dir = PathBuf::from(&profile).join(sub);
        if dir.is_dir() {
            return Some(dir);
        }
    }
    None
}

pub fn read_ini_value(path: &Path, key: &str) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    for line in content.lines() {
        let line = line.split(';').next()?.trim();
        if line.is_empty() || line.starts_with('[') {
            continue;
        }
        let (name, value) = line.split_once('=')?;
        if name.trim().eq_ignore_ascii_case(key) {
            return Some(value.trim().to_string());
        }
    }
    None
}

fn append_iracing_vr_issues(issues: &mut Vec<String>) -> (Option<u32>, Option<bool>) {
    let Some(dir) = iracing_documents_dir() else {
        issues.push("Could not find iRacing Documents folder to verify OpenXR VR settings.".into());
        return (None, None);
    };

    let open_xr_ini = dir.join("rendererDX11OpenXR.ini");
    let mut open_xr_vr_mode = None;
    let mut open_xr_enabled = None;
    if open_xr_ini.is_file() {
        open_xr_vr_mode = read_ini_value(&open_xr_ini, "VRMode").and_then(|v| v.parse().ok());
        open_xr_enabled = read_ini_value(&open_xr_ini, "OpenXREnabled").map(|v| v == "1");
        if open_xr_enabled == Some(false) {
            issues.push(
                "OpenXREnabled=0 in rendererDX11OpenXR.ini. Enable OpenXR in iRacing graphics \
                 settings."
                    .into(),
            );
        }
        if open_xr_vr_mode == Some(0) {
            issues.push(
                "iRacing rendererDX11OpenXR.ini last saved VRMode=0 (often stale while the sim \
                 is running). If you are already in OpenXR VR, ignore this — confirm telemetry \
                 publishing and that the layer is installed instead."
                    .into(),
            );
        }
    } else {
        issues.push(format!(
            "Missing {}. Launch iRacing once so graphics settings are created.",
            open_xr_ini.display()
        ));
    }

    let openvr_ini = dir.join("rendererDX11OpenVR.ini");
    if read_ini_value(&openvr_ini, "OpenVREnabled") == Some("1".into()) {
        issues.push(
            "OpenVR is enabled. When iRacing prompts for a VR runtime, choose OpenXR — the \
             Race Refinery layer does not load under OpenVR/SteamVR."
                .into(),
        );
    }

    (open_xr_vr_mode, open_xr_enabled)
}

#[cfg(windows)]
pub use windows_impl::{install_layer, is_layer_installed, layer_diagnostics, uninstall_layer};

#[cfg(windows)]
mod windows_impl {
    use super::*;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::ERROR_SUCCESS;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegEnumValueW, RegSetValueExW, HKEY,
        HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_DWORD, REG_OPTION_NON_VOLATILE,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn open_key(access: windows::Win32::System::Registry::REG_SAM_FLAGS) -> Option<HKEY> {
        let key = wide(IMPLICIT_LAYERS_KEY);
        let mut hkey = HKEY::default();
        let status = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(key.as_ptr()),
                0,
                PCWSTR::null(),
                REG_OPTION_NON_VOLATILE,
                access,
                None,
                &mut hkey,
                None,
            )
        };
        (status == ERROR_SUCCESS).then_some(hkey)
    }

    fn env_var_set(name: &str) -> bool {
        std::env::var(name)
            .map(|v| !v.is_empty() && v != "0")
            .unwrap_or(false)
    }

    fn staged_layer_dir() -> PathBuf {
        std::env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("race-refinery")
            .join("openxr-layer")
    }

    /// Registry value paths should be plain absolute paths (RaceLab uses `C:\...`, not `\\?\`).
    fn registry_manifest_path(path: &Path) -> String {
        let abs = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let text = abs.to_string_lossy();
        text.strip_prefix(r"\\?\")
            .unwrap_or(&text)
            .replace('/', "\\")
    }

    fn stage_layer_from_bundle(bundle_manifest_path: &str) -> anyhow::Result<PathBuf> {
        let bundle_manifest = Path::new(bundle_manifest_path);
        let bundle_dll = dll_path_for_manifest(bundle_manifest_path);
        let stage_dir = staged_layer_dir();
        std::fs::create_dir_all(&stage_dir)?;
        let staged_manifest = stage_dir.join(MANIFEST_FILE);
        let staged_dll = stage_dir.join(LAYER_DLL);
        std::fs::copy(bundle_manifest, &staged_manifest)?;
        std::fs::copy(&bundle_dll, &staged_dll)?;
        Ok(staged_manifest)
    }

    fn collect_race_refinery_registrations() -> Vec<String> {
        let Some(hkey) = open_key(KEY_READ) else {
            return Vec::new();
        };
        let mut paths = Vec::new();
        let mut index = 0u32;
        loop {
            let mut name = [0u16; 1024];
            let mut name_len = name.len() as u32;
            let mut data = [0u8; 8];
            let mut data_len = data.len() as u32;
            let status = unsafe {
                RegEnumValueW(
                    hkey,
                    index,
                    windows::core::PWSTR(name.as_mut_ptr()),
                    &mut name_len,
                    None,
                    None,
                    Some(data.as_mut_ptr()),
                    Some(&mut data_len as *mut u32),
                )
            };
            if status != ERROR_SUCCESS {
                break;
            }
            let value_name = String::from_utf16_lossy(&name[..name_len as usize]);
            let lower = value_name.to_lowercase();
            if lower.ends_with(MANIFEST_FILE) || lower.ends_with(LEGACY_MANIFEST_FILE) {
                paths.push(value_name);
            }
            index += 1;
        }
        unsafe {
            let _ = RegCloseKey(hkey);
        }
        paths
    }

    fn delete_registrations(hkey: HKEY, paths: &[String]) {
        for path in paths {
            let value = wide(path);
            unsafe {
                let _ = RegDeleteValueW(hkey, PCWSTR(value.as_ptr()));
            }
        }
    }

    fn register_manifest(reg_path: &str) -> anyhow::Result<()> {
        let Some(hkey) = open_key(KEY_WRITE) else {
            anyhow::bail!("Could not open OpenXR implicit layers registry key");
        };
        let value = wide(reg_path);
        let data = 0u32.to_ne_bytes();
        let status = unsafe {
            RegSetValueExW(
                hkey,
                PCWSTR(value.as_ptr()),
                0,
                REG_DWORD,
                Some(data.as_slice()),
            )
        };
        unsafe {
            let _ = RegCloseKey(hkey);
        }
        if status != ERROR_SUCCESS {
            anyhow::bail!("Failed to write OpenXR layer registry value");
        }
        Ok(())
    }

    /// Full path to our registered manifest, if any.
    pub fn find_registered_manifest_path() -> Option<String> {
        let hkey = open_key(KEY_READ)?;
        let mut index = 0u32;
        let mut found = None;
        loop {
            let mut name = [0u16; 1024];
            let mut name_len = name.len() as u32;
            let mut data = [0u8; 8];
            let mut data_len = data.len() as u32;
            let status = unsafe {
                RegEnumValueW(
                    hkey,
                    index,
                    windows::core::PWSTR(name.as_mut_ptr()),
                    &mut name_len,
                    None,
                    None,
                    Some(data.as_mut_ptr()),
                    Some(&mut data_len as *mut u32),
                )
            };
            if status != ERROR_SUCCESS {
                break;
            }
            let value_name = String::from_utf16_lossy(&name[..name_len as usize]);
            if value_name.to_lowercase().ends_with(MANIFEST_FILE) {
                let enabled =
                    data_len >= 4 && u32::from_ne_bytes([data[0], data[1], data[2], data[3]]) == 0;
                if enabled {
                    found = Some(value_name);
                    break;
                }
            }
            index += 1;
        }
        unsafe {
            let _ = RegCloseKey(hkey);
        }
        found
    }

    /// True if the Race Refinery layer manifest is registered and enabled (data == 0).
    pub fn is_layer_installed() -> bool {
        find_registered_manifest_path().is_some()
    }

    pub fn layer_diagnostics(bundle_manifest_path: &str) -> VrLayerDiagnostics {
        let mut issues = Vec::new();
        let registered_path = find_registered_manifest_path();
        let registered = registered_path.is_some();

        let manifest_path = if Path::new(bundle_manifest_path).exists() {
            Some(bundle_manifest_path.to_string())
        } else {
            issues.push(format!(
                "Bundled manifest not found at {bundle_manifest_path}. Rebuild the app."
            ));
            registered_path.clone()
        };

        let check_manifest = registered_path
            .as_deref()
            .or(manifest_path.as_deref())
            .unwrap_or(bundle_manifest_path);
        let dll_path = dll_path_for_manifest(check_manifest);
        let dll_present = dll_path.is_file();
        let dll_path_str = dll_path.to_string_lossy().into_owned();

        if !dll_present {
            issues.push(format!(
                "Layer DLL missing at {dll_path_str}. Build openxr-layer and copy {LAYER_DLL} \
                 beside the manifest (see docs/NATIVE_VR.md)."
            ));
        }

        if !registered {
            issues
                .push("Layer not registered. Click Install VR layer, then restart iRacing.".into());
        } else if let Some(reg) = &registered_path {
            let reg_lower = reg.to_lowercase();
            if reg_lower.contains("\\target\\") || reg_lower.contains("\\openxr-layer\\build\\") {
                issues.push(
                    "Layer registry points at a Race Refinery build folder. Click Reinstall VR layer \
                     to copy the DLL into AppData (RaceLab installs under Program Files)."
                        .into(),
                );
            } else if manifest_path.as_ref() != Some(reg) {
                issues.push(format!(
                    "Registry points to {reg}. Reinstall if you moved or rebuilt Race Refinery."
                ));
            }
        }

        let layer_disabled = env_var_set("RACE_REFINERY_VR_DISABLE");
        if layer_disabled {
            issues.push("RACE_REFINERY_VR_DISABLE is set — unset it and restart iRacing.".into());
        }

        if let Ok(text) = std::fs::read_to_string(check_manifest) {
            if text.contains("enable_environment") {
                issues.push(
                    "Manifest still requires RACE_REFINERY_VR_ENABLE. Rebuild Race Refinery and reinstall \
                     the VR layer."
                        .into(),
                );
            }
        }

        let (iracing_open_xr_vr_mode, iracing_open_xr_enabled) =
            append_iracing_vr_issues(&mut issues);

        let ready = registered
            && dll_present
            && !layer_disabled
            && !issues.iter().any(|i| i.contains("RACE_REFINERY_VR_ENABLE"));

        VrLayerDiagnostics {
            registered,
            manifest_path: registered_path.or(manifest_path),
            dll_present,
            dll_path: Some(dll_path_str),
            layer_disabled,
            ready,
            iracing_open_xr_vr_mode,
            iracing_open_xr_enabled,
            issues,
        }
    }

    /// Copy the bundled layer into AppData and register that stable path with OpenXR.
    pub fn install_layer(manifest_path: &str) -> anyhow::Result<()> {
        let manifest = Path::new(manifest_path);
        if !manifest.is_file() {
            anyhow::bail!(
                "Layer manifest not found at {}. Rebuild Race Refinery so resources are bundled.",
                manifest.display()
            );
        }
        let dll = dll_path_for_manifest(manifest_path);
        if !dll.is_file() {
            anyhow::bail!(
                "Layer DLL not found at {}. Build openxr-layer and copy {} next to the manifest.",
                dll.display(),
                LAYER_DLL
            );
        }

        let staged_manifest = stage_layer_from_bundle(manifest_path)?;
        let reg_path = registry_manifest_path(&staged_manifest);
        let old_regs = collect_race_refinery_registrations();

        let Some(hkey) = open_key(KEY_WRITE) else {
            anyhow::bail!("Could not open OpenXR implicit layers registry key");
        };
        delete_registrations(hkey, &old_regs);
        unsafe {
            let _ = RegCloseKey(hkey);
        }

        register_manifest(&reg_path)?;
        Ok(())
    }

    /// Remove every Race Refinery layer registration (best effort).
    pub fn uninstall_layer(_manifest_path: &str) -> anyhow::Result<()> {
        let old_regs = collect_race_refinery_registrations();
        if old_regs.is_empty() {
            return Ok(());
        }
        let Some(hkey) = open_key(KEY_WRITE) else {
            return Ok(());
        };
        delete_registrations(hkey, &old_regs);
        unsafe {
            let _ = RegCloseKey(hkey);
        }
        Ok(())
    }
}

#[cfg(not(windows))]
pub fn find_registered_manifest_path() -> Option<String> {
    None
}

#[cfg(not(windows))]
pub fn is_layer_installed() -> bool {
    false
}

#[cfg(not(windows))]
pub fn layer_diagnostics(_bundle_manifest_path: &str) -> VrLayerDiagnostics {
    VrLayerDiagnostics {
        issues: vec!["OpenXR layer install is only supported on Windows".into()],
        ..Default::default()
    }
}

#[cfg(not(windows))]
pub fn install_layer(_manifest_path: &str) -> anyhow::Result<()> {
    anyhow::bail!("OpenXR layer install is only supported on Windows")
}

#[cfg(not(windows))]
pub fn uninstall_layer(_manifest_path: &str) -> anyhow::Result<()> {
    Ok(())
}
