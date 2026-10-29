<#
.SYNOPSIS
    Build and package one release asset (Windows convenience wrapper; CI uses tools/dist.sh).

.DESCRIPTION
    Writes into ./releases :
      <Label>/                          the unpacked payload, ready to use as an install dir
      scd-<version>-<target>.<ext>      the file you upload to a GitHub Release
      scd-<version>-<target>.<ext>.sha256

    Payload layout matches docs/ARCHITECTURE.md, so unzipping gives you a working install:
      bin/scd(.exe)   shell/   config.example.json   install.ps1   install.sh
      README.md   CHANGELOG.md   LICENSE

    config.json is deliberately NOT packaged: it is user state, and shipping it would make
    every upgrade fight with the locations the user already saved.

.EXAMPLE
    .\tools\dist.ps1                                  # build + package x86_64-pc-windows-msvc
    .\tools\dist.ps1 -SkipBuild                       # reuse the existing target\release build
#>
[CmdletBinding()]
param(
    [string] $Target = 'x86_64-pc-windows-msvc',
    [string] $Label = 'Windows_x64',
    [ValidateSet('zip', 'tar.gz')] [string] $Ext = 'zip',
    [string] $Version,
    [switch] $SkipBuild
)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
# Not $isWindows: on PowerShell 7 that is a read-only automatic variable ($IsWindows), so
# assigning it aborts the script before it does anything. Windows PowerShell 5.1 has no such
# variable, which is why this only ever blew up under CI's shell: pwsh.
$forWindows = $Target -like '*windows*'
$exeName = if ($forWindows) { 'scd.exe' } else { 'scd' }

function Step([string] $Message) { Write-Host "==> $Message" -ForegroundColor Cyan }

if (-not $Version) {
    $toml = Get-Content -LiteralPath (Join-Path $repo 'Cargo.toml') -Raw
    if ($toml -notmatch '(?m)^version\s*=\s*"([^"]+)"') { throw 'cannot read version from Cargo.toml' }
    $Version = $Matches[1]
}
if (-not $forWindows) {
    Write-Warning "Packaging a unix target from Windows cannot set the executable bit - use tools/dist.sh (or CI) for $Target."
}

# --- 1. build ---------------------------------------------------------------
if (-not $SkipBuild) {
    Step "cargo build --release --locked --target $Target"
    Push-Location $repo
    try {
        & cargo build --release --locked --target $Target
        if ($LASTEXITCODE -ne 0) { throw "cargo build failed with exit code $LASTEXITCODE" }
    }
    finally { Pop-Location }
}
$built = Join-Path $repo "target\$Target\release\$exeName"
if (-not (Test-Path -LiteralPath $built)) { throw "no binary at $built - run without -SkipBuild" }

# --- 2. stage the payload ---------------------------------------------------
$folder = "scd-$Version-$Target"
$release = Join-Path $repo 'releases'
$stageRoot = Join-Path $release '.stage'
$stage = Join-Path $stageRoot $folder
if (Test-Path -LiteralPath $stageRoot) { Remove-Item -LiteralPath $stageRoot -Recurse -Force }
New-Item -ItemType Directory -Force -Path (Join-Path $stage 'bin') | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $stage 'shell') | Out-Null

Step "staging $folder"
Copy-Item -LiteralPath $built -Destination (Join-Path $stage "bin\$exeName")
Copy-Item -Path (Join-Path $repo 'shell\*') -Destination (Join-Path $stage 'shell')
foreach ($file in 'config.example.json', 'install.ps1', 'install.cmd', 'install.sh', 'README.md', 'CHANGELOG.md', 'LICENSE') {
    $source = Join-Path $repo $file
    if (-not (Test-Path -LiteralPath $source)) { throw "missing $file - a release needs it" }
    Copy-Item -LiteralPath $source -Destination $stage
}

# --- 3. the unpacked copy the user can run straight away --------------------
$payload = Join-Path $release $Label
if (Test-Path -LiteralPath $payload) { Remove-Item -LiteralPath $payload -Recurse -Force }
Copy-Item -LiteralPath $stage -Destination $payload -Recurse

# --- 4. the uploadable archive (bsdtar ships with Windows 10+ and writes .zip too) ---
$asset = "$folder.$Ext"
$assetPath = Join-Path $release $asset
if (Test-Path -LiteralPath $assetPath) { Remove-Item -LiteralPath $assetPath -Force }
Step "creating releases\$asset"
# Use Windows' own bsdtar: a GNU tar earlier on PATH would exit 0 and silently write an
# uncompressed *tar* named .zip instead of a real zip.
$systemTar = Join-Path $env:SystemRoot 'System32\tar.exe'
$tarExe = if (Test-Path -LiteralPath $systemTar) { $systemTar } else { 'tar' }
Push-Location $stageRoot
try {
    & $tarExe -a -cf $assetPath $folder
    if ($LASTEXITCODE -ne 0) { throw "tar failed with exit code $LASTEXITCODE" }
}
finally { Pop-Location }

# Verify the archive really is what its name claims.
$magic = [IO.File]::ReadAllBytes($assetPath)[0..1]
if ($Ext -eq 'zip' -and -not ($magic[0] -eq 0x50 -and $magic[1] -eq 0x4B)) {
    throw "$asset has no PK header - it is not a real zip (a GNU tar on PATH is the usual cause)"
}
if ($Ext -eq 'tar.gz' -and -not ($magic[0] -eq 0x1F -and $magic[1] -eq 0x8B)) {
    throw "$asset is not gzip-compressed"
}

Remove-Item -LiteralPath $stageRoot -Recurse -Force

# --- 5. checksum ------------------------------------------------------------
$hash = (Get-FileHash -LiteralPath $assetPath -Algorithm SHA256).Hash.ToLowerInvariant()
[IO.File]::WriteAllText("$assetPath.sha256", "$hash  $asset`n", (New-Object Text.UTF8Encoding($false)))

# --- 6. summary -------------------------------------------------------------
Write-Host ''
Write-Host "scd $Version packaged for $Target" -ForegroundColor Green
Get-ChildItem -Recurse -File $payload | ForEach-Object {
    '  {0,-52} {1,9:N0} B' -f $_.FullName.Replace($payload, $Label), $_.Length
}
Write-Host ''
Write-Host "  upload this file:  releases\$asset" -ForegroundColor Yellow
Write-Host "  checksum:          releases\$asset.sha256"
Write-Host "  sha256:            $hash"
