# Build local API reference (rustdoc + TypeDoc). Output is gitignored under docs/.api-out/.
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$OutDir = Join-Path $Root "docs\.api-out"
$RustOut = Join-Path $OutDir "rust"
$TsOut = Join-Path $OutDir "ts"

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

Write-Host "Building Rust API docs (race_refinery_desktop_lib)..."
$TauriDir = Join-Path $Root "src-tauri"
$TargetDir = Join-Path $TauriDir "target"
$env:CARGO_TARGET_DIR = $TargetDir
Push-Location $TauriDir
try {
    cargo doc --no-deps --lib
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
} finally {
    Pop-Location
    Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
}

$DocSrc = Join-Path $TargetDir "doc"
if (-not (Test-Path $DocSrc)) {
    Write-Error "Expected rustdoc output at $DocSrc"
}
if (Test-Path $RustOut) { Remove-Item -Recurse -Force $RustOut }
New-Item -ItemType Directory -Force -Path $RustOut | Out-Null
Copy-Item -Recurse -Force (Join-Path $DocSrc "*") $RustOut

Write-Host "Building TypeScript API docs..."
Push-Location $Root
try {
    npx typedoc
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
} finally {
    Pop-Location
}

$RustIndex = Join-Path $RustOut "race_refinery_desktop_lib\index.html"
$TsIndex = Join-Path $TsOut "index.html"

Write-Host ""
Write-Host "API docs ready:"
Write-Host "  Rust:       $RustIndex"
Write-Host "  TypeScript: $TsIndex"
Write-Host ""
Write-Host "Open in browser:"
Write-Host "  start `"$RustIndex`""
Write-Host "  start `"$TsIndex`""
