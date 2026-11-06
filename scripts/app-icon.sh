#!/usr/bin/env bash
# Regenerates the app icon files from crates/app/assets/app-icon/icon.svg
# (M6-04): icon-<size>.png (16 … 1024), icon.ico (Windows), icon.icns (macOS).
set -euo pipefail
cd "$(dirname "$0")/.."
dir=crates/app/assets/app-icon
# The app embeds icon-256.png; a first run needs a file to compile.
[[ -f "$dir/icon-256.png" ]] || : > "$dir/icon-256.png"
cargo test -p gitloom --locked generate_app_icons -- --ignored --exact app_icon::tests::generate_app_icons
echo "Wrote $dir/icon-*.png, icon.ico and icon.icns"
