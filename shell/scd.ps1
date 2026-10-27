# scd - shell integration for PowerShell (Windows PowerShell 5.1 and PowerShell 7+).
#
#   . "$HOME\scd\shell\scd.ps1"        # put this line in $PROFILE to load it on start
#
# Why this file exists: a child process cannot change the working directory of the shell
# that started it. scd.exe therefore resolves the directory and writes it to a temporary
# file named by $env:SCD_TARGET_FILE; this function reads it back and performs the real cd.
# The menu itself keeps talking to the console, so interactive use works as usual.
#
# Note: do not pipe into this function. Scripted input should call scd.exe directly, or
# use the non-interactive forms (scd add <path>, scd rm <target>, scd <target>).

function scd {
    param(
        # Everything the user typed is forwarded verbatim; no parameter here may compete
        # for positional arguments, or "scd list" would stop reaching the program.
        [Parameter(ValueFromRemainingArguments = $true)]
        [string[]] $ScdArgs
    )

    $exe = $null
    if ($env:SCD_BIN) {
        $exe = $env:SCD_BIN
        if (-not (Test-Path -LiteralPath $exe)) {
            Write-Error "scd: SCD_BIN points at a missing file: $exe"
            return
        }
    }
    else {
        # -CommandType Application only finds executables, so this cannot recurse.
        $found = Get-Command scd.exe -CommandType Application -ErrorAction SilentlyContinue |
            Select-Object -First 1
        if (-not $found) {
            $found = Get-Command scd -CommandType Application -ErrorAction SilentlyContinue |
                Select-Object -First 1
        }
        if ($found) { $exe = $found.Source }
    }
    if (-not $exe) {
        Write-Error 'scd: cannot find scd.exe - put <install>\bin on PATH, or set $env:SCD_BIN'
        return
    }

    $targetFile = Join-Path ([IO.Path]::GetTempPath()) ("scd-{0}.txt" -f [guid]::NewGuid().ToString('N'))
    $previous = $env:SCD_TARGET_FILE
    $env:SCD_TARGET_FILE = $targetFile
    $code = 0
    try {
        & $exe @ScdArgs
        $code = $LASTEXITCODE
        if ($code -eq 0 -and (Test-Path -LiteralPath $targetFile)) {
            # Explicit UTF-8 read: Windows PowerShell 5.1 would otherwise guess the code
            # page and mangle non-ASCII directory names.
            $destination = [IO.File]::ReadAllText($targetFile, (New-Object Text.UTF8Encoding($false))).Trim()
            if ($destination) {
                if (Test-Path -LiteralPath $destination -PathType Container) {
                    Set-Location -LiteralPath $destination
                }
                else {
                    Write-Warning "scd: not a directory: $destination"
                }
            }
        }
    }
    finally {
        if ($null -eq $previous) { Remove-Item Env:\SCD_TARGET_FILE -ErrorAction SilentlyContinue }
        else { $env:SCD_TARGET_FILE = $previous }
        Remove-Item -LiteralPath $targetFile -Force -ErrorAction SilentlyContinue
    }
    $global:LASTEXITCODE = $code
}