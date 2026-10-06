# Race Refinery documentation hub

Race Refinery helps you get faster in iRacing: **Analyze** your IBT telemetry after a session, **Live** coach + HUD while you drive, and **Voice Studio** to record the coach's voice packs.

## Guides

| Doc | Contents |
|-----|----------|
| [FOUNDATION.md](FOUNDATION.md) | **Start here** — crates, deps, AI/PR playbook |
| [SETUP.md](SETUP.md) | Prerequisites, first run, race-night checklist |
| [FEATURES.md](FEATURES.md) | What Analyze, Live, Voice Studio, and Settings do |
| [TROUBLESHOOTING.md](TROUBLESHOOTING.md) | Import, live, audio coach / Voice Studio, VR |
| [API.md](API.md) | Tauri commands + frontend IPC (`src/shared`) |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Modules, feature registry, data flow |
| [DATA_MODEL.md](DATA_MODEL.md) | SQLite schema v2, settings |
| [FRONTEND.md](FRONTEND.md) | `features/`, `shared/`, `widgets/`, shell |
| [PRIVACY.md](PRIVACY.md) | Local-only data policy |
| [FIXTURES.md](FIXTURES.md) | Tests without personal IBTs |
| [PLUGINS.md](PLUGINS.md) | Extension seams for widgets/rules |
| [I18N.md](I18N.md) | UI string catalogs |
| [RELEASING.md](RELEASING.md) | Versions, tags, updater |

## Deep dives

| Doc | Contents |
|-----|----------|
| [AUDIO_COACH.md](AUDIO_COACH.md) | Callout rules, priorities, composing numbers from clips |
| [VOICE_PACKS.md](VOICE_PACKS.md) | Recording voice packs in Voice Studio, pack format, sharing |
| [NATIVE_VR.md](NATIVE_VR.md) | OpenXR layer, shared memory, install |
| [LIVE_TELEMETRY.md](LIVE_TELEMETRY.md) | Live service, demo clock, auto-import |
| [COMPARISON.md](COMPARISON.md) | Live field awareness (leaderboard, gaps, pack) |
| [ANALYSIS.md](ANALYSIS.md) | IBT pipeline, lap cleanup, pace eligibility |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Dev conventions |
| [DESIGN_NOTES.md](DESIGN_NOTES.md) | Product and SDK design choices |

## Historical

| Doc | Note |
|-----|------|
| [VR_NATIVE_SPIKE.md](VR_NATIVE_SPIKE.md) | Research notes that led to the native OpenXR layer. Not a setup guide. |

## Keep in sync when changing code

- Commands → `src-tauri/src/commands/*.rs`, `src/shared/api.ts`, [API.md](API.md)
- Coach phrases → `crates/race-refinery-audio/src/phrases.txt`, [AUDIO_COACH.md](AUDIO_COACH.md), [VOICE_PACKS.md](VOICE_PACKS.md)
- Types → `src/shared/types.ts`, [DATA_MODEL.md](DATA_MODEL.md)
- Live UI → `src/features/live/LivePage.tsx`
- Voice Studio UI → `src/features/voice-studio/*`
- Analyze UI → `src/features/analyze/*`
- Analysis cleanup → `crates/race-refinery-analysis`, [ANALYSIS.md](ANALYSIS.md)
- Feature list → `src/features/registry.ts`
- Crate map → [FOUNDATION.md](FOUNDATION.md)

Last updated: October 2026.
