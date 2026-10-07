# Features

Race Refinery exposes features via `src/features/registry.ts`: **Analyze**, **Live**, **Voice Studio**, and **Settings**.

## Analyze

| Capability | Notes |
|------------|--------|
| Session browser | Lists imported IBTs; search by track/car/date; sort by date/car/track; hides no-pace-eligible sessions by default (checkbox to show them); shows a color-coded session-type letter (R/Q/P/T) per session; delete per session |
| Import | File / folder pickers; folder watcher auto-import |
| Config tip | Reminds when disk recording looks disabled |
| Workspace | Compare + track map as the hero row; lap picker below (all laps by default; optional Pace-only filter); Fuel/tires collapsed |
| Lap table | Shows all loaded laps by default; session type grouping; sectors; `paceEligible`; optional tire columns; traffic tags |
| Compare | Distance-aligned traces with **trace toggles** (Δ, speed, pedals, gear, RPM, clutch, steering, lat/long G, yaw). Corner table shows each lap’s absolute time through the corner plus Δ vs reference (+ = slower); expand a row for technique. Brake chart shades ABS from the sim channel. |
| Re-import | Session header / sidebar buttons re-analyze IBTs still on disk with the latest pipeline (`reimport_session_cmd`) |
| Fuel / tires | Fuel use, tire temps, and pressures (secondary section) |
| Insights | Session insights strip (stdev, sector gap, fuel vs median) |

Phantom reset buckets and sticky duplicate lap times are cleaned in the analysis pipeline (and when loading older sessions). See [ANALYSIS.md](ANALYSIS.md).

### Corner detail and driver aids

Clicking a corner row expands a detail panel:

- **Technique** — for candidate and reference: ABS time, peak brake, trail-braking time, coasting time, and apex-to-full-throttle time (definitions in [ANALYSIS.md](ANALYSIS.md#corner-technique)).
- **Brake-point consistency** — a scatter of every complete, non-pit lap in the reference's sub-session: brake point relative to the reference (metres) against time through the corner. Candidate and reference are highlighted, and the header shows the spread. It shows how brake point and corner time vary together across clean laps, not that a later brake caused the time. Laps that lost over 3 s (spins, offs) are hidden and counted.

The Compare brake chart shades where **ABS** was active (`BrakeABSactive`), in each lap's color. Hovering a shaded stretch names it in the tooltip. ABS needs a session imported under schema v6; re-import older sessions for assist data. Cars without ABS show zero.

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

Circuit outline derived from IBT GPS as a quiet ribbon. Pedals mode colors it from driver pedals; Lines mode draws both GPS paths with mild lateral exaggeration so on-track position reads, and Analyze selects Lines when both laps have GPS. A start/finish line crosses the track at lap distance 0. Also used in the monitor overlay slot, Live preview, and the VR/OpenKneeboard `trackmap` layout.
