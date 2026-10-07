# Voice packs

A voice pack is the set of human recordings the [audio coach](AUDIO_COACH.md) speaks with. Race Refinery has no text-to-speech: every flag call, spotter line, and lap time is a clip someone recorded. You record packs in the app's **Voice Studio** and share them as a zip or a folder.

---

## Tiers

The phrase registry ([`phrases.txt`](../crates/race-refinery-audio/src/phrases.txt)) splits clips into two tiers. Settings, Live, and Voice Studio show progress for each.

| Tier | Clips | What it unlocks |
|------|-------|-----------------|
| **Spotter** | 38 stand-alone lines: session intro, flags, traffic (car left / right, three wide, clear), gap trends, race clock, pits open, incident, hot tyres, invalid lap, stock fuel calls | Every safety and traffic call. Record this first. |
| **Engineer** | Numbers `n0`–`n20` plus `n30`…`n90`, glue words (`point`, `minute`, `seconds`, `faster`, `liters`, `laps`, …), and pace / gap / position intros | Lap and sector times, deltas, gaps, positions, fuel counts, incident counts |

A pack with only the Spotter tier is fully usable: callouts that need Engineer clips are skipped, never read out by a robot voice. Numbers 21–99 are spoken as tens + ones (29 = "twenty" + "nine") and 100+ digit by digit, so the 28 number clips (`n0`–`n20` and `n30`…`n90`) cover every value.

---

## Recording in Voice Studio

Open **Voice Studio** from the sidebar, or with **Open Voice Studio** in Settings.

1. **Pick or create a pack.** The bundled pack is read-only; use **New pack** for an empty one or **Clone** to start from an existing pack.
2. **Choose the microphone** and use **Check level** to watch the meter while you speak. Aim for the meter to peak well below the end of the bar; it turns red when the input clips.
3. **Record.** The teleprompter shows the prompt for the selected phrase. Hold **Space** (or hold the record button), say the line, and release. The **Missing** filter lists only what the pack still needs; with **Auto-advance** on, a clean take moves to the next phrase.
4. **Review.** **P** plays the clip, **Undo** restores the previous take (one level per phrase), **Delete** removes the clip, and the arrow keys move through the list.

Each take is cleaned before it is saved: resampled to 22,050 Hz mono, high-passed at 80 Hz to remove rumble, trimmed to 40 ms of lead-in and 90 ms of tail, noise-gated between words, normalized to a consistent loudness (peak ceiling about −1 dBFS), and faded at the edges. A take with no detectable speech is rejected. The app warns when the input clipped, was very quiet, or the take is far longer or shorter than the phrase should be.

**Tips:** record in one sitting with the same mic distance, speak at radio pace, and say numbers and glue words flat (no rising "list" intonation) so they chain naturally into lap times.

### Preview

The **Preview** tab plays composed callouts with the current pack, so you hear what the coach will say on track:

- **Presets** — a lap time, a sector, a gap, a position change, fuel, an incident count, two-digit numbers (29 and 45), and a flag
- **Composer** — fill in any of lap and lap time, sector and sector time, delta, position, gaps ahead / behind, fuel liters and laps, incidents and limit, or a bare number, and play exactly that
- **Spotter playlist** — plays every recorded Spotter line in order (pause, resume, skip, stop) for a quick level and timing check

Each preview lists what was said and chips for any missing clips; clicking a chip jumps to that phrase in the Record tab.

---

## Using a pack

In **Settings → Audio coach**, the **Voice pack** picker selects the pack the coach speaks with (`audioCoachPackId`), and **Test** plays a sample lap callout and names any clips it had to skip. The Live page's audio card shows the active pack and its tier progress. Changes apply on the next callout, with no coach restart.

| Action | What it does |
|--------|--------------|
| **Clone…** | Copies the selected pack into a new user pack you can record over |
| **Import zip** | Creates a new user pack from a shared zip and selects it |
| **Import WAVs** | Adds a folder of `{key}.wav` files to the selected user pack (files named after phrase keys; others are reported and skipped) |
| **Use folder…** | Links a pack folder in place (for example a synced or shared folder); the app reads it directly and records into it |
| **Export zip** | Writes the selected pack as a zip to share |
| **Delete pack / Unlink folder** | Deletes a user pack and its recordings, or forgets a linked folder (its files are left alone) |

If the selected pack disappears (folder moved, pack deleted), the coach falls back to the bundled pack and logs a warning.

---

## Pack format

A pack is a folder:

```text
my-voice/
  meta.json       { "name", "author", "language", "license", "formatVersion": 1 }
  manifest.json   { "lap": "lap.wav", "n12": "n12.wav", ... }
  lap.wav
  n12.wav
  ...
  radio_beep.wav  (optional; replaces the built-in beep)
  .undo/          (Voice Studio's one-level take backups; not exported)
```

- Clips are 16-bit PCM WAV, mono, 22,050 Hz. Imports accept any PCM WAV and convert it.
- `meta.json` and `manifest.json` are optional when reading: a folder of `{key}.wav` files loads as a pack, named after the folder.
- Keys are the ones in `phrases.txt`; unknown keys are ignored. Renaming a key in the registry orphans that clip in every existing pack.
- A zip holds the same files, at the root or inside one top-level folder. Import rejects absolute or `..` paths, more than 2,000 entries, and files over 50 MB.

### Where packs live

| Kind | Location | Editable |
|------|----------|----------|
| Bundled | `src-tauri/resources/audio/coach/default/` (installed with the app) | No |
| User | `%LOCALAPPDATA%\race-refinery\voice-packs\<id>\` | Yes |
| Folder | Any folder you link; stored as an absolute path in `audioCoachPackFolders` | Yes |

The bundled pack is the maintainer's recorded voice and covers every phrase in the registry (a unit test fails if a key is missing from it).

The bundled folder and a user pack such as "New default" are **separate copies**. Re-recording in Voice Studio only changes the user pack. To refresh the default that ships with the app:

1. Record in a user pack (Voice Studio can write there).
2. Copy that pack's `{key}.wav` files and `manifest.json` into `src-tauri/resources/audio/coach/default/`. Leave out `.undo/` (take backups). Keep the bundled `meta.json` so the name and license stay "Race Refinery default".
3. Commit the changed files. Until they are committed and built, only this checkout hears the new takes.

---

## Commands

| Command | Purpose |
|---------|---------|
| `list_voice_phrases` | The phrase registry (key, prompt, tier, category) |
| `list_voice_packs` / `get_voice_pack_status` | Packs with tier counts, missing keys, read-only flag |
| `create_voice_pack` / `clone_voice_pack` / `delete_voice_pack` | Manage user packs |
| `set_active_voice_pack` | Set `audioCoachPackId` and persist settings |
| `import_voice_pack_zip` / `export_voice_pack_zip` | Zip sharing (native file dialogs) |
| `link_voice_pack_folder` / `unlink_voice_pack_folder` | Folder packs |
| `import_voice_pack_wavs` | Add a folder of `{key}.wav` files to a pack |
| `list_input_devices` / `start_voice_capture` / `get_voice_capture_level` / `cancel_voice_capture` / `finish_voice_take` | Recording |
| `undo_voice_take` / `delete_voice_clip` | Edit a pack's clips |
| `list_voice_presets` / `play_voice_clip` / `play_voice_preset` / `play_voice_composition` / `play_spotter_playlist` / `get_voice_preview_status` / `control_voice_preview` | Preview |

Frontend wrappers live in `src/shared/api.ts`; see [API.md](API.md).
