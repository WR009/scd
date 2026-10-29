<#
.SYNOPSIS
    Build scd and install it in the layout described by docs/ARCHITECTURE.md.

.DESCRIPTION
    Creates, and only ever writes inside, <Dest>:

        <Dest>\bin\scd.exe        the program          (put this directory on PATH)
        <Dest>\shell\scd.ps1      the PowerShell function that performs the real `cd`
        <Dest>\config.json        your locations and settings (kept across upgrades)

    scd itself never touches environment variables - that step belongs to the user, as the
    architecture document requires. This script only offers to do it for you:
      -AddToPath     appends <Dest>\bin to the *user* PATH
      -AddToProfile  appends `. "<Dest>\shell\scd.ps1"` to $PROFILE

.PARAMETER Dest
    Install directory. Default: $HOME\scd

.EXAMPLE
    .\install.ps1
    Build (release) and install into $HOME\scd, then print the remaining manual steps.

.EXAMPLE
    .\install.ps1 -Dest D:\tools\scd -AddToPath -AddToProfile
    Install somewhere else and wire up PATH plus $PROFILE automatically.

.EXAMPLE
    .\install.ps1 -SkipBuild
    Reuse an existing target\release\scd.exe instead of rebuilding.
#>
[CmdletBinding()]
param(
    [string] $Dest = (Join-Path $HOME 'scd'),
    [ValidateSet('release', 'debug')]
    [string] $Configuration = 'release',
    [switch] $SkipBuild,
    [switch] $AddToPath,
    [switch] $AddToProfile
)

$ErrorActionPreference = 'Stop'
$repo = $PSScriptRoot
$exeName = 'scd.exe'

function Write-Step([string] $Message) {
    Write-Host "==> $Message" -ForegroundColor Cyan
}

# --- 1. find or build the binary -------------------------------------------------
# Two shapes are supported, so the same script serves a developer and a downloader:
#   * the source repository  (has Cargo.toml)      -> build with cargo, then install
#   * an extracted release   (has bin\scd.exe)     -> nothing to build, install as is
$isRepo = Test-Path -LiteralPath (Join-Path $repo 'Cargo.toml')
$prebuilt = Join-Path $repo "bin\$exeName"
$hasPrebuilt = Test-Path -LiteralPath $prebuilt
if (-not $isRepo -and -not $hasPrebuilt) {
    throw "neither Cargo.toml nor bin\$exeName found in $repo - run this from the repository or from an extracted release archive"
}

$built = Join-Path $repo "target\$Configuration\$exeName"
if ($hasPrebuilt -and -not $isRepo) {
    Write-Step "release archive detected: using $prebuilt (no Rust toolchain needed)"
    $built = $prebuilt
}
elseif ($SkipBuild) {
    if (-not (Test-Path -LiteralPath $built)) {
        if ($hasPrebuilt) { $built = $prebuilt } else { throw "no build found at $built - run without -SkipBuild" }
    }
    Write-Step "using the existing build $built"
}
else {
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        if ($hasPrebuilt) {
            Write-Step 'cargo is not on PATH - falling back to the prebuilt binary'
            $built = $prebuilt
        }
        else {
            throw 'cargo is not on PATH. Install Rust (https://rustup.rs) or use -SkipBuild.'
        }
    }
    else {
        Write-Step "cargo build --$Configuration (no dependencies, works offline)"
        Push-Location $repo
        try {
            & cargo build "--$Configuration"
            if ($LASTEXITCODE -ne 0) { throw "cargo build failed with exit code $LASTEXITCODE" }
        }
        finally {
            Pop-Location
        }
    }
}

# --- 2. the documented layout --------------------------------------------------
$binDir = Join-Path $Dest 'bin'
$shellDir = Join-Path $Dest 'shell'
Write-Step "installing into $Dest"
New-Item -ItemType Directory -Force -Path $binDir | Out-Null
New-Item -ItemType Directory -Force -Path $shellDir | Out-Null
Copy-Item -LiteralPath $built -Destination (Join-Path $binDir $exeName) -Force
Copy-Item -Path (Join-Path $repo 'shell\*') -Destination $shellDir -Force

# Docs and the installer itself, so <Dest> is self-contained (and re-installable).
foreach ($extra in 'install.ps1', 'install.cmd', 'install.sh', 'README.md', 'CHANGELOG.md', 'LICENSE', 'config.example.json') {
    $source = Join-Path $repo $extra
    $target = Join-Path $Dest $extra
    if (-not (Test-Path -LiteralPath $source)) { continue }
    # `-Dest .` means "install in place": never copy a file onto itself.
    if ([IO.Path]::GetFullPath($source) -eq [IO.Path]::GetFullPath($target)) { continue }
    Copy-Item -LiteralPath $source -Destination $target -Force
}

