# Bundled OpenXR layer staging

`tauri build` bundles everything in this folder under
`<app resources>/resources/openxr-layer/`, where Race Refinery's `install_vr_layer`
command registers the manifest with the OpenXR loader.

Before packaging a release, build the layer and stage its artifacts here:

```powershell
cmake -S openxr-layer -B openxr-layer/build -A x64
cmake --build openxr-layer/build --config Release
copy openxr-layer\build\Release\race-refinery-openxr-layer.dll  src-tauri\resources\openxr-layer\
```

`race_refinery_openxr_layer.json` is already checked in beside this README. The DLL is
gitignored and must be copied after each layer rebuild.
