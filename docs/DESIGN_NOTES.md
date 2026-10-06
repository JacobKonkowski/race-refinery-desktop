# Design notes

Short rationale for non-obvious decisions (ADR-lite). Each entry: context → decision → consequences.

---

## Sector 0 at 0% is ignored

**Context:** iRacing exposes a sector marker at the start/finish line that does not represent a timed sector.

**Decision:** Both live (`tracker.rs`) and post-session (`analysis/sectors.rs`) skip sector 0 crossings at 0% lap distance.

**Consequences:** S1/S2/S3 align with in-sim sector times; no spurious sub-second "sector" at lap start.

---

## Gaps use `CarIdxF2Time`, not physical distance

**Context:** SDK exposes time-behind-leader per car, not reliable on-track distance to neighbors.

**Decision:** Gap ahead/behind = difference in `CarIdxF2Time` between adjacent positions.

**Consequences:** Gaps match iRacing timing screens; not useful for spatial "meters to car ahead" in traffic.

---

## `CarLeftRight` is Int32 enum, not a bitfield

**Context:** Early code treated pack state as flags; iRacing sends discrete enum values.

**Decision:** `car_idx_frame.rs` reads Int32; `pack.rs` maps explicit variants (clear, left, right, three-wide, two-wide).

**Consequences:** Correct three-wide and two-wide callouts; no false combos from bitwise OR.

---

## One alert per poll + speech queue

**Context:** Unbounded speech would stack unintelligibly under yellow + pack + lap complete.

**Decision:** Coach picks highest priority per 250 ms tick; `SpeechQueue` plays one plan at a time.

**Consequences:** Lower priority waits; nothing is silently dropped except by chatter-level filtering.

---

## Human-recorded voice packs only, no TTS

**Context:** Earlier builds baked clips with a neural TTS voice (Piper) and synthesized free-form lines live, with Windows speech as a fallback. Short synthesized words, numbers especially, came out slurred or with a trailing mumble, and drivers described the result as robotic or drunk. Cloud voices were ruled out: the app is local-only and free to run.

**Decision:** Every word the coach says is a human recording in a **voice pack**. The app ships a Voice Studio that records, cleans, and previews packs, and packs are shared as zips or folders. Free-form lines (track name, session type, tyre compound) were dropped or rewritten as fixed phrases, and numbers are composed from a small recorded set: 0–20 and the round tens, with 21–99 as tens + ones and 100+ digit by digit. The phrase registry is split into a **Spotter** tier (stand-alone safety and traffic lines) and an **Engineer** tier (numbers and glue words), so a partly recorded pack still covers the calls that matter most. A callout with a missing clip is skipped rather than filled in by a robot voice.

**Consequences:** One natural voice with no synthesis anywhere and no model download (the old 78 MB voice is gone). The bundled pack is the maintainer's own recordings, so the app speaks out of the box; any other voice has to be recorded or imported. Prosody across chained number clips is flatter than a spoken sentence, which recording the number clips flat mitigates. Speed is whatever the speaker recorded (the old `audioCoachRate` setting is retired). Adding a callout means adding a registry key that every pack must then record.

---

## Native OpenXR layer vs pre-rendered SHM pixels

**Context:** Compositing pre-rendered browser bitmaps in SHM was explored; text clarity and widget layout suffered.

**Decision:** C++ implicit API layer hooks `xrEndFrame`, draws with Direct2D per widget slot, reads structured SHM.

**Consequences:** Crisp text in VR; requires layer install and OpenXR mode; see [NATIVE_VR.md](NATIVE_VR.md).

---

## Seqlock on `Local\RaceRefineryVR`

**Context:** Rust writer (~30 Hz) and C++ reader (per frame) share one memory block.

**Decision:** Seqlock protocol in `shm.rs` / `race_refinery_vr_shm.h` — reader retries on torn reads.

**Consequences:** No mutex in the compositor hot path; occasional retry on conflict.

---

## Pit / off-track suppression

**Context:** Pace and pack callouts in the pits are noise.

**Decision:** Mute pack, race, pace, gap, and strategy on pit road or off track; keep flags and incidents.

**Consequences:** Cleaner radio; player still hears safety-critical calls in the paddock.

---

## Related docs

- [AUDIO_COACH.md](AUDIO_COACH.md) — priority details
- [LIVE_TELEMETRY.md](LIVE_TELEMETRY.md) — sector and merge logic
- [VR_NATIVE_SPIKE.md](VR_NATIVE_SPIKE.md) — original layer research
