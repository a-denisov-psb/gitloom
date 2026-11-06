#!/usr/bin/env bash
# Local quality gate. Must pass before every commit (same steps as CI).
set -euo pipefail
cd "$(dirname "$0")/.."

echo "==> fmt";    cargo fmt --all --check
echo "==> clippy"; cargo clippy --workspace --all-targets --locked -- -D warnings
echo "==> test";   cargo test --workspace --locked
if command -v cargo-deny >/dev/null 2>&1; then
    echo "==> deny"; cargo deny --locked check --hide-inclusion-graph
else
    echo "==> deny skipped: cargo install --locked cargo-deny"
fi
if command -v cargo-about >/dev/null 2>&1; then
    echo "==> third-party notices"; ./scripts/third-party-licenses.sh --check
else
    echo "==> third-party notices skipped: cargo install --locked cargo-about --features cli"
fi
echo "All checks passed."
