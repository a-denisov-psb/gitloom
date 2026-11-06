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
if (Get-Command cargo-deny -ErrorAction SilentlyContinue) {
    Step "deny" "cargo deny --locked check --hide-inclusion-graph"
} else {
    Write-Host "==> deny skipped: cargo install --locked cargo-deny"
}
# The third-party notices are checked by CI and ./scripts/check.sh (bash).
Write-Host "All checks passed."
