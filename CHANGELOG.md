# Changelog

All notable changes to Race Refinery are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Modular foundation work in progress (workspace crates, dual-surface widgets, OSS hygiene).
- **Voice Studio** and shareable **voice packs**: record the coach in-app (hold Space, take clean-up, undo), preview composed callouts, and import / export zips or linked folders. See [docs/VOICE_PACKS.md](docs/VOICE_PACKS.md).
- **Brand system**: app icons, mark/logo SVGs, and promo/social art under [`brand/`](brand/README.md); desktop icons in `src-tauri/icons/` and runtime assets in `public/brand/`.

### Changed

- **Renamed from PitWall to Race Refinery.** Bundle ID is now `com.racerefinery.desktop`, crates are `race-refinery-*`, the OpenXR layer is `XR_APILAYER_RACE_REFINERY_overlay` (`race-refinery-openxr-layer.dll`), and the VR shared memory is `Local\RaceRefineryVR`.
- **Clean break, no migration.** Settings, the session database, and the track map cache now live in `%LOCALAPPDATA%\race-refinery\` and start fresh; the old `%LOCALAPPDATA%\pitwall-desktop\` folder is not read. Reinstall the VR layer from Live → Install VR layer; installing removes any old PitWall layer registration.
- **Audio coach is clips-only.** Piper and Windows speech are gone. Every word is a human recording from the active voice pack; the bundled default is the maintainer's voice. Retired settings `audioCoachVoice` / `audioCoachRate` are ignored on load.

## [0.1.0] - 2026-09-09

### Added

- Analyze (IBT import) and Live (telemetry, Path B audio coach, native OpenXR HUD)
- Lap cleanup (phantom / sticky times / coverage-based pace eligibility)
