# VR Native In-Headset HUD — Spike & Decision (**historical**)

> **Historical document.** Race Refinery’s in-headset HUD now ships via its own OpenXR
> API layer (`vrMode: native`). Browser preview remains at `:17342`. For current
> setup and architecture, use **[NATIVE_VR.md](NATIVE_VR.md)** only.
>
> The body below is research context (why a portable overlay extension was not
> viable and why an implicit API layer was chosen). It is not product guidance.

**Phase:** v3 research spike (historical)  
**Last updated:** September 2026 (marked historical)

## Goal

Determine whether Race Refinery can render its HUD *inside the headset* as a
self-contained feature that draws a panel over the iRacing OpenXR image in VR.

Today Race Refinery serves its VR HUD from a local HTTP server
([`src-tauri/src/vr/hud_server.rs`](../src-tauri/src/vr/hud_server.rs)) at
`http://127.0.0.1:17342/vr`, which the user adds as a Web Dashboard inside
OpenKneeboard. OpenKneeboard does the actual in-headset compositing. The
question for this spike is whether we can cut OpenKneeboard out of the loop.

## Runtime reality (researched)

There are only two mechanisms for a second process to draw over a running
OpenXR game, and only one of them is viable in practice.

### 1. `XR_EXTX_overlay` — not viable

`XR_EXTX_overlay` is the OpenXR extension explicitly designed for "overlay"
applications that composite on top of another app. It is the clean, intended
solution, but it is a dead end on consumer hardware:

- It is **provisional / experimental** and has been effectively abandoned for
  ~3 years (it is not even rebased onto OpenXR 1.0 core semantics).
- **SteamVR and Oculus/Meta runtimes do not implement it.** Requesting it at
  `xrCreateInstance` fails with `no support found for requested extension`.
- Only **Monado** (the open-source Linux runtime) implements it — not a runtime
  any iRacing VR user runs.

Sources:
- OpenXR Runtime Extension Support Report: https://github.khronos.org/OpenXR-Inventory/runtime_extension_support.html
- Khronos forum, "Overlay app with OpenXR and Cpp": https://community.khronos.org/t/overlay-app-with-openxr-and-cpp/110533
- SteamVR OpenXR `XR_EXTX_overlay` request thread: https://steamcommunity.com/app/250820/discussions/8/2448217320142811491/

**Conclusion:** we cannot rely on `XR_EXTX_overlay`.

### 2. OpenXR API layer — viable, but a separate native module

