# Features

Race Refinery exposes features via `src/features/registry.ts`: **Analyze**, **Live**, and **Settings**.

## Analyze

| Capability | Notes |
|------------|--------|
| Session browser | Lists imported IBTs; search by track/car/date; sort by date/car/track; hide sessions with no pace-eligible laps; shows a color-coded session-type letter (R/Q/P/T) per session; delete per session |
| Import | File / folder pickers; folder watcher auto-import |
| Config tip | Reminds when disk recording looks disabled |
| Lap table | Session type grouping; sectors; `paceEligible` (official time + both `_OK` flags + near-full coverage) |
| Compare | Two-lap traces, running time delta, and a **corner table** (time lost per corner split into entry/exit, brake-point and full-throttle deltas in metres, minimum speeds) via `compare_laps`. Clicking a corner row expands the **corner detail**. Throttle / brake charts shade where TC and ABS intervened. |
| Re-import | Session header / sidebar buttons re-analyze IBTs still on disk with the latest pipeline (`reimport_session_cmd`) |
| Fuel / tire panels | From stored lap aggregates |
| Insights strip | Deterministic client-side bullets from your laps |

Phantom reset buckets and sticky duplicate lap times are cleaned in the analysis pipeline (and when loading older sessions). See [ANALYSIS.md](ANALYSIS.md).

### Corner detail and driver aids

Clicking a corner row expands a detail panel:

- **Technique** — for candidate and reference: ABS time, TC time, peak brake, trail-braking time, coasting time, and apex-to-full-throttle time (definitions in [ANALYSIS.md](ANALYSIS.md#corner-technique)).
- **Brake-point consistency** — a scatter of every complete, non-pit lap in the reference's sub-session: brake point relative to the reference (metres) against time through the corner. Candidate and reference are highlighted, and the header shows the spread. It answers whether braking later actually gained time here. Laps that lost over 3 s (spins, offs) are hidden and counted.

The Compare throttle chart shades where **traction control** held throttle below the driver's pedal, and the brake chart shades where **ABS** was active, in each lap's color. Hovering a shaded stretch names the aid in the tooltip. ABS needs a session imported under schema v6 and TC under v5; re-import older sessions for assist data. Cars without driver aids show zero.

## Live

| Capability | Notes |
|------------|--------|
| Live monitor | Shared-memory telemetry snapshot + status |
| Leaderboard | Positions, best/last, gaps |
| Coach preview | Last coach message + widget preview |
| Audio coach | Rule engine priorities; every word is a human recording from the active voice pack (no TTS) |
| Test Coach | One-shot lap callout (lap number, lap time, delta) with the active pack; names any missing clips |
| Demo clock | Synthetic session clock / offline exercise |
| Native VR HUD | OpenXR API layer + shared memory (default `vrMode: native`) |
| Web HUD | HTTP server `:17342` for browser preview |
| Monitor overlays | Always-on-top transparent windows per enabled widget |
| Layer install / diagnostics | Registry stage, DLL presence, producer write age |

Overlay layout settings configure a **shared widget catalog** (coach / standings / relative / radar / track map). Enable once; place twice (`desktop*` for monitor windows, `vr*` for the headset). The Live page shows an in-app coach preview; the same slot config drives monitor, native VR, and the web HUD.

## Settings

Persisted via `get_settings` / `save_settings_cmd` (full write) or `patch_settings_cmd` (merge top-level keys). The **Settings** page covers VR HUD mode, per-widget VR placement, recenter bindings (keyboard / wheel button), and the audio coach: voice pack (picker with Spotter / Engineer progress, clone, zip import / export, WAV-folder import, linked folders), volume, pause between calls, low-fuel threshold, chatter level, fuel-call margin, radio beep, and every callout category. The Live page keeps quick toggles for common audio categories, shows the active pack, and has monitor overlay and VR actions (including Recenter and coach VR size/opacity/height sliders).

## Voice Studio

Records and checks voice packs for the coach (see [VOICE_PACKS.md](VOICE_PACKS.md)).

| Capability | Notes |
|------------|--------|
| Record | Phrase checklist (missing / Spotter / Engineer filters), teleprompter, hold Space to record, auto-advance, mic picker and level meter |
| Take clean-up | Resample, high-pass, trim, noise gate, loudness normalize; clipping / quiet / length warnings; one-level undo per phrase |
| Preview | Preset callouts, a composer for any lap time / gap / fuel / incident value, and a Spotter playlist with pause / skip |
| Packs | New, clone, use for coach; the bundled pack is read-only |

## Track map

Circuit outline derived from IBT GPS, shown in Analyze (pedal zones / racing lines),
the monitor overlay slot, Live preview, and the VR/OpenKneeboard `trackmap` layout.
