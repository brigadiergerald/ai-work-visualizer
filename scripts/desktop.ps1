param([switch]$Build)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $projectRoot
if (!(Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw 'Rust (MSVC) is required. Install the Tauri prerequisites described in README.md.'
}
if ($Build) { & npm.cmd run tauri -- build --no-bundle } else { & npm.cmd run tauri -- dev }
exit $LASTEXITCODE
