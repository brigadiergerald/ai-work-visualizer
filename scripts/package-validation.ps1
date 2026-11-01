# Unsigned Windows x64 validation package only; no service calls or publishing.
$ErrorActionPreference = 'Stop'
if ($env:OS -ne 'Windows_NT') { throw 'Build this package on Windows x64 with the MSVC prerequisites.' }
$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location -LiteralPath $projectRoot
try {
    & npm.cmd run tauri -- build --config src-tauri/tauri.validation.conf.json --bundles nsis --target x86_64-pc-windows-msvc -- --locked
    if ($LASTEXITCODE -ne 0) { throw "Tauri package build failed: $LASTEXITCODE" }
    $bundleDir = Join-Path $projectRoot 'src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis'
    $packages = @(Get-ChildItem -LiteralPath $bundleDir -Filter '*-setup.exe' -File)
    if ($packages.Count -ne 1) { throw 'Expected exactly one NSIS installer. Use a clean checkout or inspect stale build outputs.' }
    $package = $packages[0]
    $signature = Get-AuthenticodeSignature -LiteralPath $package.FullName
    if ($signature.Status -ne 'NotSigned') { throw "Expected unsigned validation installer, got $($signature.Status)" }
    $hash = (Get-FileHash -LiteralPath $package.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    [IO.File]::WriteAllText(($package.FullName + '.sha256'), ($hash + '  ' + $package.Name + [Environment]::NewLine))
    Write-Host "Unsigned validation installer: $($package.FullName)"
    Write-Host "SHA256: $hash"
} finally {
    Pop-Location
}
