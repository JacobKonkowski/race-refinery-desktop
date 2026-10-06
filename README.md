# Race Refinery

[![CI](https://github.com/JacobKonkowski/race-refinery-desktop/actions/workflows/ci.yml/badge.svg)](https://github.com/JacobKonkowski/race-refinery-desktop/actions/workflows/ci.yml)

Race Refinery is iRacing telemetry for Windows: post-session IBT analysis, live shared-memory telemetry, a rule-based voice coach, and an in-headset HUD via Race Refinery’s OpenXR layer (plus a local web preview).

**Repository:** [github.com/JacobKonkowski/race-refinery-desktop](https://github.com/JacobKonkowski/race-refinery-desktop)

## Quick start

```powershell
git clone https://github.com/JacobKonkowski/race-refinery-desktop.git
cd race-refinery-desktop
.\setup.ps1 -SkipBuild
npm run tauri dev
```

**Full setup + race-night checklist:** [docs/SETUP.md](docs/SETUP.md)  
**Contributor / architecture map:** [docs/FOUNDATION.md](docs/FOUNDATION.md) · [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) · [SECURITY.md](SECURITY.md)

## Prerequisites

| Requirement | Notes |
|-------------|-------|
| [Rust](https://rustup.rs/) 1.89+ | Required by the `pitwall` crate |
| [Node.js](https://nodejs.org/) 18+ | Frontend build and Tauri CLI |
| iRacing | Disk telemetry for IBT import; shared memory for live monitor |
| OpenXR VR runtime (optional) | In-headset HUD via Race Refinery’s OpenXR API layer |

### iRacing configuration

Add to `Documents\iRacing\app.ini`:

```ini
irsdkEnableMem=1    ; live telemetry (shared memory)
irsdkEnableDisk=1   ; IBT file recording
```

Record telemetry in-car with **Alt+L**. Files land in `Documents\iRacing\telemetry\*.ibt`.

## Features

The UI is a feature shell with **Analyze**, **Live**, **Voice Studio**, and **Settings** (`src/features/registry.ts`).

### Analyze (post-session)

- Auto-import from the telemetry folder (file watcher) plus manual import
- Session browser, lap table with P/Q/R grouping, sectors, and `paceEligible` laps (official time, sim `_OK` flags, and near-full distance coverage)
- Two-lap trace compare via `compare_laps`
- Fuel / tire panels from stored lap facts
- **Insights** strip — deterministic client-side bullets (consistency, weak sector, fuel outliers)

### Live (in-session)

- Real-time telemetry via `pitwall::Pitwall::connect()`
- Live leaderboard, session deltas, coach message preview
- **Audio coach** — every callout is a human recording from a voice pack (no TTS); lap times and gaps are composed from recorded number clips. See [docs/AUDIO_COACH.md](docs/AUDIO_COACH.md)

### Voice Studio

- Record your own coach voice: teleprompter, hold Space to record, automatic clean-up (trim, noise gate, loudness), undo
- Preview composed callouts and play back every Spotter line before you drive
- Share packs as a zip or a linked folder. See [docs/VOICE_PACKS.md](docs/VOICE_PACKS.md)
- **In-headset HUD** — Race Refinery OpenXR API layer; web preview at `http://127.0.0.1:17342/vr`. See [docs/NATIVE_VR.md](docs/NATIVE_VR.md)
- Demo clock for offline UI / SHM test pattern without a sim session

### Field awareness

Live gaps, pack state, and session best/optimal deltas while connected. See [docs/COMPARISON.md](docs/COMPARISON.md).

## Build & run

| Command | Purpose |
|---------|---------|
| `npm run tauri dev` | Dev mode |
| `npm run build` | Frontend only (`tsc` + Vite) |
| `npm run tauri build` | Release installer |
| `npm run docs:api` | rustdoc + TypeDoc from `src/shared` |
| `cargo test --manifest-path src-tauri/Cargo.toml --lib` | Backend unit tests |

## Data storage

- SQLite under `%LOCALAPPDATA%\race-refinery\` (schema **v2** — see [docs/DATA_MODEL.md](docs/DATA_MODEL.md))
- Settings JSON beside the DB (audio + VR + overlay layout for VR slots)
- Coach voice packs: bundled pack in `src-tauri/resources/audio/coach/default/`, your packs under `%LOCALAPPDATA%\race-refinery\voice-packs\`

## Documentation

Start at [docs/FOUNDATION.md](docs/FOUNDATION.md) and [docs/README.md](docs/README.md).

## License

See repository license file.
