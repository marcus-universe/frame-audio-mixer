# One-line install of Frame Audio Link on Windows. In PowerShell:
#   irm https://frame-audio-mixer.marcus-universe.com/get-windows | iex
# The website serves a copy of this file at /get-windows (site/get-windows). Keep both the same.
# The release also carries it as get-windows.ps1.
#
# Uninstall (add -Purge to also delete the settings):
#   iex "& { $(irm https://frame-audio-mixer.marcus-universe.com/get-windows) } -Uninstall"
#
# Downloads the latest Windows archive and SHA256SUMS, checks the archive against it, and
# runs the installer inside (install.ps1, with -Uninstall and -Purge passed on). Nothing is
# installed when the check fails.
# FRAME_AUDIO_LINK_RELEASES overrides where the two files come from, for testing.
[CmdletBinding()]
param(
    [switch]$Uninstall,
    [switch]$Purge
)

$ErrorActionPreference = "Stop"

$base = if ($env:FRAME_AUDIO_LINK_RELEASES) { $env:FRAME_AUDIO_LINK_RELEASES.TrimEnd("/") } else { "https://github.com/marcus-universe/frame-audio-mixer/releases/latest/download" }
$name = "frame-audio-link-windows-x86_64.zip"
$work = Join-Path ([IO.Path]::GetTempPath()) ("frame-audio-link-get-" + [Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $work | Out-Null
try {
    $archive = Join-Path $work $name
    $sums = Join-Path $work "SHA256SUMS"
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
    Write-Host "Downloading $base/$name"
    Invoke-WebRequest -UseBasicParsing -Uri "$base/$name" -OutFile $archive
    Invoke-WebRequest -UseBasicParsing -Uri "$base/SHA256SUMS" -OutFile $sums

    $expected = Get-Content $sums | ForEach-Object {
        if ($_ -match '^([0-9a-fA-F]{64})\s+\*?(.+)$' -and $Matches[2] -eq $name) { $Matches[1].ToLowerInvariant() }
    } | Select-Object -First 1
    if (-not $expected) { throw "SHA256SUMS has no line for $name." }
    $actual = (Get-FileHash -Algorithm SHA256 $archive).Hash.ToLowerInvariant()
    if ($actual -ne $expected) { throw "The download does not match its checksum. Nothing was installed." }

    $unpacked = Join-Path $work "unpacked"
    Expand-Archive -LiteralPath $archive -DestinationPath $unpacked
    $options = @{}
    if ($Uninstall) { $options.Uninstall = $true }
    if ($Purge) { $options.Purge = $true }
    & (Join-Path $unpacked "install.ps1") @options
} finally {
    Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
}
