# IPC contract sync
#
# Prefer regenerating shared TypeScript types from Rust with specta/tauri-specta
# as that lands. Until then, CI runs this drift smoke check: every #[tauri::command]
# name under src-tauri/src/commands/ should appear in src/shared/api.ts.

$ErrorActionPreference = "Stop"
$cmdFiles = Get-ChildItem -Path (Join-Path $PSScriptRoot "..\src-tauri\src\commands") -Filter *.rs
$apiFile = Join-Path $PSScriptRoot "..\src\shared\api.ts"

$commands = foreach ($file in $cmdFiles) {
  $lines = Get-Content $file.FullName
  for ($i = 0; $i -lt $lines.Count; $i++) {
    if ($lines[$i] -notmatch '#\[tauri::command\]') { continue }
    for ($j = $i + 1; $j -lt [Math]::Min($i + 4, $lines.Count); $j++) {
      if ($lines[$j] -match 'pub (async )?fn ([a-z0-9_]+)\s*[<(]') {
        $Matches[2]
        break
      }
    }
  }
}
$commands = $commands | Sort-Object -Unique

$api = Get-Content $apiFile -Raw
$missing = @()
foreach ($c in $commands) {
  # snake_case command often invoked as camelCase helper — check both
  $camel = [regex]::Replace($c, '_(.)', { param($m) $m.Groups[1].Value.ToUpper() })
  if ($api -notmatch [regex]::Escape($c) -and $api -notmatch [regex]::Escape($camel)) {
    $missing += $c
  }
}

if ($missing.Count -gt 0) {
  Write-Error ("IPC drift: commands missing from api.ts: " + ($missing -join ", "))
  exit 1
}
Write-Host ("IPC drift check OK (" + $commands.Count + " commands scanned).")
