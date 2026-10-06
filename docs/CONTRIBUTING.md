# Contributing

Thanks for helping improve Race Refinery.

---

## Prerequisites

Same as [SETUP.md](SETUP.md):

- Windows 10/11, Rust 1.89+, Node 18+
- `npm install` then `npm run tauri dev`

---

## Commands

| Command | Purpose |
|---------|---------|
| `npm run tauri dev` | Dev app + hot reload |
| `npm run build` | Frontend production build |
| `npm run tauri build` | Release installer |
| `cargo test --manifest-path src-tauri/Cargo.toml` | Rust unit tests |
| `npm run docs:api` | Generate rustdoc + TypeDoc (output in `docs/.api-out/`, gitignored) |

OpenXR layer build: [NATIVE_VR.md](NATIVE_VR.md) and [openxr-layer/README.md](../openxr-layer/README.md).

---

## Code layout

| Area | Path |
|------|------|
| IPC / state | `src-tauri/src/commands/mod.rs` |
| Live telemetry | `src-tauri/src/live/` |
| Audio coach + voice packs | `crates/race-refinery-audio/`, `src-tauri/src/commands/voice.rs` |
| Settings | `src-tauri/src/settings/` |
| IBT analysis | `src-tauri/src/analysis/`, `ingest/`, `storage/` |
| VR | `src-tauri/src/vr/`, `openxr-layer/` |
| Frontend shell | `src/shell/`, `src/features/registry.ts` |
| Features | `src/features/analyze/`, `src/features/live/`, `src/features/voice-studio/`, `src/features/settings/` |
| Shared IPC | `src/shared/` |
| HUD widgets | `src/widgets/` |
| Monitor host | `src/monitor/`, `crates/race-refinery-monitor` |
| Docs hub | `docs/README.md` |

Start with [ARCHITECTURE.md](ARCHITECTURE.md) for the system map.

---

## Conventions

- Rust modules by domain; `#[tauri::command]` handlers in `commands/mod.rs`
- IPC JSON uses **camelCase** (`serde(rename_all = "camelCase")`)
- TypeScript types in `src/shared/types.ts` mirror Rust structs
- Prefer `cargo test --manifest-path src-tauri/Cargo.toml --lib` for unit tests
- Do not edit `.cursor/plans/*.plan.md` in PRs unless explicitly asked

---

## Coach voice clips

The coach speaks only human recordings; there is no clip generator. Phrase keys and prompts live in `crates/race-refinery-audio/src/phrases.txt`, and clips are recorded in the app's Voice Studio. The bundled pack (`src-tauri/resources/audio/coach/default/`) is the maintainer's recorded voice; only change it with new recordings from the same speaker, and record a clip for any new phrase key (the `bundled_pack_records_every_phrase` test enforces this). See [VOICE_PACKS.md](VOICE_PACKS.md) and [AUDIO_COACH.md](AUDIO_COACH.md).

---

## CI

[`.github/workflows/ci.yml`](../.github/workflows/ci.yml) on PRs to `main`:

- `npm ci` → `npm run build`
- `cargo test`
- `npm run docs:api` (smoke — ensures rustdoc + TypeDoc config valid)

---

## Documentation maintenance

| Code change | Update |
|-------------|--------|
| New Tauri command / event | `commands/mod.rs` `///`, `api.ts` TSDoc, **API.md** |
| New IPC type | `types.ts`, rustdoc, **DATA_MODEL.md** if stored |
| New settings field | **DATA_MODEL.md**, SETUP/LivePanel if user-facing |
| New audio message | **AUDIO_COACH.md**, `phrases.txt` (new keys show as missing in every pack) |
| Voice pack format / Voice Studio | **VOICE_PACKS.md** |
| Live field | **LIVE_TELEMETRY.md**, **COMPARISON.md** if SDK-related |
| Lap cleanup / pace rules | **ANALYSIS.md**, `analysis/cleanup.rs` |

Index: [docs/README.md](README.md). Prefer goal-oriented wording (what Race Refinery does); mention other apps only when needed for OpenXR load-order conflicts.

---

## Suggested reading order

1. `src-tauri/src/lib.rs`
2. `src-tauri/src/commands/mod.rs`
3. `src-tauri/src/live/mod.rs`
4. `crates/race-refinery-audio/src/lib.rs`
5. `src/shell/AppShell.tsx` + `src/shared/api.ts`