This is how OpenKneeboard, fpsVR-style tools, and projects like
[DesktopXR](https://github.com/glenimp617/DesktopXR) actually work. An OpenXR
**API layer** is a native DLL inserted between the game and the runtime by the
OpenXR loader. It shims `xrEndFrame` and appends an `XrCompositionLayerQuad`
(backed by its own swapchain) to the frame's layer list before forwarding the
call down the chain.

Reference implementation pattern (Ybalrid/OpenXR-API-Layer-Template):

```cpp
XRAPI_ATTR XrResult XRAPI_CALL thisLayer_xrEndFrame(
    XrSession session, const XrFrameEndInfo* frameEndInfo) {
    static PFN_xrEndFrame next = GetNextLayerFunction(xrEndFrame);
    // Build a modified XrFrameEndInfo with our quad appended to `layers`,
    // then forward to the next layer / runtime.
    return next(session, &modifiedFrameEndInfo);
}
```

Key properties:

- The layer is a **standalone C++ DLL** registered with the OpenXR loader via a
  JSON manifest and a registry key
  (`HKLM/HKCU\Software\Khronos\OpenXR\1\ApiLayers\Implicit`). It is **not** code
  running inside the Tauri webview or the Race Refinery main process.
- It must create its own DirectX (11/12) swapchain, render the HUD texture, and
  manage an `XrSpace` for placement. World/view-locking requires recreating the
  reference space on recenter.
- Layer **load order matters** — it must compose correctly alongside other
  installed layers (OpenKneeboard, OpenXR Toolkit, etc.).

Sources:
- OpenXR API layer template: https://github.com/Ybalrid/OpenXR-API-Layer-Template
- OpenXR 1.1 spec, §2.7 API Layers: https://registry.khronos.org/OpenXR/specs/1.1/html/xrspec.html
- `xrEndFrame` reference: https://registry.khronos.org/OpenXR/specs/1.1/man/html/xrEndFrame.html
- DesktopXR (third-party HUD as an API layer): https://github.com/glenimp617/DesktopXR

## OpenKneeboard architecture reference

OpenKneeboard is the model to copy if we ever build a native layer:

- An **OpenXR API layer** (`OpenKneeboard-OpenXR.dll`) hooks `xrEndFrame` and
  injects one or more quad layers.
- The layer reads the panel image from a **shared resource** — a shared D3D
  texture plus a shared-memory control block — produced by the separate
  OpenKneeboard app process. This decouples "what to draw" (app) from "how to
  composite it in VR" (layer).
- Placement, opacity, and gaze/zoom behavior are driven through that shared
  control block.

For Race Refinery, the equivalent would be: Race Refinery main process writes the current
[`LiveSnapshot`](../src-tauri/src/live/snapshot.rs) (or a pre-rendered HUD
texture) into shared memory; the Race Refinery OpenXR layer DLL reads it each frame
and composites a quad.

## iRacing OpenXR runtimes to support

A native layer would have to be validated against each runtime an iRacing VR
user might run, because behavior varies by vendor:

| Runtime | Notes |
|---------|-------|
| Oculus/Meta (Quest Link / Air Link) | Most common; Meta OpenXR runtime |
| SteamVR OpenXR | Index, Vive, many headsets via SteamVR |
| Virtual Desktop (VDXR) | Popular wireless Quest path; own OpenXR runtime |
| Pimax | Own runtime |
| Windows Mixed Reality | Deprecated by Microsoft; declining but still in use |

Each is a separate compositor with its own quirks around swapchain formats,
layer flags, and recenter behavior — hence the per-runtime testing burden.

## Go / No-go criteria

**Go** only if all of these hold:

1. A static hardcoded quad (e.g. "Race Refinery" text) renders correctly in iRacing VR
   on at least the two most common runtimes (Meta + SteamVR OpenXR).
2. The layer composes cleanly when OpenKneeboard / OpenXR Toolkit are also
   installed (no black screen, no crash, sane load order).
3. Frame-time overhead is negligible (no measurable FPS loss in iRacing).
4. Shared-memory `LiveSnapshot` → quad pipeline works at 30+ Hz.

**No-go** if the spike shows per-runtime breakage, anti-cheat/loader friction,
or maintenance cost that outweighs the marginal benefit over OpenKneeboard.

## Decision: No-go (for now)

We are **keeping OpenKneeboard as the official VR HUD path** and **not** building
a native OpenXR API layer at this time. Rationale:

1. **Effort vs. benefit.** A native layer is a separate C++ DLL with its own
   build/sign/install story (loader registry registration, MSI integration) and
   per-runtime validation. OpenKneeboard already solves all of this and is
   widely used in the iRacing community today.
2. **No portable extension.** `XR_EXTX_overlay` is unusable on consumer
   runtimes, so the only path is the same API-layer hooking OpenKneeboard
   already does — we would be reimplementing OpenKneeboard, not leapfrogging it.
3. **Maintenance & risk.** Hooking `xrEndFrame` across Meta, SteamVR, VDXR, and
   Pimax is an ongoing compatibility commitment. A bug here can black-screen a
   user's headset mid-session.
4. **We lose nothing today.** The HTTP HUD + OpenKneeboard already delivers an
   in-headset panel without SteamVR (OpenKneeboard is OpenXR-native).

This matches the explicit fallback in the v3 roadmap: "If spike fails or too
costly: keep OpenKneeboard path as official VR HUD."

## Incremental improvement we *will* keep

Rather than a native layer, make the OpenKneeboard path more self-explanatory:

- Race Refinery already exposes the HUD URL and setup steps in the Live panel.
- The HUD server (`hud_server.rs`) is the integration surface; the setup
  checklist should surface only when relevant (server up, OpenKneeboard not yet
  pointed at the URL). This is a far cheaper win than a native compositor.

## If we ever revisit (POC plan)

Time-box to 2–3 focused sessions:

1. New **separate crate** `openxr-layer/` (C++ or Rust via `openxr` + raw FFI),
   **not** inside the Tauri process. Start from
   Ybalrid/OpenXR-API-Layer-Template.
2. Register an implicit API layer; shim `xrEndFrame`; append a hardcoded
   `XrCompositionLayerQuad`. Success = static panel visible in iRacing VR.
3. Add a shared-memory channel and pipe `LiveSnapshot` from Race Refinery into the
   layer; render real telemetry.
4. Re-evaluate against the Go/No-go criteria above before investing further.

## References

- OpenXR 1.1 Specification (§2.7 API Layers): https://registry.khronos.org/OpenXR/specs/1.1/html/xrspec.html
- `xrEndFrame` man page: https://registry.khronos.org/OpenXR/specs/1.1/man/html/xrEndFrame.html
- OpenXR Runtime Extension Support Report: https://github.khronos.org/OpenXR-Inventory/runtime_extension_support.html
- OpenXR API Layer Template (Ybalrid): https://github.com/Ybalrid/OpenXR-API-Layer-Template
- DesktopXR (overlay as API layer): https://github.com/glenimp617/DesktopXR
- OpenKneeboard: https://openkneeboard.com/
- Khronos forum — overlay apps in OpenXR: https://community.khronos.org/t/overlay-app-with-openxr-and-cpp/110533
- Monado `XR_EXTX_overlay`: https://www.collabora.com/news-and-blog/news-and-events/monado-multi-application-support-with-xr-extx-overlay.html
