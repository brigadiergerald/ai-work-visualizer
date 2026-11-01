# Disposable GitHub-hosted runner only. Never run on a user's Windows profile.
$ErrorActionPreference = 'Stop'
if ($env:OS -ne 'Windows_NT' -or $env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted') {
    throw 'Installer smoke test is restricted to disposable GitHub-hosted Windows runners.'
}
$projectRoot = Split-Path -Parent $PSScriptRoot
$bundleDir = Join-Path $projectRoot 'src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis'
$packages = @(Get-ChildItem -LiteralPath $bundleDir -Filter '*-setup.exe' -File)
if ($packages.Count -ne 1) { throw 'Expected exactly one validation installer.' }
$installRoot = Join-Path $env:RUNNER_TEMP ('aiwv-install-' + [guid]::NewGuid())
$profileRoot = Join-Path $env:LOCALAPPDATA 'local.ai-guild.desktop'
if (Test-Path -LiteralPath $profileRoot) { throw 'Profile already exists; refusing to touch it.' }
# Synthetic sentinels only. No real DB, account, credentials or external services.
New-Item -ItemType Directory -Path (Join-Path $profileRoot 'skins'), (Join-Path $profileRoot 'presentation') | Out-Null
[IO.File]::WriteAllText((Join-Path $profileRoot 'installer-sentinel.txt'), 'synthetic profile: preserve')
[IO.File]::WriteAllText((Join-Path $profileRoot 'skins/installer-sentinel.txt'), 'synthetic skin: preserve')
[IO.File]::WriteAllText((Join-Path $profileRoot 'presentation/installer-sentinel.txt'), 'synthetic selection: preserve')
function ProfileHashes {
    @(Get-ChildItem -LiteralPath $profileRoot -File -Recurse | Sort-Object FullName | ForEach-Object {
        $_.FullName.Substring($profileRoot.Length) + ':' + (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash
    }) -join "`n"
}
$before = ProfileHashes
function AssertProfile {
    if ((ProfileHashes) -cne $before) { throw 'Installer changed the synthetic profile.' }
}
function Install {
    # NSIS /D must be last and is intentionally unquoted, including spaces.
    $process = Start-Process -FilePath $packages[0].FullName -ArgumentList "/S /D=$installRoot" -Wait -PassThru
    if ($process.ExitCode -ne 0) { throw "Installer failed: $($process.ExitCode)" }
    if (!(Test-Path -LiteralPath (Join-Path $installRoot 'ai-guild.exe'))) { throw 'Installed executable missing.' }
    AssertProfile
}
Install
foreach ($name in @('LICENSE','NOTICE','LICENSING.md','CHARACTER_ASSETS_LICENSE.md','THIRD_PARTY_NOTICES.md','README.ja.md')) {
    $sourceHash = (Get-FileHash -LiteralPath (Join-Path $projectRoot $name) -Algorithm SHA256).Hash
    $installedHash = (Get-FileHash -LiteralPath (Join-Path $installRoot "licenses/$name") -Algorithm SHA256).Hash
    if ($sourceHash -cne $installedHash) { throw "Installed document differs: $name" }
}
if ((Get-AuthenticodeSignature -LiteralPath (Join-Path $installRoot 'ai-guild.exe')).Status -ne 'NotSigned') {
    throw 'Installed application must be unsigned for this validation.'
}
Install
$uninstaller = Join-Path $installRoot 'uninstall.exe'
if (!(Test-Path -LiteralPath $uninstaller)) { throw 'NSIS uninstaller missing.' }
$process = Start-Process -FilePath $uninstaller -ArgumentList '/S' -Wait -PassThru
if ($process.ExitCode -ne 0) { throw "Uninstaller failed: $($process.ExitCode)" }
if (Test-Path -LiteralPath (Join-Path $installRoot 'ai-guild.exe')) { throw 'Application remains after uninstall.' }
AssertProfile
Write-Host 'PASS: silent install, six license document hashes, unsigned app, same-version reinstall, uninstall, synthetic profile preservation.'
Write-Host 'NOT VERIFIED: GUI launch, WebView2, real DB upgrade, purchased skins, Codex or external services.'
