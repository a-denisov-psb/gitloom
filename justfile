# Task runner (ADR-052). Install: `cargo install --locked just`
# (Windows: `winget install Casey.Just`), then run `just` for the list.

set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-Command"]

# List the recipes
[private]
default:
    @just --list

# Quality gate: format, clippy, tests
[unix]
check:
    ./scripts/check.sh

# Quality gate: format, clippy, tests
[windows]
check:
    powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts\check.ps1

# Portable Windows x64 exe in dist/ (same as `just build win-x64`)
portable: (build "win-x64")

# Build the bare binary for a target into dist/ (see `just targets`)
[unix]
build target:
    ./scripts/build.sh {{ quote(target) }}

# Build the bare binary for a target into dist/ (see `just targets`)
[windows]
build target:
    powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts\build.ps1 {{ target }}

# Target aliases and whether this machine can build them
[unix]
targets:
    @./scripts/build.sh --targets

# Target aliases and whether this machine can build them
[windows]
targets:
    @powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts\build.ps1 -Targets

# Release packages of this platform into dist/ (AppImage/tar.gz/deb, dmg, msi/zip)
[unix]
package:
    ./scripts/package.sh

# Release packages of this platform into dist/ (AppImage/tar.gz/deb, dmg, msi/zip)
[windows]
package:
    powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts\package.ps1

# Packages of all platforms from a CI dry run of `ref` into dist/ci-<sha>/ (needs gh)
[unix]
ci-packages ref="develop":
    ./scripts/ci-packages.sh {{ quote(ref) }}

# Packages of all platforms from a CI dry run of `ref` into dist/ci-<sha>/ (needs gh)
[windows]
ci-packages ref="develop":
    powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts\ci-packages.ps1 {{ ref }}

# Remove build output and dist/ (the next build takes longer)
[unix]
clean:
    cargo clean
    rm -rf dist
    @echo "Removed target/ and dist/; the next build compiles everything again and takes longer."

# Remove build output and dist/ (the next build takes longer)
[windows]
clean:
    cargo clean
    if (Test-Path dist) { Remove-Item -Recurse -Force dist }
    @Write-Host "Removed target\ and dist\; the next build compiles everything again and takes longer."
