# Builds the Windows packages into dist\ (M6-08, ADR-031):
#   <Name>-<version>-<arch>.msi  (WiX via cargo-packager, per machine)
#   <name>-<version>-windows-<arch>.zip  (portable: exe, licenses, README)
# Usage: scripts\package.ps1 [-Target x86_64-pc-windows-msvc | i686-pc-windows-msvc]
# Needs cargo-packager (`cargo install --locked cargo-packager`); WiX is
# downloaded by cargo-packager on first use.
param([string]$Target = "x86_64-pc-windows-msvc")
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")
$root = (Get-Location).Path

function Run($exe, [string[]]$arguments) {
    & $exe @arguments
    if ($LASTEXITCODE -ne 0) { throw "$exe $($arguments -join ' ') failed" }
}

# Name-dependent values come from git_core::identity (ADR-028).
$identity = Get-Content "crates\git-core\src\identity.rs" -Raw
function Const($name) {
    if ($identity -match "pub const ${name}: &str = `"([^`"]*)`";") { return $Matches[1] }
    throw "$name not found in identity.rs"
}
$name = Const "DISPLAY_NAME"
$bin = Const "BINARY_NAME"
$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.*)"' | Select-Object -First 1).Matches[0].Groups[1].Value
$arch = if ($Target -like "i686-*") { "x86" } else { "x64" }

$dist = Join-Path $root "dist"
New-Item -ItemType Directory -Force $dist | Out-Null
Run cargo @("build", "--release", "--locked", "-p", $bin, "--target", $Target)
$bins = Join-Path $root "target\$Target\release"

# MSI
$work = Join-Path $root "target\packager\windows-$arch"
if (Test-Path $work) { Remove-Item -Recurse -Force $work }
New-Item -ItemType Directory -Force $work | Out-Null
Run cargo @("packager", "-p", $bin, "--release", "--target", $Target, "-f", "wix",
    "--binaries-dir", $bins, "--out-dir", $work)
$msi = Get-ChildItem -Path $work -Filter *.msi -Recurse | Select-Object -First 1
if (-not $msi) { throw "no .msi produced" }
Copy-Item $msi.FullName (Join-Path $dist "$name-$version-$arch.msi") -Force

# Portable zip
$stage = Join-Path $work "$bin-$version"
New-Item -ItemType Directory -Force $stage | Out-Null
Copy-Item (Join-Path $bins "$bin.exe"), LICENSE, THIRD-PARTY-LICENSES.md, README.md $stage
$zip = Join-Path $dist "$bin-$version-windows-$arch.zip"
if (Test-Path $zip) { Remove-Item $zip }
Compress-Archive -Path $stage -DestinationPath $zip

Get-ChildItem $dist
