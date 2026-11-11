#!/usr/bin/env bash
# Renders the README screenshot from a scripted fixture repository (M11-15):
# fictional authors, Dark theme, 1600 x 900 at 100 %, headless (tiny-skia,
# no display needed). Writes docs/screenshots/main-dark.png.
set -euo pipefail
cd "$(dirname "$0")/.."
ICED_TEST_BACKEND=tiny-skia cargo test -q -p gitloom --bin gitloom readme_screenshot \
    -- --ignored --exact ui_check::tests::readme_screenshot
cp target/ui-snapshots/readme-main-dark.png docs/screenshots/main-dark.png
echo "Wrote docs/screenshots/main-dark.png"
