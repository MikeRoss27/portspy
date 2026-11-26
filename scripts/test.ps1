# PortSpy unit tests (workspace, including Windows-only platform tests).
# Usage: .\scripts\test.ps1
$ErrorActionPreference = "Stop"

Write-Host "Running PortSpy tests (host target)..." -ForegroundColor Cyan
cargo test --workspace
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "Running PortSpy tests (x86_64-pc-windows-msvc)..." -ForegroundColor Cyan
cargo test --workspace --target x86_64-pc-windows-msvc
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "All tests passed." -ForegroundColor Green
