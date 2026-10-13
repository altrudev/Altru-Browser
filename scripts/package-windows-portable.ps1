# Windows portable developer preview — no installer or administrator permissions
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $root
cargo test --locked --features desktop-preview --bin altru-browser
if ($LASTEXITCODE -ne 0) { throw 'Desktop regression tests failed' }
cargo build --locked --release --features desktop-preview --bin altru-browser
if ($LASTEXITCODE -ne 0) { throw 'Windows executable build failed' }
$exe = Join-Path $root 'target\release\altru-browser.exe'
if (!(Test-Path $exe)) { throw 'Windows executable missing' }
$dist = Join-Path $root 'dist\Altru-Browser-Windows-Portable'
New-Item -ItemType Directory -Force -Path $dist | Out-Null
Copy-Item $exe (Join-Path $dist 'Altru-Browser.exe') -Force
@'
Altru Browser — Windows portable developer preview
Double-click Altru-Browser.exe. No installer required.
This is experimental and is not a certified production web browser.
If Windows SmartScreen flags the unsigned developer build, do not bypass
your organization's security policy. Obtain a signed build instead.
'@ | Set-Content (Join-Path $dist 'README.txt')
$hash = (Get-FileHash (Join-Path $dist 'Altru-Browser.exe') -Algorithm SHA256).Hash
"$hash  Altru-Browser.exe" | Set-Content (Join-Path $dist 'SHA256SUMS.txt')
$zip = Join-Path $root 'dist\Altru-Browser-Windows-Portable.zip'
if (Test-Path $zip) { Remove-Item $zip }
Compress-Archive -Path $dist -DestinationPath $zip
Write-Host "WINDOWS_PORTABLE_PACKAGE=$zip"
Write-Host "WINDOWS_PORTABLE_SHA256=$hash"
