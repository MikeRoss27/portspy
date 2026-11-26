# PortSpy release build — single self-contained portspy.exe (x86_64-pc-windows-msvc).
# Usage: .\scripts\release.ps1
$ErrorActionPreference = "Stop"

$Target = "x86_64-pc-windows-msvc"
$Output = Join-Path $PSScriptRoot "..\target\$Target\release\portspy.exe"
$Dist = Join-Path $PSScriptRoot "..\dist"
$DistBin = Join-Path $Dist "portspy.exe"

Write-Host "Building PortSpy release ($Target)..." -ForegroundColor Cyan
cargo build --release --target $Target
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

if (-not (Test-Path $Output)) {
    Write-Host "ERROR: binary not found at $Output" -ForegroundColor Red
    exit 1
}

New-Item -ItemType Directory -Force -Path $Dist | Out-Null
Copy-Item $Output $DistBin -Force

$Hash = (Get-FileHash $DistBin -Algorithm SHA256).Hash
$Size = (Get-Item $DistBin).Length

Write-Host "" -ForegroundColor Cyan
Write-Host "Release ready:" -ForegroundColor Green
Write-Host "  $DistBin"
Write-Host "  size:  $([math]::Round($Size / 1MB, 1)) MB"
Write-Host "  sha256: $Hash"
Write-Host ""
Write-Host "Smoke test:" -ForegroundColor Cyan
& $DistBin --version
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
