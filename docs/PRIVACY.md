# Privacy

Race Refinery is designed to run **entirely on your machine**.

## What we do not do

- No analytics, crash phone-home, or third-party telemetry SDKs
- No accounts, cloud sync, or remote coaching servers
- No uploading of IBT files or live session data by the app itself

## What stays local

| Data | Location |
|------|----------|
| Imported sessions / laps | SQLite under `%LOCALAPPDATA%\race-refinery\` |
| Settings | JSON beside the database |
| Coach voice packs | Bundled pack in app resources; packs you record or import under `%LOCALAPPDATA%\race-refinery\voice-packs\`; linked folders stay where you put them |
| Microphone audio | Captured only while you hold record in Voice Studio; the cleaned take is saved to the pack and nothing is sent anywhere |
| OpenXR layer install | Per-user registry + staged DLL under AppData |
| Logs | Process stdout/stderr when you enable `RUST_LOG` |

## Optional MCP (Cursor) on localhost

Settings includes an **MCP** toggle that is **off by default**. When enabled, Race Refinery
binds a read-only MCP HTTP endpoint on `127.0.0.1` only (shown in Settings when listening).
Nothing listens while the toggle is off.

- Tools expose local session / lap / compare / technique / consistency data already in RR
- No shared secret: access control is the Settings toggle plus the localhost bind
- The app does **not** upload telemetry for MCP; any AI host you connect (e.g. Cursor) may
  see tool results under that host's own settings — that is outside Race Refinery

## What may appear in logs

When logging is enabled (`RUST_LOG=…`), messages may include:

- File paths under your iRacing telemetry folder
- Track/car names and session identifiers from the sim
- Error strings from import, live connect, VR layer install, or voice pack load / record
- Voice pack folder paths and missing clip keys (`voice pack '…' has no clip for <key>`)

Do not share raw logs publicly if they contain paths or driver names you want private.

## Imports

IBT import is limited to paths you pick via the file dialog or files under the
configured telemetry directory (see import path policy in FOUNDATION / SETUP).
