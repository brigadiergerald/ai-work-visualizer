param([string]$Python = 'python')
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $projectRoot
$pythonCommand = Get-Command $Python -ErrorAction Stop
$testRoot = Join-Path $projectRoot 'target/test-data'
New-Item -ItemType Directory -Path $testRoot -Force | Out-Null
$env:AI_GUILD_TEST_ROOT = $testRoot
$env:AI_GUILD_QA_PEER_PYTHON = $pythonCommand.Source
try {
    & cargo test --locked --manifest-path src-tauri/Cargo.toml -- --test-threads=1
    $testExit = $LASTEXITCODE
} finally {
    Remove-Item Env:AI_GUILD_TEST_ROOT -ErrorAction SilentlyContinue
    Remove-Item Env:AI_GUILD_QA_PEER_PYTHON -ErrorAction SilentlyContinue
}
exit $testExit
