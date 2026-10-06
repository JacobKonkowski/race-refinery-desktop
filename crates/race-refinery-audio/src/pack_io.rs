//! Moving voice packs around: clone, zip import / export, and importing a folder of
//! `{key}.wav` files recorded in another tool.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{Cursor, Read, Write};
use std::path::{Component, Path, PathBuf};

use anyhow::Context;
use serde::Serialize;

use super::dsp;
use super::pack::{PackMeta, PackStore, VoicePack};
use super::phrases::is_pack_key;

/// Zip safety limits: a pack is ~100 short clips.
const MAX_ZIP_ENTRIES: usize = 2_000;
const MAX_CLIP_BYTES: u64 = 50 * 1024 * 1024;

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    /// Keys added that the pack did not have.
    pub imported: Vec<String>,
    /// Keys whose existing clip was replaced.
    pub overwritten: Vec<String>,
    /// WAV files whose name is not a pack key.
    pub unknown: Vec<String>,
    /// `key: reason` for files that could not be decoded.
    pub failed: Vec<String>,
}

impl ImportReport {
    fn record(&mut self, key: &str, existed: bool) {
        if existed {
            self.overwritten.push(key.to_string());
        } else {
            self.imported.push(key.to_string());
        }
    }
}

/// Copy every clip of `source` into a new user pack named `name`.
pub fn clone_pack(store: &PackStore, source: &VoicePack, name: &str) -> anyhow::Result<VoicePack> {
    let mut pack = store.create(name)?;
    pack.meta.author = source.meta.author.clone();
    pack.meta.license = source.meta.license.clone();
    pack.meta.language = source.meta.language.clone();
    for key in source.clips.keys() {
        let Some(from) = source.clip_path(key) else {
            continue;
        };
        let Ok(to) = pack.clip_file_for(key) else {
            continue;
        };
        fs::copy(&from, &to).with_context(|| format!("copy {}", from.display()))?;
        pack.clips.insert(key.clone(), format!("{key}.wav"));
    }
    pack.save()?;
    Ok(pack)
}

/// Write `pack` as a flat zip: `meta.json`, `manifest.json`, and `{key}.wav`.
/// Returns the number of clips written.
pub fn export_zip(pack: &VoicePack, dest: &Path) -> anyhow::Result<usize> {
    let file = File::create(dest).with_context(|| format!("create {}", dest.display()))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    let mut manifest = BTreeMap::new();
    for key in pack.clips.keys() {
        let Some(path) = pack.clip_path(key) else {
            continue;
        };
        let name = format!("{key}.wav");
        zip.start_file(name.as_str(), options)?;
        zip.write_all(&fs::read(&path)?)?;
        manifest.insert(key.clone(), name);
    }
    zip.start_file("meta.json", options)?;
    zip.write_all(serde_json::to_string_pretty(&pack.meta)?.as_bytes())?;
    zip.start_file("manifest.json", options)?;
    zip.write_all(serde_json::to_string_pretty(&manifest)?.as_bytes())?;
    zip.finish()?;
    Ok(manifest.len())
}

/// `a/b/c.wav` inside the zip, rejecting absolute and `..` paths.
fn safe_entry_path(name: &str) -> Option<PathBuf> {
    let path = PathBuf::from(name.replace('\\', "/"));
    path.components()
        .all(|c| matches!(c, Component::Normal(_)))
        .then_some(path)
}

