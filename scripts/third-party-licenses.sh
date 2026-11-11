#!/usr/bin/env bash
# Generates the third-party notices (M6-03):
#   THIRD-PARTY-LICENSES.md          full list with license texts (shipped in every package)
#   crates/app/assets/third-party.tsv compact list shown in the About dialog
# Needs cargo-about (`cargo install --locked cargo-about --features cli`).
#
# ./scripts/third-party-licenses.sh          regenerate both files
# ./scripts/third-party-licenses.sh --check  fail if they are not current or
#                                            miss a crate of `cargo tree`
set -euo pipefail
cd "$(dirname "$0")/.."

if ! command -v cargo-about >/dev/null 2>&1; then
    echo "cargo-about is not installed: cargo install --locked cargo-about --features cli" >&2
    exit 1
fi

check=false
[[ "${1:-}" == "--check" ]] && check=true

out_dir=$(mktemp -d)
trap 'rm -rf "$out_dir"' EXIT

cargo about generate --locked scripts/third-party/notices.md.hbs -o "$out_dir/notices.raw.md"
# Some license files use CRLF; the repository stores LF.
tr -d '\r' < "$out_dir/notices.raw.md" > "$out_dir/notices.md"
cargo about generate --locked scripts/third-party/list.tsv.hbs -o "$out_dir/list.tsv"

# Bundled assets that are not crates.
{
    printf '\n## Bundled assets\n\n'
    printf '### Lucide icons (ISC License)\n\n'
    printf 'SVG icons under `crates/app/assets/icons/`, https://lucide.dev\n\n'
    printf '```text\n'
    cat crates/app/assets/icons/LICENSE
    printf '```\n\n'
    printf '### Iced icon font\n\n'
    printf 'The icon font embedded by iced (`Iced-Icons.ttf`) is part of the iced crates and covered by their MIT license above.\n'
    printf '\n### .gitignore templates (CC0-1.0)\n\n'
    printf 'Templates under `crates/git-core/assets/gitignore/` (C++, Go, Java, JetBrains, macOS, Node, Python, Rust, VisualStudio, Windows), https://github.com/github/gitignore\n\n'
    printf '```text\n'
    cat crates/git-core/assets/gitignore/LICENSE
    printf '```\n'
    printf '\n### Syntax definitions and color themes\n\n'
    printf 'Bundled by the two-face crate (from bat) for syntax highlighting; generated into `crates/app/assets/syntax-acknowledgements.md` by a unit test.\n\n'
    cat crates/app/assets/syntax-acknowledgements.md
} >> "$out_dir/notices.md"
printf 'Lucide icons\t-\tISC\n' >> "$out_dir/list.tsv"
printf 'gitignore templates\t-\tCC0-1.0\n' >> "$out_dir/list.tsv"
printf 'Syntax definitions and themes (bat)\t-\tvarious, see notices\n' >> "$out_dir/list.tsv"
# Drop blank lines of the template.
grep -v '^$' "$out_dir/list.tsv" > "$out_dir/list.clean.tsv"

if $check; then
    status=0
    if ! diff -q "$out_dir/notices.md" THIRD-PARTY-LICENSES.md >/dev/null; then
        echo "THIRD-PARTY-LICENSES.md is out of date: run ./scripts/third-party-licenses.sh" >&2
        status=1
    fi
    if ! diff -q "$out_dir/list.clean.tsv" crates/app/assets/third-party.tsv >/dev/null; then
        echo "crates/app/assets/third-party.tsv is out of date: run ./scripts/third-party-licenses.sh" >&2
        status=1
    fi
    # Every crate the app links (host target) must be listed.
    missing=$(cargo tree --locked -p gitloom -e normal,build --prefix none --format '{p}' \
        | awk '{print $1 "\t" substr($2, 2)}' | sort -u \
        | grep -v -E '^(gitloom|git-core|git-graph)\b' \
        | while IFS=$'\t' read -r name version; do
            grep -q -P "^\Q$name\E\t\Q$version\E\t" "$out_dir/list.clean.tsv" || echo "$name $version"
        done)
    if [[ -n "$missing" ]]; then
        echo "Crates missing from the notices:" >&2
        echo "$missing" >&2
        status=1
    fi
    exit $status
fi

cp "$out_dir/notices.md" THIRD-PARTY-LICENSES.md
cp "$out_dir/list.clean.tsv" crates/app/assets/third-party.tsv
echo "Wrote THIRD-PARTY-LICENSES.md and crates/app/assets/third-party.tsv"