# --- 3. starter config.json (an existing one is never touched) -----------------
$configPath = Join-Path $Dest 'config.json'
if (Test-Path -LiteralPath $configPath) {
    Write-Step "keeping the existing $configPath"
}
else {
    $starter = @"
{
  "version": 1,
  "max_display": 9,
  "locations": []
}
"@
    [IO.File]::WriteAllText($configPath, $starter + [Environment]::NewLine, (New-Object Text.UTF8Encoding($false)))
    Write-Step "created $configPath"
}

$installedExe = Join-Path $binDir $exeName
& $installedExe --version

# --- 4. optional: PATH ---------------------------------------------------------
if ($AddToPath) {
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    if ([string]::IsNullOrEmpty($userPath)) { $userPath = '' }
    $entries = New-Object 'System.Collections.Generic.List[string]'
    $entries.AddRange([string[]]@($userPath -split ';' | Where-Object { $_ -ne '' }))

    if ($entries.Contains($binDir)) {
        Write-Step "$binDir is already on the user PATH"
    }
    else {
        # Note: this stores the expanded value; %VAR% indirections in the user PATH
        # are resolved once, which is the normal behaviour of the PATH editor too.
        $entries.Add($binDir)
        Write-Step "added $binDir to the user PATH (open a new terminal to pick it up)"
    }

    # cmd.exe cannot define a function, so there "scd" is shell\scd.cmd - and because
    # cmd.exe prefers .exe over .cmd inside one directory, shell\ must sit AHEAD of bin\.
    if ($entries.Contains($shellDir)) {
        Write-Step "$shellDir is already on the user PATH"
    }
    else {
        $entries.Insert($entries.IndexOf($binDir), $shellDir)
        Write-Step "added $shellDir to the user PATH (ahead of bin, so cmd.exe can cd too)"
    }

    $newPath = [string]::Join(';', $entries.ToArray())
    if ($newPath -ne $userPath) { [Environment]::SetEnvironmentVariable('Path', $newPath, 'User') }
    foreach ($dir in @($shellDir, $binDir)) {
        if (@($env:Path -split ';') -notcontains $dir) { $env:Path = "$env:Path;$dir" }
    }
}

# --- 5. optional: $PROFILE -----------------------------------------------------
$wrapper = Join-Path $shellDir 'scd.ps1'
if ($AddToProfile) {
    $profilePath = $PROFILE
    if ((Test-Path -LiteralPath $profilePath) -and
        ((Get-Content -LiteralPath $profilePath -Raw) -match [regex]::Escape($wrapper))) {
        Write-Step "$profilePath already loads scd"
    }
    else {
        $profileDir = Split-Path -Parent $profilePath
        if ($profileDir -and -not (Test-Path -LiteralPath $profileDir)) {
            New-Item -ItemType Directory -Force -Path $profileDir | Out-Null
        }
        Add-Content -LiteralPath $profilePath -Value "`r`n# scd - super cd (added by install.ps1)`r`n. `"$wrapper`""
        Write-Step "added the scd function to $profilePath"
    }
}

# --- 6. what to do next --------------------------------------------------------
Write-Host ''
Write-Host "scd is installed in $Dest" -ForegroundColor Green
Write-Host ''
if (-not $AddToPath) {
    Write-Host '1) put the program on PATH (系统变量 -> Path -> 新建):' -ForegroundColor Yellow
    Write-Host "     $binDir"
    Write-Host "     $shellDir   (cmd.exe needs this one too, ahead of bin)"
    Write-Host '   or let this script do it next time: .\install.ps1 -AddToPath'
    Write-Host ''
}
if (-not $AddToProfile) {
    Write-Host '2) load the shell function so "scd" really changes directory:' -ForegroundColor Yellow
    Write-Host "     Add-Content `$PROFILE `'. `"$wrapper`"`'   ; then restart the shell"
    Write-Host '   or let this script do it next time: .\install.ps1 -AddToProfile'
    Write-Host ''
}

# A Restricted execution policy stops PowerShell from running $PROFILE at all, so the scd
# function would never load and "scd" would fall back to the .exe, which can only print
# where to go. Say so here rather than letting the user wonder why cd did not happen.
if ($AddToProfile -and (Get-ExecutionPolicy) -in @('Restricted', 'Undefined')) {
    Write-Host ''
    Write-Host 'NOTE: your PowerShell execution policy is Restricted, so $PROFILE will not run' -ForegroundColor Yellow
    Write-Host '      and the scd function will not load. Fix it once (current user, no admin):' -ForegroundColor Yellow
    Write-Host '        Set-ExecutionPolicy -Scope CurrentUser RemoteSigned' -ForegroundColor Yellow
}
Write-Host '3) try it:' -ForegroundColor Yellow
Write-Host '     scd            # the [c]/[a]/[1..n] menu'
Write-Host '     scd add .      # save the current directory'
Write-Host '     scd <name>     # jump straight there'
Write-Host '     scd --help     # the full user guide'
