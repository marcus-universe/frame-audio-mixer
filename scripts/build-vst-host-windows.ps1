# Builds the Windows x64 VST host that runs Windows plugins under Proton ARM64 on the Frame.
# Run on Windows with Rust and the MSVC build tools. The result goes to vst-host/prebuilt/,
# which install.sh and package.sh pick up.
$ErrorActionPreference = "Stop"
$root = Join-Path $PSScriptRoot ".."
cargo build --release --target x86_64-pc-windows-msvc --manifest-path (Join-Path $root "vst-host/Cargo.toml")
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
$out = Join-Path $root "vst-host/prebuilt"
New-Item -ItemType Directory -Force $out | Out-Null
Copy-Item (Join-Path $root "vst-host/target/x86_64-pc-windows-msvc/release/frame-vst-host.exe") $out -Force
Write-Output "Wrote $out/frame-vst-host.exe"
