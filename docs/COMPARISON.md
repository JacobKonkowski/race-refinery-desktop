# Live field awareness

Race Refinery’s live goal is **field awareness while you drive**: where you are relative to
the cars around you, session pace markers, and pack/spotter state — fed into the Live
tab, voice coach, and in-headset HUD.

Analyze Insights stay focused on **your** IBT laps (client-side). This document covers
live SDK channels and how Race Refinery uses them.

## What live telemetry provides

| Capability | Live source | Your IBT |
|------------|-------------|----------|
| Others' best / last lap | `CarIdxBestLapTime`, `CarIdxLastLapTime` | — |
| Others' sectors / traces | Not available from the SDK for other cars | Your sectors/traces only |
| Overall + class position | `CarIdxPosition`, `CarIdxClassPosition` | — |
| Gap to leader / ahead / behind | `CarIdxF2Time` | — |
| Delta to session best / optimal (you) | `LapDeltaToSessionBestLap`, `LapDeltaToSessionOptimalLap` | Same channels when present |
| Pack / 3-wide spotter | `CarLeftRight` | — |
| Flags | `SessionFlags` | — |
| Your incident count | `PlayerCarMyIncidentCount` | Partial |

Race Refinery does not import other drivers’ IBT files; field data comes from the live
shared-memory session.

## Live field awareness

The live monitor opens a second telemetry subscription
([`CarIdxFrame`](../src-tauri/src/live/car_idx_frame.rs)) alongside the player frame.
The per-car arrays are merged with the session driver roster in
[`competitors.rs`](../src-tauri/src/live/competitors.rs) and surfaced on the
[`LiveSnapshot`](../src-tauri/src/live/snapshot.rs):

- **Leaderboard** — overall/class toggle, best/last lap, and delta to your best lap,
  shown on the Live tab ([`SessionLeaderboard.tsx`](../src/features/live/SessionLeaderboard.tsx)).
- **Session deltas** — delta to the session's best and optimal laps next to your own
  delta-to-best.
- **Gaps** — time to the car ahead and behind, derived from the difference in
  `CarIdxF2Time` (time behind the leader) between adjacent cars.
- **Pack state** — `CarLeftRight` mapped to a [`PackState`](../src-tauri/src/live/pack.rs)
  (clear, car left/right, three-wide, two cars left/right).

The same data drives the in-headset HUD (web [`hud_server.rs`](../src-tauri/src/vr/hud_server.rs)
and native SHM), which adds a position line, gap ahead/behind, the field delta, and a
compact pack indicator.

## Audio coach

Every callout is a **human recording** from the active voice pack: fixed phrases are one clip, and lap times, gaps, and positions are composed from recorded number clips.
Full pipeline, priority order, and session modes: **[AUDIO_COACH.md](AUDIO_COACH.md)**; recording and sharing packs: **[VOICE_PACKS.md](VOICE_PACKS.md)**.

The coach ([`audio/engine`](../src-tauri/src/audio/engine/)) speaks at most one alert per 250 ms tick. Lower-priority alerts wait for the next tick rather than being dropped.

| Priority | Category | Examples |
|----------|----------|----------|
| 1 (highest) | Critical | Red, checkered, black |
| 2 | Safety | Yellow (incl. waving), green, blue, incidents |
| 3 | Pack | Car left/right, three-wide (4 s cooldown) |
| 4 | Race | Fuel-to-finish, low fuel, pit-this-lap |
| 5 | Pace | Sector and lap callouts |
| 6 | Strategy | Race clock, pits open, position |

Pack, race, pace, gap, and strategy alerts are suppressed on pit road or off track; flags and incidents still announce. Toggle categories and **chatter level** in Live settings (`audioGapAlertsEnabled`, `audioPaceEnabled`, `audioRaceClockEnabled`, etc. — see [DATA_MODEL.md](DATA_MODEL.md)).

### Notes on `CarIdxF2Time` and the blue flag

`CarIdxF2Time` and the per-car flag semantics are interpreted conservatively. Gaps are
computed as the absolute difference in F2 time between adjacent cars in the overall
order, and the blue flag is read from the player's `SessionFlags` bitfield. Both should
be sanity-checked against a live or replay session; if iRacing reports them differently
than assumed, adjust the resolver in `competitors.rs` / `audio/engine` without changing
the surrounding feature.

## Multi-class notes

Positions are tracked both overall and within class. The live leaderboard offers an
overall/class toggle; in a multi-class field the class view filters to your class and
sorts by class position. Class color (from the session roster) is shown as the chip
background next to each car number.
