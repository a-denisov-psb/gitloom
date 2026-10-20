# Local quality gate for Windows. Must pass before every commit (same steps as CI).
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

function Step($name, $cmd) {
    Write-Host "==> $name"
    & cmd /c $cmd
    if ($LASTEXITCODE -ne 0) { throw "$name failed" }
}

Step "fmt"    "cargo fmt --all --check"
Step "clippy" "cargo clippy --workspace --all-targets --locked -- -D warnings"
Step "test"   "cargo test --workspace --locked"
Write-Host "All checks passed."
