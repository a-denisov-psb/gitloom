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

# MSI. WiX needs a numeric version: a pre-release 1.0.0-rc.N becomes
# 1.0.0.N (Windows Installer ignores the fourth field for upgrades, so the
# final 1.0.0 replaces it). cargo-packager takes the version from Cargo, so
# for a pre-release its configuration is passed in full with that version.
$work = Join-Path $root "target\packager\windows-$arch"
if (Test-Path $work) { Remove-Item -Recurse -Force $work }
New-Item -ItemType Directory -Force $work | Out-Null
$packagerArgs = @("packager", "-p", $bin, "--release", "--target", $Target, "-f", "wix",
    "--binaries-dir", $bins, "--out-dir", $work)
$configFile = $null
if ($version -match '^(\d+\.\d+\.\d+)-rc\.(\d+)$') {
    $msiVersion = "$($Matches[1]).$($Matches[2])"
    # Read Cargo's UTF-8 output as such (the copyright contains a non-ASCII sign).
    [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
    $metadata = (cargo metadata --no-deps --format-version 1 --locked) -join "`n" | ConvertFrom-Json
    $package = $metadata.packages | Where-Object { $_.name -eq $bin }
    $config = $package.metadata.packager
    $config | Add-Member -NotePropertyName version -NotePropertyValue $msiVersion -Force
    $config | Add-Member -NotePropertyName description -NotePropertyValue $package.description -Force
    $config | Add-Member -NotePropertyName homepage -NotePropertyValue $package.homepage -Force
    $config | Add-Member -NotePropertyName binaries -NotePropertyValue @(@{ path = $bin; main = $true }) -Force
    # Relative paths in the configuration are relative to the crate.
    $configFile = Join-Path (Split-Path $package.manifest_path) "packager-msi.json"
    # UTF-8 without BOM: cargo-packager's JSON parser rejects a BOM.
    [System.IO.File]::WriteAllText($configFile, ($config | ConvertTo-Json -Depth 10),
        [System.Text.UTF8Encoding]::new($false))
    $packagerArgs = @("packager", "--config", $configFile, "--target", $Target, "-f", "wix",
        "--binaries-dir", $bins, "--out-dir", $work)
} elseif ($version -match '-') {
    throw "Pre-release version ${version}: only -rc.N can be mapped to an MSI version"
}
try {
    Run cargo $packagerArgs
} finally {
    if ($configFile) { Remove-Item -Force $configFile }
}
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
