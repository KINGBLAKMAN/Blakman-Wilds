# Tests, optimised release build, and a double-clickable private package: dist\<Name>\ (+ zip).
# Includes everything the build needs to run on this PC: the exe (with the generated data table
# compiled in) and any baked assets.
# Usage: powershell -ExecutionPolicy Bypass -File package.ps1 [-Name MyRewrite]
param([string]$Name = "Rewrite")
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

cargo test --workspace --release
if ($LASTEXITCODE -ne 0) { throw "tests failed" }
cargo build --release -p rewrite-app
if ($LASTEXITCODE -ne 0) { throw "release build failed" }

$out = Join-Path $PSScriptRoot "dist\$Name"
if (Test-Path $out) { Remove-Item -Recurse -Force $out }
New-Item -ItemType Directory -Force $out | Out-Null
Copy-Item "target\release\rewrite-app.exe" "$out\$Name.exe"
if (Test-Path "assets") { Copy-Item -Recurse "assets" "$out\assets" }
Copy-Item "README.md" "$out\README.md"

$zip = Join-Path $PSScriptRoot "dist\$Name.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
Compress-Archive -Path $out -DestinationPath $zip
"Packaged $out\$Name.exe"
"Zip: $zip"
