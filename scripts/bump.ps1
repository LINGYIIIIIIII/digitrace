# Digitrace version bump script (ASCII-only for PS 5.1 compatibility)
# Usage: powershell -ExecutionPolicy Bypass -File scripts/bump.ps1 2.23.0
# Syncs version in src-tauri/Cargo.toml ([package]) and src-tauri/tauri.conf.json,
# then reminds you to update CHANGELOG.md.
param(
    [Parameter(Mandatory = $true)]
    [string]$Version
)

if ($Version -notmatch '^\d+\.\d+\.\d+$') {
    Write-Error "Version must be x.y.z (e.g. 2.23.0)"
    exit 1
}

$root = Split-Path -Parent $PSScriptRoot
$cargoFile = Join-Path $root 'src-tauri\Cargo.toml'
$confFile = Join-Path $root 'src-tauri\tauri.conf.json'

# 1) Cargo.toml: patch the FIRST `version = "x.y.z"` line only ([package],
#    not [workspace.package]).
$lines = Get-Content $cargoFile -Encoding UTF8
$changed = $false
for ($i = 0; $i -lt $lines.Count; $i++) {
    if ($lines[$i] -match '^version = "\d+\.\d+\.\d+"$') {
        $lines[$i] = "version = `"$Version`""
        $changed = $true
        break
    }
}
if (-not $changed) {
    Write-Error "No [package] version line found in $cargoFile"
    exit 1
}
# UTF-8 无 BOM 写入（File.WriteAllLines 默认即 UTF-8 无 BOM，无需显式编码对象——
# PS 5.1 下 New-Object UTF8Encoding($false) 作为实参会解析失败导致写入静默跳过）
[System.IO.File]::WriteAllLines($cargoFile, $lines)

# 2) tauri.conf.json: replace the "version" field via regex (avoids JSON
#    re-serialization which would escape non-ASCII chars).
$conf = Get-Content $confFile -Raw -Encoding UTF8
if ($conf -notmatch '"version"\s*:\s*"\d+\.\d+\.\d+"') {
    Write-Error "No version field found in $confFile"
    exit 1
}
$conf = $conf -replace '("version"\s*:\s*)"\d+\.\d+\.\d+"', "`$1`"$Version`""
[System.IO.File]::WriteAllText($confFile, $conf)

Write-Host "OK: bumped to v$Version in:"
Write-Host "  - $cargoFile"
Write-Host "  - $confFile"
Write-Host "Reminder: add a v$Version entry at the top of CHANGELOG.md if needed."
