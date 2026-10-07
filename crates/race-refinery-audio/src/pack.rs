//! Voice packs: a folder of `{key}.wav` clips plus `meta.json` and `manifest.json`.
//!
//! Three kinds share one format: the read-only bundled `default`, user packs under
//! the app data folder (addressed by id), and folders linked from anywhere on disk
//! (addressed by absolute path).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::{Deserialize, Serialize};

use super::phrases::{phrases, Tier, RADIO_BEEP_KEY};

pub const PACK_FORMAT_VERSION: u32 = 1;
/// Pack id of the bundled pack shipped with the app.
pub const BUNDLED_PACK_ID: &str = "default";
const META_FILE: &str = "meta.json";
const MANIFEST_FILE: &str = "manifest.json";
/// Per-key backups of the previous take, for one level of undo.
pub(crate) const UNDO_DIR: &str = ".undo";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PackMeta {
    pub name: String,
    pub author: String,
    pub language: String,
    pub license: String,
    pub format_version: u32,
}

impl Default for PackMeta {
    fn default() -> Self {
        Self {
            name: String::new(),
            author: String::new(),
            language: "en".into(),
            license: String::new(),
            format_version: PACK_FORMAT_VERSION,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PackKind {
    Bundled,
    User,
    Folder,
}

#[derive(Debug, Clone)]
pub struct VoicePack {
    /// Reference stored in settings: `default`, a user pack id, or a folder path.
    pub id: String,
    pub kind: PackKind,
    pub dir: PathBuf,
    pub meta: PackMeta,
    /// Clip key -> wav path relative to `dir`.
    pub clips: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TierCount {
    pub recorded: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackStatus {
    pub id: String,
    pub name: String,
    pub author: String,
    pub kind: PackKind,
    pub read_only: bool,
    pub dir: String,
    pub spotter: TierCount,
    pub engineer: TierCount,
    /// Registry keys without a playable clip, in registry order.
    pub missing: Vec<String>,
    /// The pack overrides the built-in radio beep.
    pub custom_beep: bool,
}

impl VoicePack {
    /// Read a pack folder. `meta.json` and `manifest.json` are optional: a folder of
    /// `{key}.wav` files named after registry keys is a valid pack on its own.
    pub fn load(id: String, kind: PackKind, dir: PathBuf) -> anyhow::Result<Self> {
        if !dir.is_dir() {
            anyhow::bail!("voice pack folder not found: {}", dir.display());
        }
        let mut meta: PackMeta = read_json(&dir.join(META_FILE))?.unwrap_or_default();
        if meta.name.trim().is_empty() {
            meta.name = default_name(kind, &dir);
        }
        let mut clips: BTreeMap<String, String> =
            read_json(&dir.join(MANIFEST_FILE))?.unwrap_or_default();
        let keys = phrases()
            .iter()
            .map(|p| p.key)
            .chain(std::iter::once(RADIO_BEEP_KEY));
        for key in keys {
            let file = format!("{key}.wav");
            if !clips.contains_key(key) && dir.join(&file).is_file() {
                clips.insert(key.to_string(), file);
            }
        }
        Ok(Self {
            id,
            kind,
            dir,
            meta,
            clips,
        })
    }

    pub fn read_only(&self) -> bool {
        self.kind == PackKind::Bundled
    }

    /// Path of `key`'s clip when the file exists.
    pub fn clip_path(&self, key: &str) -> Option<PathBuf> {
        self.clips
            .get(key)
            .map(|rel| self.dir.join(rel))
            .filter(|p| p.is_file())
    }

    pub fn has(&self, key: &str) -> bool {
        self.clip_path(key).is_some()
    }

    pub fn status(&self) -> PackStatus {
        let mut spotter = TierCount::default();
        let mut engineer = TierCount::default();
        let mut missing = Vec::new();
        for p in phrases() {
            let count = match p.tier {
                Tier::Spotter => &mut spotter,
                Tier::Engineer => &mut engineer,
            };
            count.total += 1;
            if self.has(p.key) {
                count.recorded += 1;
            } else {
                missing.push(p.key.to_string());
            }
        }
        PackStatus {
            id: self.id.clone(),
            name: self.meta.name.clone(),
            author: self.meta.author.clone(),
            kind: self.kind,
            read_only: self.read_only(),
            dir: self.dir.to_string_lossy().into_owned(),
            spotter,
            engineer,
            missing,
            custom_beep: self.has(RADIO_BEEP_KEY),
        }
    }

    fn ensure_writable(&self) -> anyhow::Result<()> {
        if self.read_only() {
            anyhow::bail!("the bundled pack is read-only; clone it to make changes");
        }
        Ok(())
    }

    /// Write `meta.json` and `manifest.json`.
    pub fn save(&self) -> anyhow::Result<()> {
        self.ensure_writable()?;
        fs::create_dir_all(&self.dir)?;
        write_json(&self.dir.join(META_FILE), &self.meta)?;
        write_json(&self.dir.join(MANIFEST_FILE), &self.clips)
    }

    /// Destination for `key`'s clip. Errors for read-only packs and unknown keys.
    pub fn clip_file_for(&self, key: &str) -> anyhow::Result<PathBuf> {
        self.ensure_writable()?;
        if !super::phrases::is_pack_key(key) {
            anyhow::bail!("'{key}' is not a voice pack key");
        }
        Ok(self.dir.join(format!("{key}.wav")))
    }

    /// Point `key` at `{key}.wav` (already written) and persist the manifest.
    pub fn register_clip(&mut self, key: &str) -> anyhow::Result<()> {
        self.ensure_writable()?;
        self.clips.insert(key.to_string(), format!("{key}.wav"));
        self.save()
    }

    /// Delete `key`'s clip file and manifest entry.
    pub fn remove_clip(&mut self, key: &str) -> anyhow::Result<()> {
        self.ensure_writable()?;
        if let Some(path) = self.clip_path(key) {
            fs::remove_file(path)?;
        }
        self.clips.remove(key);
        self.save()
    }
}

fn default_name(kind: PackKind, dir: &Path) -> String {
    match kind {
        PackKind::Bundled => "Race Refinery default".into(),
        _ => dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Voice pack".into()),
    }
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> anyhow::Result<Option<T>> {
    match fs::read_to_string(path) {
        Ok(raw) => Ok(Some(
            serde_json::from_str(&raw).with_context(|| format!("parse {}", path.display()))?,
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(anyhow::Error::from(e).context(format!("read {}", path.display()))),
    }
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> anyhow::Result<()> {
    let mut json = serde_json::to_string_pretty(value)?;
    json.push('\n');
    fs::write(path, json).with_context(|| format!("write {}", path.display()))
}

/// Finds packs: the bundled folder, user packs under `user_root`, and linked folders.
#[derive(Debug, Clone)]
pub struct PackStore {
    pub bundled_dir: PathBuf,
    pub user_root: PathBuf,
}

impl PackStore {
    pub fn new(bundled_dir: PathBuf, user_root: PathBuf) -> Self {
        Self {
            bundled_dir,
            user_root,
        }
    }

    /// User packs live in `%LOCALAPPDATA%\race-refinery\voice-packs\<id>`.
    pub fn default_user_root() -> PathBuf {
        race_refinery_settings::data_dir().join("voice-packs")
    }

    pub fn bundled(&self) -> anyhow::Result<VoicePack> {
        VoicePack::load(
            BUNDLED_PACK_ID.into(),
            PackKind::Bundled,
            self.bundled_dir.clone(),
        )
    }

    /// Open the pack a settings reference names.
    pub fn resolve(&self, id: &str) -> anyhow::Result<VoicePack> {
        let id = id.trim();
        if id.is_empty() || id == BUNDLED_PACK_ID {
            return self.bundled();
        }
        let path = Path::new(id);
        if path.is_absolute() {
            return VoicePack::load(id.to_string(), PackKind::Folder, path.to_path_buf());
        }
        if !is_valid_pack_id(id) {
            anyhow::bail!("invalid voice pack id '{id}'");
        }
        VoicePack::load(id.to_string(), PackKind::User, self.user_root.join(id))
    }

    /// The pack the coach should speak with; the bundled pack when the chosen one
    /// is gone (a deleted user pack, an unplugged drive).
    pub fn resolve_or_bundled(&self, id: &str) -> VoicePack {
        self.resolve(id).unwrap_or_else(|e| {
            tracing::warn!("voice pack '{id}' unavailable, using the bundled pack: {e:#}");
            self.bundled().unwrap_or_else(|_| VoicePack {
                id: BUNDLED_PACK_ID.into(),
                kind: PackKind::Bundled,
                dir: self.bundled_dir.clone(),
                meta: PackMeta {
                    name: default_name(PackKind::Bundled, &self.bundled_dir),
                    ..PackMeta::default()
                },
                clips: BTreeMap::new(),
            })
        })
    }

    /// Bundled pack, then user packs by name, then linked folders that still exist.
    pub fn list(&self, folders: &[String]) -> Vec<VoicePack> {
        let mut out: Vec<VoicePack> = self.bundled().into_iter().collect();
        let mut user: Vec<VoicePack> = fs::read_dir(&self.user_root)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().is_dir())
            .filter_map(|e| {
                let id = e.file_name().to_string_lossy().into_owned();
                if !is_valid_pack_id(&id) {
                    return None;
                }
                VoicePack::load(id, PackKind::User, e.path()).ok()
            })
            .collect();
        user.sort_by_key(|p| p.meta.name.to_lowercase());
        out.extend(user);
        out.extend(folders.iter().filter_map(|f| self.resolve(f).ok()));
        out
    }

    /// Create an empty user pack named `name`.
    pub fn create(&self, name: &str) -> anyhow::Result<VoicePack> {
        let name = name.trim();
        if name.is_empty() {
            anyhow::bail!("pack name is empty");
        }
        let id = self.unused_id(name);
        let pack = VoicePack {
            id: id.clone(),
            kind: PackKind::User,
            dir: self.user_root.join(&id),
            meta: PackMeta {
                name: name.to_string(),
                ..PackMeta::default()
            },
            clips: BTreeMap::new(),
        };
        pack.save()?;
        Ok(pack)
    }

    /// Remove a user pack folder. Linked folders are never deleted from disk.
    pub fn delete_user_pack(&self, id: &str) -> anyhow::Result<()> {
        let pack = self.resolve(id)?;
        if pack.kind != PackKind::User {
            anyhow::bail!("only packs created in Race Refinery can be deleted");
        }
        fs::remove_dir_all(&pack.dir)?;
        Ok(())
    }

    fn unused_id(&self, name: &str) -> String {
        let base = slugify(name);
        let mut id = base.clone();
        let mut n = 2;
        while self.user_root.join(&id).exists() || id == BUNDLED_PACK_ID {
            id = format!("{base}-{n}");
            n += 1;
        }
        id
    }
}

fn is_valid_pack_id(id: &str) -> bool {
    !id.is_empty()
        && id != BUNDLED_PACK_ID
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn slugify(name: &str) -> String {
    let mut slug = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
    }
    let slug = slug.trim_end_matches('-').to_string();
    if slug.is_empty() {
        "pack".into()
    } else {
        slug
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::path::PathBuf;

    /// A fresh, empty temp folder unique to `name` and this process.
    pub fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("race-refinery-audio-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Write a short mono 16-bit wav.
    pub fn write_wav(path: &std::path::Path, rate: u32, samples: &[f32]) {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(path, spec).unwrap();
        for s in samples {
            w.write_sample((s * i16::MAX as f32) as i16).unwrap();
        }
        w.finalize().unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::{temp_dir, write_wav};
    use super::*;

    fn store(name: &str) -> PackStore {
        let root = temp_dir(name);
        let bundled = root.join("bundled");
        fs::create_dir_all(&bundled).unwrap();
        PackStore::new(bundled, root.join("user"))
    }

    #[test]
    fn empty_bundled_pack_reports_every_key_missing() {
        let store = store("empty-bundled");
        let status = store.bundled().unwrap().status();
        assert!(status.read_only);
        assert_eq!(status.spotter.recorded, 0);
        assert_eq!(status.engineer.recorded, 0);
        assert_eq!(status.missing.len(), phrases().len());
        assert!(!status.custom_beep);
    }

    #[test]
    fn wav_files_named_after_keys_count_without_a_manifest() {
        let store = store("folder-wavs");
        let folder = store.user_root.parent().unwrap().join("loose");
        fs::create_dir_all(&folder).unwrap();
        write_wav(&folder.join("flag_green.wav"), 22_050, &[0.1; 100]);
        write_wav(&folder.join("n7.wav"), 22_050, &[0.1; 100]);
        write_wav(&folder.join("radio_beep.wav"), 22_050, &[0.1; 100]);
        write_wav(&folder.join("not_a_key.wav"), 22_050, &[0.1; 100]);

        let pack = store.resolve(folder.to_str().unwrap()).unwrap();
        assert_eq!(pack.kind, PackKind::Folder);
        let status = pack.status();
        assert_eq!(status.spotter.recorded, 1);
        assert_eq!(status.engineer.recorded, 1);
        assert!(status.custom_beep);
        assert!(!pack.clips.contains_key("not_a_key"));
    }

    #[test]
    fn create_list_and_delete_user_packs() {
        let store = store("create");
        let a = store.create("My Voice!").unwrap();
        let b = store.create("My Voice").unwrap();
        assert_eq!(a.id, "my-voice");
        assert_eq!(b.id, "my-voice-2");

        let listed: Vec<_> = store.list(&[]).into_iter().map(|p| p.id).collect();
        // Sorted by display name: "My Voice" before "My Voice!".
        assert_eq!(listed, ["default", "my-voice-2", "my-voice"]);

        store.delete_user_pack("my-voice").unwrap();
        assert!(store.resolve("my-voice").is_err());
        assert!(store.delete_user_pack("default").is_err());
    }

    #[test]
    fn bundled_pack_rejects_writes() {
        let store = store("readonly");
        let mut pack = store.bundled().unwrap();
        assert!(pack.clip_file_for("flag_green").is_err());
        assert!(pack.register_clip("flag_green").is_err());
    }

    #[test]
    fn user_packs_reject_unknown_keys_and_path_ids() {
        let store = store("validate");
        let pack = store.create("Test").unwrap();
        assert!(pack.clip_file_for("flag_green").is_ok());
        assert!(pack.clip_file_for("../evil").is_err());
        assert!(store.resolve("../escape").is_err());
    }

    #[test]
    fn missing_pack_falls_back_to_bundled() {
        let store = store("fallback");
        assert_eq!(store.resolve_or_bundled("gone").kind, PackKind::Bundled);
        assert_eq!(store.resolve_or_bundled("").id, BUNDLED_PACK_ID);
    }
}