/// Import a pack zip as a new user pack. The zip may hold the pack at its root or
/// inside one top-level folder.
pub fn import_zip(store: &PackStore, path: &Path) -> anyhow::Result<(VoicePack, ImportReport)> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut archive = zip::ZipArchive::new(file).context("not a zip file")?;
    if archive.len() > MAX_ZIP_ENTRIES {
        anyhow::bail!("zip has too many files to be a voice pack");
    }

    let mut files: Vec<(usize, PathBuf)> = Vec::new();
    for i in 0..archive.len() {
        let entry = archive.by_index(i)?;
        if entry.is_dir() {
            continue;
        }
        if let Some(p) = safe_entry_path(entry.name()) {
            files.push((i, p));
        }
    }
    let root = common_root(&files);
    let rel = |p: &Path| p.strip_prefix(&root).unwrap_or(p).to_path_buf();

    let mut read_entry = |index: usize| -> anyhow::Result<Vec<u8>> {
        let mut entry = archive.by_index(index)?;
        if entry.size() > MAX_CLIP_BYTES {
            anyhow::bail!("{} is too large", entry.name());
        }
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut bytes)?;
        Ok(bytes)
    };
    let find = |name: &str| {
        files
            .iter()
            .find(|(_, p)| rel(p) == Path::new(name))
            .map(|(i, _)| *i)
    };

    let meta: PackMeta = match find("meta.json") {
        Some(i) => serde_json::from_slice(&read_entry(i)?).context("meta.json")?,
        None => PackMeta::default(),
    };
    let manifest: BTreeMap<String, String> = match find("manifest.json") {
        Some(i) => serde_json::from_slice(&read_entry(i)?).context("manifest.json")?,
        None => BTreeMap::new(),
    };

    // Key -> zip entry: manifest paths first, then `{key}.wav` anywhere in the pack root.
    let mut sources: BTreeMap<String, usize> = BTreeMap::new();
    for (key, file) in &manifest {
        if is_pack_key(key) {
            if let Some(i) = find(&file.replace('\\', "/")) {
                sources.insert(key.clone(), i);
            }
        }
    }
    let mut report = ImportReport::default();
    for (i, p) in &files {
        let name = rel(p);
        let is_wav = name
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("wav"));
        if !is_wav || name.components().count() != 1 {
            continue;
        }
        let stem = name.file_stem().unwrap_or_default().to_string_lossy();
        if is_pack_key(&stem) {
            sources.entry(stem.into_owned()).or_insert(*i);
        } else if !sources.values().any(|v| v == i) {
            report.unknown.push(name.to_string_lossy().into_owned());
        }
    }

    let name = if meta.name.trim().is_empty() {
        path.file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Imported pack".into())
    } else {
        meta.name.clone()
    };
    let mut pack = store.create(&name)?;
    pack.meta.author = meta.author;
    pack.meta.license = meta.license;
    pack.meta.language = meta.language;

    for (key, index) in sources {
        let converted = read_entry(index).and_then(|bytes| dsp::decode_wav(Cursor::new(bytes)));
        match converted {
            Ok(samples) => {
                dsp::write_pack_wav(&pack.clip_file_for(&key)?, &samples)?;
                pack.clips.insert(key.clone(), format!("{key}.wav"));
                report.record(&key, false);
            }
            Err(e) => report.failed.push(format!("{key}: {e:#}")),
        }
    }
    pack.save()?;
    Ok((pack, report))
}

/// The single top-level folder every file sits in, or empty when files are at the root.
fn common_root(files: &[(usize, PathBuf)]) -> PathBuf {
    let mut root: Option<&std::ffi::OsStr> = None;
    for (_, p) in files {
        let mut parts = p.components();
        let (Some(first), Some(_)) = (parts.next(), parts.next()) else {
            return PathBuf::new();
        };
        let first = first.as_os_str();
        match root {
            None => root = Some(first),
            Some(r) if r == first => {}
            Some(_) => return PathBuf::new(),
        }
    }
    root.map(PathBuf::from).unwrap_or_default()
}

