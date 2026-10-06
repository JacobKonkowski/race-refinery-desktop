# Data model

SQLite at `%LOCALAPPDATA%\race-refinery\` (see `storage/db.rs`). **`PRAGMA user_version = 6`**.

Opening a pre-v2 DB drops analysis tables (`sessions`, `laps`, `sectors`, `lap_traces`) and requires reimport. v2 → v6 are additive migrations on `lap_traces` (GPS, elapsed time, raw pedals, then ABS activity), so existing sessions survive — they just have no GPS / exact corner timing / driver-pedal coloring / assist data until re-imported.

## Tables

### `sessions`

| Column | Notes |
|--------|--------|
| `ibt_path`, `file_hash` | Dedup + source |
| `track`, `car`, `session_date` | Metadata |
| `lap_count`, `best_lap_ms` | Summary (best is pace-eligible; list/detail may refresh from cleaned laps) |
| `imported_at` | ISO timestamp |

`SessionSummary` also exposes a derived `session_type` (not a stored column): the `laps.session_type` of the highest `session_num` (latest stint) for that session, recomputed alongside `lap_count`/`best_lap_ms` whenever laps are loaded.

### `laps` (schema v2)

| Column | Notes |
|--------|--------|
| `session_num`, `session_type`, `iracing_lap`, `lap_number` | Identity |
| `lap_time_ms` | Official time when present (sticky copies cleared at import / on read) |
| `delta_best_ok`, `delta_session_best_ok` | iRacing `_OK` integers |
| `on_pit_road_start`, `on_pit_road_end` | Pit-road samples |
| `lap_dist_pct_min`, `lap_dist_pct_max` | Coverage (pace eligibility needs max ≥ 0.95) |
| `pace_eligible` | Derived: time + both `_OK` + full coverage (see [ANALYSIS.md](ANALYSIS.md)) |
| `fuel_*`, `avg_speed`, tire temps | Optional aggregates |

### `sectors` / `lap_traces`

Per-lap sector times and distance-sampled traces (speed, throttle, brake, gear, steering).

`lap_traces.lat` / `.lon` (schema v3, nullable) keep the GPS for each sample. They are `NULL` for sessions imported before v3 and for IBTs without GPS channels.

`lap_traces.elapsed_ms` (schema v4, nullable) is the time since the lap's first frame.
Compare uses it for the running delta and corner times; older rows fall back to a
speed-integrated estimate (see [ANALYSIS.md](ANALYSIS.md#lap-compare-and-corners)).

`throttle` / `brake` are the **applied** values (`Throttle` / `Brake`), after auto-blip, traction control and ABS. Schema v5 adds the driver's pedals alongside them, all nullable:

| Column | SDK channel | Notes |
|--------|-------------|-------|
| `throttle_raw` | `ThrottleRaw` | Driver throttle; no downshift blips |
| `brake_raw` | `BrakeRaw` | Driver brake, before ABS |
| `clutch` | `Clutch` | Applied clutch (0 = disengaged, 1 = engaged) |
| `clutch_raw` | `ClutchRaw` | Driver clutch pedal |
| `handbrake_raw` | `HandbrakeRaw` | Driver handbrake |

They are `NULL` for sessions imported before v5 and for IBTs without the channel. Corner
pickup reads raw with a fallback to applied; compare charts stay on applied. Clutch and
handbrake are stored only — nothing displays them yet.

`lap_traces.abs_active` (schema v6, nullable `0`/`1`) is `BrakeABSactive`: ABS reducing
brake pressure. A trace sample stands for 6 IBT frames, and the flag is OR-ed across them
so short ABS pulses survive downsampling. `NULL` before v6 or when the IBT lacks the
channel (cars without ABS still record it, as `0`).

## Settings

JSON beside the DB (`crates/race-refinery-settings` → `AppSettings`). Includes:

- `vrMode` (`native` \| `web`), HUD offset/opacity, recenter bindings (`vrRecenterHotkey` accelerator string, `vrRecenterButton` `{deviceGuid, deviceName, button}` or null)
- `overlayLayout` — five widget slots (monitor + VR) + field pace mode; layouts saved by older builds are padded to the current slot count on load. Each slot's VR fields: `vrLock` (`world` \| `head`), `vrOffsetX/Y/Z`, `vrTiltDeg`, `vrScale`, `vrOpacity`; missing fields load as world-locked with zero offsets
- Audio coach volume, chatter level, category toggles, gaps
- Voice packs: `audioCoachPackId` (`default`, a user pack id, or an absolute folder path), `audioCoachPackFolders` (linked folder packs), `audioCoachMicDevice` (Voice Studio mic; empty = system default), `audioStudioAutoAdvance`. The retired `audioCoachVoice` and `audioCoachRate` keys from older builds are ignored on load and dropped on the next save; there is no TTS voice to migrate

Shared `enabled` flags; `desktop*` places monitor windows, `vr*` places the in-headset HUD (enable once, place twice).

Some older geometry fields may still exist in the JSON for migration; the UI drives slots via `overlayLayout`.

Frontend types: `src/shared/types.ts`.

## Track map cache

Generated circuit outlines are stored as JSON beside the database:
`%LOCALAPPDATA%\race-refinery\track-maps\{slug}.json`. Imports write via
`race_refinery_storage::save_track_map`; Analyze / monitor / VR read via `load_track_map`.
Outlines require GPS channels on the source IBT. Trace `lat` / `lon` (kept on
`lap_traces`) let Analyze draw a lap's racing line in the outline's projection.
