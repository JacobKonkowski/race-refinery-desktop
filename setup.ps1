# Race Refinery — first-run setup
param(
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
$Root = $PSScriptRoot

function Write-Step($msg) {
    Write-Host "==> $msg" -ForegroundColor Cyan
}

function Write-Ok($msg) {
    Write-Host "    $msg" -ForegroundColor Green
}

function Write-Warn2($msg) {
    Write-Host "    WARNING: $msg" -ForegroundColor Yellow
}

Write-Step "Race Refinery setup"

# Rust toolchain (pitwall MSRV 1.89+)
if (Get-Command rustc -ErrorAction SilentlyContinue) {
    $ver = rustc --version
    Write-Ok "Rust: $ver"
} else {
    Write-Warn2 "Rust not found. Install from https://rustup.rs/"
    exit 1
}

# Node.js
if (Get-Command node -ErrorAction SilentlyContinue) {
    Write-Ok "Node: $(node --version)"
} else {
    Write-Warn2 "Node.js not found. Install from https://nodejs.org/"
    exit 1
}

Write-Step "Installing npm dependencies"
Set-Location $Root
npm install
Write-Ok "npm install complete"

if (-not $SkipBuild) {
    Write-Step "Building frontend"
    npm run build
    Write-Ok "Frontend build complete"

    Write-Step "Building Tauri app (debug)"
    npm run tauri build -- --debug
    Write-Ok "Tauri build complete"
}

Write-Host ""
Write-Host "iRacing prerequisites (Documents\iRacing\app.ini):" -ForegroundColor Cyan
Write-Host "  irsdkEnableMem=1"
Write-Host "  irsdkEnableDisk=1"
Write-Host ""
Write-Host "Telemetry files: Documents\iRacing\telemetry\*.ibt (record in-car with Alt+L)"
Write-Host ""
Write-Host "Setup guide:  docs\SETUP.md" -ForegroundColor Cyan
Write-Host "Run dev:      npm run tauri dev" -ForegroundColor Green
