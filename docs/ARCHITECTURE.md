# Architecture

Race Refinery is a Tauri 2 + React desktop app with a **Cargo workspace** of domain crates
and a thin `src-tauri` composition root. See [FOUNDATION.md](FOUNDATION.md) for the
dependency table and contributor playbook.

```
┌─────────────────────────────────────────────────────────┐
│  React shell + features (Analyze | Live | Voice Studio | Settings) │
│  widgets catalog → monitor windows + VR HUD             │
└─────────────┬───────────────────────────┬───────────────┘
              │ invoke / events           │
┌─────────────▼───────────────────────────▼───────────────┐
│  race-refinery-desktop (commands / AppState)                  │
├──────────┬──────────┬──────────┬──────────┬─────────────┤
│ ingest   │ live     │ audio    │ monitor  │ vr          │
│ analysis │ storage  │ settings │ telemetry│             │
└──────────┴──────────┴──────────┴──────────┴─────────────┘
```

## Frontend

| Path | Role |
|------|------|
| `src/shell/` | AppShell, feature nav |
| `src/features/registry.ts` | Analyze, Live, Voice Studio, Settings |
| `src/features/analyze/` | Post-session UI |
| `src/features/live/` | Live / coach / monitor / VR controls |
| `src/features/voice-studio/` | Record and preview coach voice packs |
| `src/features/settings/` | VR placement, recenter bindings, audio coach and voice pack picker |
| `src/shared/` | api, types, format, toast, i18n, navigation (`navigateToFeature`) |
| `src/widgets/` | Shared presentational widgets |
| `src/monitor/` | Monitor window entry (when present) |

## Live data paths

1. **UI** — ~10 Hz snapshot  
2. **Monitor** — always-on-top Tauri windows per enabled widget  
3. **Native VR** — ~30 Hz SHM for OpenXR layer  
4. **Web HUD** — `:17342`  
5. **Audio** — 250 ms rule engine poll; callouts play from the active voice pack ([AUDIO_COACH.md](AUDIO_COACH.md), [VOICE_PACKS.md](VOICE_PACKS.md))  

## Related

- [FOUNDATION.md](FOUNDATION.md) — crates and rules  
- [NATIVE_VR.md](NATIVE_VR.md) — OpenXR details  
- [PLUGINS.md](PLUGINS.md) — extension seams  
