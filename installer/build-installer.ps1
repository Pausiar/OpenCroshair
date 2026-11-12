# Builds the per-user Inno Setup installer from installer\OpenCrosshair.iss.
# Requires Inno Setup 6 (ISCC.exe on PATH or in the default install folder) and
# `cargo build --release` to have produced target\release\opencrosshair.exe.
#
#   powershell -ExecutionPolicy Bypass -File installer\build-installer.ps1 -Version 0.1.0

param(
    [Parameter(Mandatory = $true)]
    [string]$Version
)

$ErrorActionPreference = 'Stop'

if ($Version -notmatch '^\d+\.\d+\.\d+$') {
    throw "Version '$Version' must look like X.Y.Z"
}

$root = Split-Path -Parent $PSScriptRoot
$script = Join-Path $root 'installer\OpenCrosshair.iss'
$inputs = @(
    (Join-Path $root 'target\release\opencrosshair.exe'),
    (Join-Path $root 'assets\icons\icon.ico'),
    (Join-Path $root 'LICENSE'),
    (Join-Path $root 'README.md'),
    $script
)
foreach ($path in $inputs) {
    if (-not (Test-Path -LiteralPath $path)) {
        throw "Missing '$path'. Build the release binary with 'cargo build --release', and make sure assets\icons\icon.ico exists."
    }
}

function Find-Iscc {
    $command = Get-Command iscc -ErrorAction SilentlyContinue
    if ($command) {
        return $command.Source
    }
    $candidates = @(
        (Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6\ISCC.exe'),
        (Join-Path $env:ProgramFiles 'Inno Setup 6\ISCC.exe'),
        (Join-Path $env:LOCALAPPDATA 'Programs\Inno Setup 6\ISCC.exe')
    )
    foreach ($candidate in $candidates) {
        if ($candidate -and (Test-Path -LiteralPath $candidate)) {
            return $candidate
        }
    }
    throw 'Inno Setup 6 was not found. Install it from https://jrsoftware.org/isinfo.php or run: choco install innosetup'
}

$iscc = Find-Iscc
Write-Host "Using $iscc"

New-Item -ItemType Directory -Force -Path (Join-Path $root 'dist') | Out-Null

& $iscc "/DMyAppVersion=$Version" $script
if ($LASTEXITCODE -ne 0) {
    throw "ISCC failed with exit code $LASTEXITCODE"
}

$output = Join-Path $root "dist\OpenCrosshair-$Version-setup.exe"
if (-not (Test-Path -LiteralPath $output)) {
    throw "Expected installer was not produced: $output"
}
Write-Host "Installer written to $output"
