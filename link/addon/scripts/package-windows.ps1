# Build the Windows release archive of Frame Audio Link. Run it on Windows, in a checkout:
#   .\scripts\package-windows.ps1
# Writes dist\frame-audio-link-<version>-windows-x86_64.zip, the same bytes as
# dist\frame-audio-link-windows-x86_64.zip (the name get-windows.ps1 downloads), and their
# lines in dist\SHA256SUMS. Lines of other archives in that file are kept.
[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"

$crate = Split-Path $PSScriptRoot -Parent
$repo = Split-Path (Split-Path $crate -Parent) -Parent
$dist = Join-Path $crate "dist"

$manifest = Get-Content -Raw (Join-Path $crate "Cargo.toml")
if ($manifest -notmatch '(?m)^version\s*=\s*"([^"]+)"') { throw "No version in Cargo.toml" }
$version = $Matches[1]

Push-Location $crate
try {
    # cargo reports progress on stderr. Windows PowerShell turns that into errors
    # when the output is redirected, so "Stop" is lifted for this one call.
    $ErrorActionPreference = "Continue"
    cargo build --release
    $ErrorActionPreference = "Stop"
    if ($LASTEXITCODE -ne 0) { throw "The build failed." }
} finally {
    Pop-Location
}

$stage = Join-Path ([IO.Path]::GetTempPath()) ("frame-audio-link-stage-" + [Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $stage | Out-Null
try {
    Copy-Item (Join-Path $crate "target\release\frame-audio-link.exe") $stage
    Copy-Item (Join-Path $crate "scripts\install-windows.ps1") (Join-Path $stage "install.ps1")
    foreach ($notice in @("LICENSE", "NOTICE")) {
        $file = Join-Path $repo $notice
        if (Test-Path $file) { Copy-Item $file $stage }
    }
    # Scripts from a downloaded archive are blocked by the default execution policy. These
    # two start the installer with the policy lifted for that one run.
    Set-Content -Path (Join-Path $stage "install.cmd") -Encoding ascii -Value @(
        '@echo off',
        'powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0install.ps1" %*',
        'echo.',
        'pause'
    )
    Set-Content -Path (Join-Path $stage "uninstall.cmd") -Encoding ascii -Value @(
        '@echo off',
        'powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0install.ps1" -Uninstall %*',
        'echo.',
        'pause'
    )
    Set-Content -Path (Join-Path $stage "README.txt") -Encoding utf8 -Value @"
Frame Audio Link $version for Windows (x86_64)

The SteamVR add-on of Frame Audio Mixer. While SteamVR runs it gives this PC four
more streaming microphones (Steam Streaming Mic B1-B4, the B outputs of the mixer on the Steam
Frame) and four outputs (Steam Streaming Out PC1-PC4) that the mixer can take in as channels.

Install or update: double-click install.cmd. Windows asks once for permission to
create the eight sound devices.
Remove: double-click uninstall.cmd.

Needs Steam with its streaming audio drivers (Steam installs them the first time
Remote Play or wireless VR streaming is used) and Frame Audio Mixer 2.2 or newer
on the headset.

The devices are further instances of Steam's own two audio drivers. To use other
virtual cables instead, run install.cmd -NoDevices and name them in
%APPDATA%\frame-audio-link\config.json (mic_devices, output_devices).
"@

    New-Item -ItemType Directory -Force -Path $dist | Out-Null
    $versioned = "frame-audio-link-$version-windows-x86_64.zip"
    $latest = "frame-audio-link-windows-x86_64.zip"
    $archive = Join-Path $dist $versioned
    if (Test-Path $archive) { Remove-Item $archive }
    Compress-Archive -Path (Join-Path $stage "*") -DestinationPath $archive -CompressionLevel Optimal
    Copy-Item $archive (Join-Path $dist $latest) -Force

    # SHA256SUMS in the format of `sha256sum`: lower-case hash, two spaces, name. Lines of
    # other archives (the Linux one) stay.
    $hash = (Get-FileHash -Algorithm SHA256 $archive).Hash.ToLowerInvariant()
    $sums = Join-Path $dist "SHA256SUMS"
    $kept = @()
    if (Test-Path $sums) {
        $kept = @(Get-Content $sums | Where-Object {
                $file = ($_ -split '\s+', 2)[1]
                $_ -and $file -and (@($versioned, $latest) -notcontains $file.TrimStart('*'))
            })
    }
    $lines = $kept + @("$hash  $versioned", "$hash  $latest")
    # LF endings and no BOM, so `sha256sum -c` on Linux reads the same file.
    [IO.File]::WriteAllText($sums, (($lines -join "`n") + "`n"), (New-Object Text.UTF8Encoding $false))
    Write-Host "Wrote $archive"
    Write-Host "      $hash"
} finally {
    Remove-Item -LiteralPath $stage -Recurse -Force
}
