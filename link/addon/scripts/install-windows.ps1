# Install or update Frame Audio Link, the SteamVR add-on of Frame Audio Mixer, on a Windows PC.
# Run it as your normal user, from a release archive (as install.ps1) or from a git checkout
# (a checkout is built first, which needs Rust):
#   .\install.ps1                      install or update, create the sound devices, register with SteamVR
#   .\install.ps1 -NoDevices           the same without the sound devices (for other virtual cables)
#   .\install.ps1 -Uninstall           remove the sound devices, the registration and the program
#   .\install.ps1 -Uninstall -Purge    also delete its settings
#
# The program goes to %LOCALAPPDATA%\Programs\FrameAudioLink. Creating or removing the sound
# devices is the one step that needs administrator rights; Windows asks for it once.
# The devices are further instances of the two audio drivers Steam already installed
# (Steam Streaming Microphone and Steam Streaming Speakers). No driver is added or changed.
[CmdletBinding()]
param(
    [switch]$Uninstall,
    [switch]$Purge,
    [switch]$NoDevices
)

$ErrorActionPreference = "Stop"

$target = Join-Path $env:LOCALAPPDATA "Programs\FrameAudioLink"
$exe = Join-Path $target "frame-audio-link.exe"
$settings = Join-Path $env:APPDATA "frame-audio-link"

if ($Purge -and -not $Uninstall) {
    throw "-Purge only works together with -Uninstall."
}

# The program has no console of its own (SteamVR starts it in the background), so it is
# started in this one and waited for.
function Invoke-Link {
    param([Parameter(Mandatory = $true)][string[]]$Arguments)
    $process = Start-Process -FilePath $exe -ArgumentList $Arguments -Wait -PassThru -NoNewWindow
    return $process.ExitCode
}

# Ends a running add-on, so its file can be replaced and the next VR session starts the new
# one. Only processes started from the installed program are touched.
function Stop-Link {
    Get-Process -Name "frame-audio-link" -ErrorAction SilentlyContinue |
        Where-Object { $_.Path -eq $exe } |
        ForEach-Object {
            $_ | Stop-Process -Force
            $_ | Wait-Process -Timeout 5 -ErrorAction SilentlyContinue
        }
}

if ($Uninstall) {
    if (Test-Path $exe) {
        Stop-Link
        # Removes the SteamVR registration and the sound devices. A failure must not keep
        # the files around.
        if ((Invoke-Link @("uninstall")) -ne 0) {
            Write-Warning "The SteamVR registration or the sound devices could not be removed completely. See the messages above."
        }
        Remove-Item -LiteralPath $target -Recurse -Force
    }
    if ($Purge -and (Test-Path $settings)) {
        Remove-Item -LiteralPath $settings -Recurse -Force
    }
    Write-Host "Frame Audio Link is removed."
    if (-not $Purge) {
        Write-Host "Its settings stay in $settings."
    }
    return
}

# The program to install: next to this script in a release archive, or built in a checkout.
$source = Join-Path $PSScriptRoot "frame-audio-link.exe"
if (-not (Test-Path $source)) {
    $crate = Split-Path $PSScriptRoot -Parent
    if (-not (Test-Path (Join-Path $crate "Cargo.toml"))) {
        throw "frame-audio-link.exe is not next to this script, and this is not a checkout."
    }
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        throw "Building from a checkout needs Rust (https://rustup.rs). Or download the Windows archive from the releases page."
    }
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
    $source = Join-Path $crate "target\release\frame-audio-link.exe"
}

Stop-Link
New-Item -ItemType Directory -Force -Path $target | Out-Null
Copy-Item -LiteralPath $source -Destination $exe -Force
Write-Host "Installed $exe"

$code = if ($NoDevices) { Invoke-Link @("register") } else { Invoke-Link @("install") }
if ($code -ne 0) {
    Write-Warning "The installation did not finish. See the messages above; running this script again is safe."
    exit 1
}

Write-Host ""
Write-Host "Next:"
Write-Host "  1. Start SteamVR. Frame Audio Link starts with it; its switch is under"
Write-Host "     Settings > Startup / Shutdown > Choose Startup Overlay Apps."
Write-Host "  2. In Windows, the microphones are 'Steam Streaming Mic B1' to 'B4' and the outputs"
Write-Host "     'Steam Streaming Out PC1' to 'PC4'."
Write-Host "  3. In Frame Audio Mixer on the headset, route channels to B1-B4, and add"
Write-Host "     PC1-PC4 as channels with the + button."
