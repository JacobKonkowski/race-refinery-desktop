# Setup

## Prerequisites

1. **Rust** 1.89+ (`rustup`)
2. **Node.js** 18+
3. **iRacing** with disk + memory telemetry enabled
4. Optional: an OpenXR VR runtime for the in-headset HUD

Analyze Insights are computed in the app from imported laps (no external AI service).

## First install

```powershell
git clone https://github.com/JacobKonkowski/race-refinery-desktop.git
cd race-refinery-desktop
.\setup.ps1          # or .\setup.ps1 -SkipBuild
npm run tauri dev
```

Build the OpenXR layer when you want the native in-headset HUD (see [NATIVE_VR.md](NATIVE_VR.md)).

### Upgrading from PitWall

Race Refinery was previously named PitWall. The rename is a clean break: nothing is migrated.

- Settings, the session database, and the track map cache start fresh in `%LOCALAPPDATA%\race-refinery\`. Reimport IBTs; the old `%LOCALAPPDATA%\pitwall-desktop\` folder can be deleted.
- Click **Install VR layer** once on the Live page. It removes the old PitWall layer registration and registers the Race Refinery layer.
- An existing checkout keeps working after the GitHub repo rename (GitHub redirects), but you can update the remote: `git remote set-url origin https://github.com/JacobKonkowski/race-refinery-desktop.git`.

## iRacing `app.ini`

Edit `Documents\iRacing\app.ini` (or OneDrive Documents equivalent):

```ini
irsdkEnableMem=1
irsdkEnableDisk=1
```

Restart iRacing after changing. Record with **Alt+L** → `Documents\iRacing\telemetry\*.ibt`.

## Coach voice

The coach speaks only human recordings from a **voice pack**; there is no text-to-speech and nothing to download. The bundled pack under `src-tauri/resources/audio/coach/default/` is a complete recorded voice and is used out of the box. To use a different voice:

1. Open **Voice Studio** (sidebar), create a pack, and record the **Spotter** tier first (flags, traffic, race clock). Add the **Engineer** tier (numbers and glue words) for lap times, gaps, and fuel.
2. Or import a shared pack in **Settings → Audio coach → Import zip**.
3. Select the pack in **Settings → Audio coach → Voice pack**.

Test Coach plays a sample lap callout with the active pack and volume, and lists any clips it skipped. See [VOICE_PACKS.md](VOICE_PACKS.md).

If you have an older checkout, the gitignored `src-tauri/resources/audio/coach/piper/` folder (about 78 MB) is no longer used and can be deleted.

---

## Race tonight checklist

1. **app.ini** — `irsdkEnableMem=1` and `irsdkEnableDisk=1`, then restart iRacing.
2. **Import** — Confirm Analyze can see sessions (auto-watcher or Import). Reimport after schema upgrades.
3. **Test Coach** — Live tab → Test Coach. You should hear a radio-style lap callout in your pack's voice; a toast names any clips the pack has not recorded.
4. **Demo clock** — Start Demo Clock on Live to exercise UI / SHM test pattern without a session.
5. **HUD preview** — Open HUD preview → browser at `http://127.0.0.1:17342/vr`.
6. **Other OpenXR layers** — Disable any other OpenXR API layers that composite overlays. Only one layer stack should own compositing while you test Race Refinery.
7. **Install Race Refinery layer** — Live → Install VR layer (stages DLL under AppData + registry). Restart iRacing in **OpenXR** VR.
8. **Test pattern** — Start HUD (native). With no live track data, Race Refinery publishes a VR test pattern so the coach quad should appear.
9. **Drive** — Start live monitor + audio coach; confirm write age stays low in Diagnostics and pack/flag calls work with clips.

If the headset stays blank: layer ready + DLL present, OpenXR (not OpenVR), no conflicting API layers, restart sim after install. See [TROUBLESHOOTING.md](TROUBLESHOOTING.md).

---

## Daily workflow

| Goal | Where |
|------|--------|
| Review last race | Analyze → pick session → Insights / Compare / Fuel |
| Practice with coach | Live → Start live + Start audio |
| Headset HUD | Live → Install layer once → Start HUD |
| Offline UI check | Live → Demo clock / Test Coach / HUD preview |
| Record or check the coach voice | Voice Studio → Record / Preview |
