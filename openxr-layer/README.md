# race-refinery-openxr-layer

A standalone Windows OpenXR **API layer** that composites the Race Refinery HUD inside
the headset while iRacing runs in OpenXR. It is built outside the Tauri/Cargo
tree because the OpenXR loader injects it into the **iRacing** process, not into
Race Refinery.

```
iRacing (OpenXR app)
   -> race-refinery-openxr-layer.dll   (hooks xrEndFrame, appends quad layers)
   -> Meta / SteamVR / VDXR runtime
```

The DLL reads a shared-memory block (`Local\RaceRefineryVR`, see
[`include/race_refinery_vr_shm.h`](include/race_refinery_vr_shm.h)) that the Race Refinery desktop
process writes from the live `LiveSnapshot`, then draws each enabled overlay with
Direct2D/DirectWrite and appends it as an `XrCompositionLayerQuad`.

## Build

Requires CMake 3.22+, a C++17 MSVC toolchain, and the Windows SDK (D3D11, D2D1,
DirectWrite). The OpenXR SDK headers are fetched automatically.

```powershell
cmake -S . -B build -A x64
cmake --build build --config Release
```

Output: `build/Release/race-refinery-openxr-layer.dll` and a copy of
`race_refinery_openxr_layer.json` beside it.

## Install (developer / manual)

Register the layer as an implicit API layer for the current user:

```powershell
reg add "HKCU\Software\Khronos\OpenXR\1\ApiLayers\Implicit" `
  /v "<full-path>\race_refinery_openxr_layer.json" /t REG_DWORD /d 0 /f
```

A value of `0` means enabled. Race Refinery performs this registration through the
`install_vr_layer` command and the MSI installer; the manual command is for
local layer development. Set `RACE_REFINERY_VR_DISABLE=1` to bypass the layer without
unregistering it.

## Phase A POC (go/no-go gate)

The first milestone is a **static quad** in iRacing VR on Meta Quest Link:

1. Build and register the layer.
2. Temporarily hardcode one overlay (`enabled = 1`, `kind = COACH`, a fixed pose
   ~1.2 m forward) and skip the SHM read, or run Race Refinery so the block exists.
3. Launch iRacing in OpenXR mode on Quest Link and confirm:
   - the Race Refinery panel is visible and stable,
   - no black screen with iRacing alone,
   - no measurable FPS loss.

If the static quad does not render on Quest Link, stop and document — do not
invest further in the rendering pipeline. See
[`../docs/NATIVE_VR.md`](../docs/NATIVE_VR.md) and
[`../docs/VR_NATIVE_SPIKE.md`](../docs/VR_NATIVE_SPIKE.md).

## Files

| File | Role |
|------|------|
| `include/race_refinery_vr_shm.h` | Shared-memory contract (mirrored by `src-tauri/src/vr/shm.rs`) |
| `src/layer.cpp` | Loader negotiation, dispatch, `xrEndFrame` quad injection |
| `src/shm_reader.h` | Seqlock reader for the producer's block |
| `src/hud_renderer.{h,cpp}` | Direct2D/DirectWrite overlay drawing |
| `manifest/race_refinery_openxr_layer.json` | OpenXR API layer manifest |
