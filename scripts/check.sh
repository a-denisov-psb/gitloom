#!/usr/bin/env bash
# Local quality gate. Must pass before every commit (same steps as CI).
set -euo pipefail
cd "$(dirname "$0")/.."

echo "==> fmt";    cargo fmt --all --check
echo "==> clippy"; cargo clippy --workspace --all-targets --locked -- -D warnings
echo "==> test";   cargo test --workspace --locked
echo "All checks passed."
