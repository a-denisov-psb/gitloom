#!/usr/bin/env bash
# Builds the release packages of the local platform into dist/ (M6-08, ADR-031).
#   Linux:  <Name>-<version>-x86_64.AppImage, <name>-<version>-linux-x86_64.tar.gz
#   macOS:  <Name>-<version>-universal.dmg (arm64 + x86_64 via lipo)
# Windows: scripts/package.ps1.
# Needs cargo-packager (`cargo install --locked cargo-packager`); Linux also
# needs `file`; macOS also
# needs both Rust targets (`rustup target add aarch64-apple-darwin x86_64-apple-darwin`).
set -euo pipefail
cd "$(dirname "$0")/.."
root=$(pwd)

if ! command -v cargo-packager >/dev/null 2>&1; then
    echo "cargo-packager is not installed: cargo install --locked cargo-packager" >&2
    exit 1
fi

# Name-dependent values come from git_core::identity (ADR-028).
identity=crates/git-core/src/identity.rs
const() { sed -n "s/^pub const $1: &str = \"\(.*\)\";/\1/p" "$identity"; }
name=$(const DISPLAY_NAME)
bin=$(const BINARY_NAME)
app_id=$(const APP_ID)
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
dist="$root/dist"
mkdir -p "$dist"

package_linux() {
    local target=x86_64-unknown-linux-gnu
    # linuxdeploy and its plugins are AppImages themselves: run them without
    # FUSE (containers, WSL). appimagetool needs `file` on PATH.
    export APPIMAGE_EXTRACT_AND_RUN=1
    cargo build --release --locked -p "$bin" --target "$target"
    local bins="$root/target/$target/release"

    # AppImage (cargo-packager downloads linuxdeploy on first use).
    local work="$root/target/packager/linux"
    rm -rf "$work" && mkdir -p "$work"
    cargo packager -p "$bin" --release --target "$target" -f appimage \
        --binaries-dir "$bins" --out-dir "$work"
    local appimage
    appimage=$(find "$work" -maxdepth 1 -name '*.AppImage' | head -n 1)
    cp "$appimage" "$dist/$name-$version-x86_64.AppImage"

    # Plain archive: binary, licenses, desktop entry, metainfo and icon.
    local stage="$work/$bin-$version"
    mkdir -p "$stage"
    cp "$bins/$bin" LICENSE THIRD-PARTY-LICENSES.md README.md "$stage/"
    cp "packaging/linux/$app_id.desktop" "packaging/linux/$app_id.metainfo.xml" "$stage/"
    cp crates/app/assets/app-icon/icon-256.png "$stage/$app_id.png"
    tar -C "$work" -czf "$dist/$bin-$version-linux-x86_64.tar.gz" "$bin-$version"
}

package_macos() {
    local arm=aarch64-apple-darwin intel=x86_64-apple-darwin
    cargo build --release --locked -p "$bin" --target "$arm"
    cargo build --release --locked -p "$bin" --target "$intel"
    local universal="$root/target/universal-apple-darwin/release"
    mkdir -p "$universal"
    lipo -create -output "$universal/$bin" \
        "$root/target/$arm/release/$bin" "$root/target/$intel/release/$bin"
    local work="$root/target/packager/macos"
    rm -rf "$work" && mkdir -p "$work"
    cargo packager -p "$bin" --release --target universal-apple-darwin -f app \
        --binaries-dir "$universal" --out-dir "$work"
    # Ad-hoc signature (ADR-016): no Developer ID, not notarized.
    codesign --force --deep --sign - "$work/$name.app"
    codesign --verify --deep --strict "$work/$name.app"
    cargo packager -p "$bin" --release --target universal-apple-darwin -f dmg \
        --binaries-dir "$universal" --out-dir "$work"
    local dmg
    dmg=$(find "$work" -maxdepth 1 -name '*.dmg' | head -n 1)
    cp "$dmg" "$dist/$name-$version-universal.dmg"
}

case "$(uname -s)" in
    Linux) package_linux ;;
    Darwin) package_macos ;;
    *) echo "Unsupported platform; on Windows use scripts/package.ps1" >&2; exit 1 ;;
esac

ls -l "$dist"