/// Merge every `{key}.wav` in `folder` into `pack`, converting to the pack format.
pub fn import_wav_folder(pack: &mut VoicePack, folder: &Path) -> anyhow::Result<ImportReport> {
    if pack.read_only() {
        anyhow::bail!("the bundled pack is read-only; clone it first");
    }
    let mut entries: Vec<PathBuf> = fs::read_dir(folder)
        .with_context(|| format!("read {}", folder.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e.eq_ignore_ascii_case("wav")))
        .collect();
    entries.sort();

    let mut report = ImportReport::default();
    for path in entries {
        let stem = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        if !is_pack_key(&stem) {
            report.unknown.push(
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
            );
            continue;
        }
        let dest = pack.clip_file_for(&stem)?;
        if dest == path {
            continue;
        }
        match dsp::read_wav_file(&path) {
            Ok(samples) => {
                let existed = pack.has(&stem);
                dsp::write_pack_wav(&dest, &samples)?;
                pack.clips.insert(stem.clone(), format!("{stem}.wav"));
                report.record(&stem, existed);
            }
            Err(e) => report.failed.push(format!("{stem}: {e:#}")),
        }
    }
    pack.save()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::super::pack::test_support::{temp_dir, write_wav};
    use super::super::pack::PackKind;
    use super::*;

    fn store(name: &str) -> (PackStore, PathBuf) {
        let root = temp_dir(name);
        let bundled = root.join("bundled");
        fs::create_dir_all(&bundled).unwrap();
        write_wav(&bundled.join("flag_green.wav"), 22_050, &[0.1; 500]);
        write_wav(&bundled.join("n5.wav"), 22_050, &[0.1; 500]);
        (PackStore::new(bundled, root.join("user")), root)
    }

    #[test]
    fn clone_copies_clips_into_an_editable_pack() {
        let (store, _) = store("clone");
        let bundled = store.bundled().unwrap();
        let copy = clone_pack(&store, &bundled, "My copy").unwrap();
        assert_eq!(copy.kind, PackKind::User);
        assert!(!copy.read_only());
        assert!(copy.has("flag_green") && copy.has("n5"));
        let reloaded = store.resolve(&copy.id).unwrap();
        assert_eq!(reloaded.meta.name, "My copy");
        assert!(reloaded.has("n5"));
    }

    #[test]
    fn zip_round_trip_preserves_clips_and_name() {
        let (store, root) = store("zip");
        let mut source = clone_pack(&store, &store.bundled().unwrap(), "Shared voice").unwrap();
        source.meta.author = "Jacob".into();
        source.save().unwrap();

        let zip_path = root.join("shared.zip");
        assert_eq!(export_zip(&source, &zip_path).unwrap(), 2);

        let (imported, report) = import_zip(&store, &zip_path).unwrap();
        assert_eq!(imported.meta.name, "Shared voice");
        assert_eq!(imported.meta.author, "Jacob");
        assert_ne!(imported.id, source.id);
        assert_eq!(report.imported, ["flag_green", "n5"]);
        assert!(imported.has("flag_green") && imported.has("n5"));
    }

    #[test]
    fn zip_with_a_top_level_folder_and_stray_files() {
        let (store, root) = store("zip-folder");
        let zip_path = root.join("nested.zip");
        let mut zip = zip::ZipWriter::new(File::create(&zip_path).unwrap());
        let opts = zip::write::SimpleFileOptions::default();
        let mut wav = Vec::new();
        {
            let spec = hound::WavSpec {
                channels: 2,
                sample_rate: 48_000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            };
            let mut w = hound::WavWriter::new(Cursor::new(&mut wav), spec).unwrap();
            for _ in 0..4_800 {
                w.write_sample(1000_i16).unwrap();
            }
            w.finalize().unwrap();
        }
        for name in [
            "voice/pits_open.wav",
            "voice/readme.wav",
            "voice/../evil.wav",
        ] {
            zip.start_file(name, opts).unwrap();
            zip.write_all(&wav).unwrap();
        }
        zip.finish().unwrap();

        let (pack, report) = import_zip(&store, &zip_path).unwrap();
        assert_eq!(pack.meta.name, "nested");
        assert_eq!(report.imported, ["pits_open"]);
        assert_eq!(report.unknown, ["readme.wav"]);
        let spec = hound::WavReader::open(pack.clip_path("pits_open").unwrap())
            .unwrap()
            .spec();
        assert_eq!((spec.channels, spec.sample_rate), (1, dsp::PACK_RATE));
    }

    #[test]
    fn wav_folder_import_reports_matches() {
        let (store, root) = store("wav-folder");
        let mut pack = store.create("Mine").unwrap();
        write_wav(&pack.dir.join("n5.wav"), 22_050, &[0.1; 100]);
        pack.clips.insert("n5".into(), "n5.wav".into());

        let folder = root.join("takes");
        fs::create_dir_all(&folder).unwrap();
        write_wav(&folder.join("n5.wav"), 44_100, &[0.2; 4410]);
        write_wav(&folder.join("flag_red.WAV"), 22_050, &[0.2; 100]);
        write_wav(&folder.join("take_03.wav"), 22_050, &[0.2; 100]);
        fs::write(folder.join("broken.wav"), b"not audio").unwrap();
        fs::write(folder.join("n6.wav"), b"not audio").unwrap();

        let report = import_wav_folder(&mut pack, &folder).unwrap();
        assert_eq!(report.imported, ["flag_red"]);
        assert_eq!(report.overwritten, ["n5"]);
        assert_eq!(report.unknown, ["broken.wav", "take_03.wav"]);
        assert_eq!(report.failed.len(), 1);
        assert!(report.failed[0].starts_with("n6"));
        assert!(store.resolve(&pack.id).unwrap().has("flag_red"));
    }

    #[test]
    fn bundled_pack_rejects_wav_import() {
        let (store, root) = store("wav-readonly");
        let mut bundled = store.bundled().unwrap();
        assert!(import_wav_folder(&mut bundled, &root).is_err());
    }
}
