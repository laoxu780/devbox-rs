# Build a portable Windows folder + zip for DevBox.
# Run in "x64 Native Tools Command Prompt for VS" (MSVC) so windres is available.
$ErrorActionPreference = "Stop"
Set-Location (Split-Path -Parent $PSScriptRoot)

$target = "x86_64-pc-windows-msvc"
rustup target add $target 2>$null | Out-Null

cargo build --release --target $target

$out = "dist/DevBox-windows"
Remove-Item -Recurse -Force $out -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $out | Out-Null

Copy-Item "target/$target/release/devbox.exe" "$out/devbox.exe"
Copy-Item "resources/AppIcon.ico" "$out/DevBox.ico"
Copy-Item "README.md" "$out/README.md" -ErrorAction SilentlyContinue

Compress-Archive -Force -Path "$out/*" -DestinationPath "dist/DevBox-windows.zip"
Write-Host "built $out and dist/DevBox-windows.zip"
