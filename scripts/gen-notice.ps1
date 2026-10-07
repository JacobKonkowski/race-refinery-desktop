# Generate a dependency inventory for NOTICE (best-effort).
$ErrorActionPreference = "Continue"
$out = Join-Path $PSScriptRoot "..\NOTICE.generated.md"
$lines = @()
$lines += "# Generated dependency inventory"
$lines += ""
$lines += "## Cargo (race-refinery-desktop)"
$lines += '```'
$lines += (cargo tree -p race-refinery-desktop --prefix none 2>$null | Sort-Object -Unique)
$lines += '```'
$lines += ""
$lines += "## npm"
$lines += '```'
Push-Location (Join-Path $PSScriptRoot "..")
$lines += (npm ls --all --parseable 2>$null | ForEach-Object { $_ -replace [regex]::Escape((Get-Location).Path + '\'), '' })
Pop-Location
$lines += '```'
$lines | Set-Content -Path $out -Encoding utf8
Write-Host "Wrote $out — merge relevant lines into NOTICE before release."
