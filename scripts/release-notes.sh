#!/usr/bin/env bash
# Prints the CHANGELOG.md section of a version (release notes, M6-09).
# ./scripts/release-notes.sh 1.0.0-rc.1
set -euo pipefail
cd "$(dirname "$0")/.."
version="${1:?usage: release-notes.sh <version>}"
version="${version#v}"
notes=$(awk -v v="$version" '
    $0 ~ "^## \\[" v "\\]" { found = 1; next }
    found && /^## \[/ { exit }
    found { print }
' CHANGELOG.md | sed -e '/./,$!d')
if [[ -z "${notes//[[:space:]]/}" ]]; then
    echo "CHANGELOG.md has no section for $version" >&2
    exit 1
fi
printf '%s\n' "$notes"
