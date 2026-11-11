#!/usr/bin/env bash
# Builds the release packages of the local platform into dist/ (M6-08, ADR-031).
#   Linux:  <Name>-<version>-<arch>.AppImage, <name>-<version>-linux-<arch>.tar.gz,
#           x86_64 also <name>_<version>_amd64.deb (<arch>: the machine's,
#           x86_64 or aarch64)
#   macOS:  <Name>-<version>-universal.dmg (arm64 + x86_64 via lipo)
# Windows: scripts/package.ps1.
# Builds with the `dist` profile (ADR-052).
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
    local arch
    arch=$(uname -m)
    case "$arch" in
        x86_64 | aarch64) ;;
        *) echo "Unsupported Linux architecture: $arch" >&2; exit 1 ;;
    esac
    local target="$arch-unknown-linux-gnu"
    # linuxdeploy and its plugins are AppImages themselves: run them without
    # FUSE (containers, WSL). appimagetool needs `file` on PATH.
    export APPIMAGE_EXTRACT_AND_RUN=1
    cargo build --profile dist --locked -p "$bin" --target "$target"
    local bins="$root/target/$target/dist"

    # AppImage (cargo-packager downloads linuxdeploy on first use).
    local work="$root/target/packager/linux"
    rm -rf "$work" && mkdir -p "$work"
    cargo packager -p "$bin" --profile dist --target "$target" -f appimage \
        --binaries-dir "$bins" --out-dir "$work"
    local appimage
    appimage=$(find "$work" -maxdepth 1 -name '*.AppImage' | head -n 1)
    cp "$appimage" "$dist/$name-$version-$arch.AppImage"

    # Debian package (x86_64 only, ADR-052).
    if [[ "$arch" == x86_64 ]]; then
        cargo packager -p "$bin" --profile dist --target "$target" -f deb \
            --binaries-dir "$bins" --out-dir "$work"
        local deb
        deb=$(find "$work" -maxdepth 1 -name '*.deb' | head -n 1)
        cp "$deb" "$dist/"
    fi

    # Plain archive: binary, licenses, desktop entry, metainfo and icon.
    local stage="$work/$bin-$version"
    mkdir -p "$stage"
    cp "$bins/$bin" LICENSE THIRD-PARTY-LICENSES.md README.md "$stage/"
    cp "packaging/linux/$app_id.desktop" "packaging/linux/$app_id.metainfo.xml" "$stage/"
    cp crates/app/assets/app-icon/icon-256.png "$stage/$app_id.png"
    tar -C "$work" -czf "$dist/$bin-$version-linux-$arch.tar.gz" "$bin-$version"
}

package_macos() {
    local arm=aarch64-apple-darwin intel=x86_64-apple-darwin
    cargo build --profile dist --locked -p "$bin" --target "$arm"
    cargo build --profile dist --locked -p "$bin" --target "$intel"
    local universal="$root/target/universal-apple-darwin/dist"
    mkdir -p "$universal"
    lipo -create -output "$universal/$bin" \
        "$root/target/$arm/dist/$bin" "$root/target/$intel/dist/$bin"
    local work="$root/target/packager/macos"
    rm -rf "$work" && mkdir -p "$work"
    cargo packager -p "$bin" --profile dist --target universal-apple-darwin -f app \
        --binaries-dir "$universal" --out-dir "$work"
    # Ad-hoc signature (ADR-016): no Developer ID, not notarized.
    codesign --force --deep --sign - "$work/$name.app"
    codesign --verify --deep --strict "$work/$name.app"
    # The dmg is made here, not by cargo-packager, which would bundle the
    # app again without the signature.
    local volume="$work/dmg"
    rm -rf "$volume" && mkdir -p "$volume"
    cp -R "$work/$name.app" "$volume/"
    ln -s /Applications "$volume/Applications"
    local dmg="$work/$name-$version-universal.dmg"
    hdiutil create -volname "$name" -srcfolder "$volume" -ov -format UDZO "$dmg"
    cp "$dmg" "$dist/$name-$version-universal.dmg"
}

case "$(uname -s)" in
    Linux) package_linux ;;
    Darwin) package_macos ;;
    *) echo "Unsupported platform; on Windows use scripts/package.ps1" >&2; exit 1 ;;
esac

ls -l "$dist"
