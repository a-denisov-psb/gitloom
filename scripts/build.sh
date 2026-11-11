#!/usr/bin/env bash
# Builds the bare binary for one target with the `dist` profile and copies it
# to dist/<bin>-<version>-<alias>[.exe] (ADR-052). Used by `just build`.
#   ./scripts/build.sh <alias>     e.g. win-x64, linux-x64
#   ./scripts/build.sh --targets   table of aliases and what this machine can build
# Windows: scripts/build.ps1.
set -euo pipefail
cd "$(dirname "$0")/.."

aliases=(win-x64 win-x86 win-arm64 linux-x64 linux-arm64 mac)

# Name-dependent values come from git_core::identity (ADR-028).
bin=$(sed -n 's/^pub const BINARY_NAME: &str = "\(.*\)";/\1/p' crates/git-core/src/identity.rs)
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
host_os=$(uname -s)
host_arch=$(uname -m)
[[ "$host_arch" == arm64 ]] && host_arch=aarch64

# Sets `triple`, `ext`, `tool` (cargo subcommand) and `problem` (empty when
# the target can be built here). Returns 1 for an unknown alias.
resolve() {
    triple="" ext="" tool=build problem=""
    case "$1" in
        win-x64 | win-x86)
            local arch=x86_64
            [[ "$1" == win-x86 ]] && arch=i686
            triple="$arch-pc-windows-gnu" ext=.exe
            if [[ "$host_os" != Linux ]]; then
                problem="build it on Windows or Linux/WSL, or use: just ci-packages"
            elif ! command -v "$arch-w64-mingw32-gcc" >/dev/null 2>&1 ||
                ! command -v "$arch-w64-mingw32-windres" >/dev/null 2>&1; then
                problem="needs mingw-w64: sudo apt install mingw-w64"
            fi
            ;;
        win-arm64)
            triple=aarch64-pc-windows-msvc ext=.exe
            problem="needs the MSVC toolchain; build it on Windows or use: just ci-packages"
            ;;
        linux-x64 | linux-arm64)
            local arch=x86_64
            [[ "$1" == linux-arm64 ]] && arch=aarch64
            triple="$arch-unknown-linux-gnu"
            if [[ "$host_os" != Linux ]]; then
                problem="build it on Linux/WSL or use: just ci-packages"
            elif [[ "$arch" != "$host_arch" ]]; then
                if command -v cargo-zigbuild >/dev/null 2>&1 && command -v zig >/dev/null 2>&1; then
                    tool=zigbuild
                else
                    problem="needs cargo-zigbuild and zig: cargo install --locked cargo-zigbuild; pip3 install ziglang (or zig from https://ziglang.org/download/)"
                fi
            fi
            ;;
        mac)
            if [[ "$host_os" == Darwin ]]; then
                triple="$host_arch-apple-darwin"
            else
                triple=universal-apple-darwin
                problem="needs macOS; use: just ci-packages"
            fi
            ;;
        *) return 1 ;;
    esac
    if [[ -z "$problem" ]] && command -v rustup >/dev/null 2>&1 &&
        ! rustup target list --installed | grep -qx "$triple"; then
        problem="needs the Rust target: rustup target add $triple"
    fi
    return 0
}

if [[ "${1:-}" == --targets ]]; then
    printf '%-12s %-26s %s\n' Alias "Rust target" "On this machine"
    for alias in "${aliases[@]}"; do
        resolve "$alias"
        status=${problem:-yes}
        [[ -z "$problem" && "$tool" == zigbuild ]] && status="yes (cargo zigbuild)"
        printf '%-12s %-26s %s\n' "$alias" "$triple" "$status"
    done
    exit 0
fi

alias="${1:-}"
if ! resolve "$alias"; then
    echo "Unknown target '$alias'. Valid targets: ${aliases[*]}" >&2
    exit 1
fi
if [[ -n "$problem" ]]; then
    echo "$alias ($triple): $problem" >&2
    exit 1
fi

cargo "$tool" --profile dist --locked -p "$bin" --target "$triple"
mkdir -p dist
out="dist/$bin-$version-$alias$ext"
cp "target/$triple/dist/$bin$ext" "$out"
size=$(wc -c < "$out")
echo "$out ($(awk -v s="$size" 'BEGIN { printf "%.1f MB", s / 1048576 }'))"
