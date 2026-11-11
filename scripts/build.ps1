# Builds the bare binary for one target with the `dist` profile and copies it
# to dist\<bin>-<version>-<alias>.exe (ADR-052). Used by `just build`.
#   scripts\build.ps1 <alias>     e.g. win-x64, win-arm64
#   scripts\build.ps1 -Targets    table of aliases and what this machine can build
# Linux, macOS: scripts/build.sh.
param([string]$Alias = "", [switch]$Targets)
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

$aliases = @("win-x64", "win-x86", "win-arm64", "linux-x64", "linux-arm64", "mac")

# Name-dependent values come from git_core::identity (ADR-028).
$identity = Get-Content "crates\git-core\src\identity.rs" -Raw
if ($identity -notmatch 'pub const BINARY_NAME: &str = "([^"]*)";') { throw "BINARY_NAME not found in identity.rs" }
$bin = $Matches[1]
$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.*)"' | Select-Object -First 1).Matches[0].Groups[1].Value

$installed = @()
if (Get-Command rustup -ErrorAction SilentlyContinue) { $installed = @(rustup target list --installed) }

# Returns @{ Triple; Problem } (Problem empty when buildable here), $null for an unknown alias.
function Resolve-Alias([string]$name) {
    $triple = switch ($name) {
        "win-x64" { "x86_64-pc-windows-msvc" }
        "win-x86" { "i686-pc-windows-msvc" }
        "win-arm64" { "aarch64-pc-windows-msvc" }
        "linux-x64" { "x86_64-unknown-linux-gnu" }
        "linux-arm64" { "aarch64-unknown-linux-gnu" }
        "mac" { "universal-apple-darwin" }
        default { "" }
    }
    if (-not $triple) { return $null }
    $problem = ""
    if ($name -like "linux-*") {
        $problem = "build it in WSL (just build $name) or use: just ci-packages"
    } elseif ($name -eq "mac") {
        $problem = "needs macOS; use: just ci-packages"
    } elseif ($installed.Count -gt 0 -and $installed -notcontains $triple) {
        $problem = "needs the Rust target: rustup target add $triple"
    }
    return @{ Triple = $triple; Problem = $problem }
}

if ($Targets) {
    "{0,-12} {1,-26} {2}" -f "Alias", "Rust target", "On this machine"
    foreach ($a in $aliases) {
        $r = Resolve-Alias $a
        $status = if ($r.Problem) { $r.Problem } else { "yes" }
        if (-not $r.Problem -and $a -eq "win-arm64") { $status = "yes (needs the MSVC ARM64 build tools)" }
        "{0,-12} {1,-26} {2}" -f $a, $r.Triple, $status
    }
    exit 0
}

$r = Resolve-Alias $Alias
if (-not $r) {
    Write-Host "Unknown target '$Alias'. Valid targets: $($aliases -join ' ')" -ForegroundColor Red
    exit 1
}
if ($r.Problem) {
    Write-Host "${Alias} ($($r.Triple)): $($r.Problem)" -ForegroundColor Red
    exit 1
}

& cargo build --profile dist --locked -p $bin --target $r.Triple
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
New-Item -ItemType Directory -Force dist | Out-Null
$out = "dist\$bin-$version-$Alias.exe"
Copy-Item "target\$($r.Triple)\dist\$bin.exe" $out -Force
"{0} ({1:N1} MB)" -f $out, ((Get-Item $out).Length / 1MB)
