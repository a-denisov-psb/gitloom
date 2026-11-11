# Runs the release workflow as a dry run on a ref, waits for it and downloads
# all packages into dist\ci-<short-sha>\ (ADR-052). Used by `just ci-packages`.
#   scripts\ci-packages.ps1 [ref]    default: develop
# Needs the GitHub CLI, logged in (`gh auth login`). Linux, macOS: scripts/ci-packages.sh.
param([string]$Ref = "develop")
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
    Write-Host "Needs the GitHub CLI: winget install GitHub.cli (then: gh auth login)" -ForegroundColor Red
    exit 1
}
& gh auth status *> $null
if ($LASTEXITCODE -ne 0) {
    Write-Host "The GitHub CLI is not logged in: gh auth login" -ForegroundColor Red
    exit 1
}

$started = (Get-Date).ToUniversalTime().AddSeconds(-5)
& gh workflow run release.yml --ref $Ref
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
"Waiting for the run to appear ..."
$id = ""
for ($i = 0; $i -lt 30 -and -not $id; $i++) {
    Start-Sleep -Seconds 2
    $runs = (& gh run list --workflow release.yml --event workflow_dispatch --branch $Ref `
        --limit 5 --json databaseId,createdAt) -join "`n" | ConvertFrom-Json
    # Windows PowerShell keeps createdAt as text, PowerShell 7 makes it a date.
    $run = $runs | Where-Object { ([datetime]$_.createdAt).ToUniversalTime() -ge $started } |
        Select-Object -First 1
    if ($run) { $id = "$($run.databaseId)" }
}
if (-not $id) {
    Write-Host "The workflow run did not show up; see: gh run list --workflow release.yml" -ForegroundColor Red
    exit 1
}

& gh run watch $id --exit-status
$status = $LASTEXITCODE
$sha = & gh run view $id --json headSha --jq '.headSha[0:7]'
$out = "dist\ci-$sha"
if (Test-Path $out) { Remove-Item -Recurse -Force $out }
New-Item -ItemType Directory -Force $out | Out-Null
# Best-effort jobs may have failed: download what the run produced.
& gh run download $id --dir $out
# One folder per artifact; flatten them.
Get-ChildItem $out -Directory | ForEach-Object {
    Get-ChildItem $_.FullName -File -Recurse | Move-Item -Destination $out -Force
    Remove-Item -Recurse -Force $_.FullName
}
Get-ChildItem $out
if ($status -ne 0) {
    Write-Host "The run failed (see: gh run view $id); the files above are what it produced." -ForegroundColor Red
}
exit $status
