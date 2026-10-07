# Plugins & extension points

Race Refinery does not load third-party DLLs at runtime. Extensions land as **in-repo
slices** (or forks) using stable seams:

## Coach rules

1. Add `crates/race-refinery-audio/src/engine/rules/my_rule.rs`
2. Register in `rules/mod.rs`
3. Add phrase keys if needed (`crates/race-refinery-audio/src/phrases.txt`); packs record them in Voice Studio
4. Unit-test the rule with a synthetic `LiveSnapshot`

Do not call Tauri or storage from a rule — only `RaceContext` / settings flags.

## Widgets

1. Add a React component under `src/widgets/`
2. Register in the widget catalog (`src/widgets/index.tsx`) and `WidgetKind` types
3. Add default monitor + VR placement in settings defaults
4. Monitor host and VR SHM pick up enabled slots

Keep widgets presentational: props from `LiveSnapshot`, no IPC inside the widget.

## Stable IPC subset

Prefer existing commands in `src/shared/api.ts` / `commands/mod.rs`. New IPC should
be documented in [API.md](API.md) and keep camelCase DTOs in `shared/types.ts`
(or generated types when codegen is enabled).

## Out of scope for plugins

- Injecting into the OpenXR layer binary from outside the repo
- Network microservices
- Replacing the live SDK reader
