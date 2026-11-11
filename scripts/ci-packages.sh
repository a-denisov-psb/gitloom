#!/usr/bin/env bash
# Runs the release workflow as a dry run on a ref, waits for it and downloads
# all packages into dist/ci-<short-sha>/ (ADR-052). Used by `just ci-packages`.
#   ./scripts/ci-packages.sh [ref]    default: develop
# Needs the GitHub CLI, logged in (`gh auth login`). Windows: scripts/ci-packages.ps1.
set -euo pipefail
cd "$(dirname "$0")/.."
ref="${1:-develop}"

if ! command -v gh >/dev/null 2>&1; then
    echo "Needs the GitHub CLI: https://cli.github.com (then: gh auth login)" >&2
    exit 1
fi
if ! gh auth status >/dev/null 2>&1; then
    echo "The GitHub CLI is not logged in: gh auth login" >&2
    exit 1
fi

started=$(date -u +%Y-%m-%dT%H:%M:%SZ)
gh workflow run release.yml --ref "$ref"
echo "Waiting for the run to appear …"
id=""
for _ in $(seq 1 30); do
    sleep 2
    id=$(gh run list --workflow release.yml --event workflow_dispatch --branch "$ref" \
        --limit 5 --json databaseId,createdAt \
        --jq "[.[] | select(.createdAt >= \"$started\")][0].databaseId // empty")
    [[ -n "$id" ]] && break
done
if [[ -z "$id" ]]; then
    echo "The workflow run did not show up; see: gh run list --workflow release.yml" >&2
    exit 1
fi

status=0
gh run watch "$id" --exit-status || status=$?
sha=$(gh run view "$id" --json headSha --jq '.headSha[0:7]')
out="dist/ci-$sha"
rm -rf "$out" && mkdir -p "$out"
# Best-effort jobs may have failed: download what the run produced.
gh run download "$id" --dir "$out" || true
# One folder per artifact; flatten them.
find "$out" -mindepth 2 -type f -exec mv -f {} "$out/" \;
find "$out" -mindepth 1 -type d -empty -delete
ls -l "$out"
if [[ $status -ne 0 ]]; then
    echo "The run failed (see: gh run view $id); the files above are what it produced." >&2
fi
exit "$status"
