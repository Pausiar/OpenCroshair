# Builds the portable ZIP: opencrosshair.exe, LICENSE and README.md at the top level.
# Run `cargo build --release` first.
#
#   powershell -ExecutionPolicy Bypass -File installer\build-portable.ps1 -Version 0.1.0

param(
    [Parameter(Mandatory = $true)]
    [string]$Version
)

$ErrorActionPreference = 'Stop'

if ($Version -notmatch '^\d+\.\d+\.\d+$') {
    throw "Version '$Version' must look like X.Y.Z"
}

$root = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $root 'target\release\opencrosshair.exe'
$license = Join-Path $root 'LICENSE'
$readme = Join-Path $root 'README.md'

foreach ($required in @($exe, $license, $readme)) {
    if (-not (Test-Path -LiteralPath $required)) {
        throw "Missing '$required'. Run 'cargo build --release' first if the executable is missing."
    }
}

$dist = Join-Path $root 'dist'
$staging = Join-Path $dist 'portable-staging'
$zip = Join-Path $dist "OpenCrosshair-$Version-portable-win64.zip"

New-Item -ItemType Directory -Force -Path $dist | Out-Null
if (Test-Path -LiteralPath $staging) {
    Remove-Item -LiteralPath $staging -Recurse -Force
}
New-Item -ItemType Directory -Path $staging | Out-Null

try {
    Copy-Item -LiteralPath $exe -Destination $staging
    Copy-Item -LiteralPath $license -Destination $staging
    Copy-Item -LiteralPath $readme -Destination $staging

    if (Test-Path -LiteralPath $zip) {
        Remove-Item -LiteralPath $zip -Force
    }
    Compress-Archive -Path (Join-Path $staging '*') -DestinationPath $zip -CompressionLevel Optimal
}
finally {
    Remove-Item -LiteralPath $staging -Recurse -Force -ErrorAction SilentlyContinue
}

Write-Host "Portable ZIP written to $zip"
