//! On-disk cache of generated circuit outlines.
//!
//! Outlines are expensive to source (they need one clean GPS lap) but tiny to
//! store, so each track gets a JSON file beside the database:
//! `%LOCALAPPDATA%\race-refinery\track-maps\{slug}.json`. Imports write here;
//! Analyze, the live surfaces, and the VR producer read from here.

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};

use race_refinery_analysis::TrackOutline;

/// Directory holding one JSON file per track.
pub fn track_map_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("race-refinery")
        .join("track-maps")
}

/// Filesystem-safe key for a track's display name.
pub fn track_slug(track: &str) -> String {
    let slug: String = track
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    // Collapse runs of separators so "Monza - Full" and "Monza  Full" agree.
    let mut out = String::with_capacity(slug.len());
    for c in slug.chars() {
        if c == '-' && out.ends_with('-') {
            continue;
        }
        out.push(c);
    }
    out.trim_matches('-').to_string()
}

fn track_map_path(track: &str) -> Option<PathBuf> {
    let slug = track_slug(track);
    if slug.is_empty() {
        return None;
    }
    Some(track_map_dir().join(format!("{slug}.json")))
}

/// Load the cached outline for a track, or `None` when absent / unreadable.
pub fn load_track_map(track: &str) -> Option<TrackOutline> {
    let path = track_map_path(track)?;
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Store `outline` unless the cache already holds an equal-or-better one.
///
/// Returns whether the file was written.
pub fn save_track_map(outline: &TrackOutline) -> Result<bool> {
    let Some(path) = track_map_path(&outline.track) else {
        return Ok(false);
    };
    if let Some(existing) = load_track_map(&outline.track) {
        if !existing.is_improved_by(outline) {
            return Ok(false);
        }
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("create track-map directory")?;
    }
    let json = serde_json::to_string(outline).context("serialize track outline")?;
    fs::write(&path, json).with_context(|| format!("write track map {}", path.display()))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_is_filesystem_safe_and_stable() {
        assert_eq!(track_slug("Monza - Full"), "monza-full");
        assert_eq!(track_slug("Monza  Full"), "monza-full");
        assert_eq!(
            track_slug("Circuit de Spa-Francorchamps"),
            "circuit-de-spa-francorchamps"
        );
        assert_eq!(track_slug("  "), "");
    }

    #[test]
    fn missing_track_has_no_path() {
        assert!(track_map_path("").is_none());
    }
}
