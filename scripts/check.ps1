# PortSpy quality gates — runs every check before a PR.
# Usage: .\scripts\check.ps1
$ErrorActionPreference = "Stop"

function Step {
    param([string]$Name, [string]$Command)
    Write-Host "`n=== $Name ===" -ForegroundColor Cyan
    Invoke-Expression $Command
    if ($LASTEXITCODE -ne 0) {
        Write-Host "FAILED: $Name" -ForegroundColor Red
        exit $LASTEXITCODE
    }
    Write-Host "OK: $Name" -ForegroundColor Green
}

Write-Host "PortSpy quality gates" -ForegroundColor Yellow

Step "fmt"      "cargo fmt --all -- --check"
Step "check"    "cargo check --workspace"
Step "clippy"   "cargo clippy --workspace --all-targets -- -D warnings"
Step "test"     "cargo test --workspace"

if (Get-Command cargo-deny -ErrorAction SilentlyContinue) {
    Step "deny" "cargo deny check"
} else {
    Write-Host "cargo-deny not installed, skipping (optional supply-chain check)" -ForegroundColor DarkYellow
}

Write-Host "`nAll quality gates passed." -ForegroundColor Green
