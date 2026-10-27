@echo off
rem scd - shell integration for cmd.exe.
rem
rem Put <install>\shell AHEAD of <install>\bin on PATH so that "scd" finds this wrapper
rem instead of scd.exe (cmd.exe resolves .exe before .cmd inside one directory).
rem A batch file runs inside the caller's cmd.exe, so the cd below really moves you.

setlocal EnableExtensions
set "SCD_TMP=%TEMP%\scd_%RANDOM%%RANDOM%.txt"
if defined SCD_BIN (set "SCD_EXE=%SCD_BIN%") else (set "SCD_EXE=scd.exe")

set "SCD_TARGET_FILE=%SCD_TMP%"
"%SCD_EXE%" %*
set "SCD_RC=%ERRORLEVEL%"

set "SCD_TGT="
if exist "%SCD_TMP%" (
    set /p SCD_TGT=<"%SCD_TMP%"
    del "%SCD_TMP%" >nul 2>&1
)

if not defined SCD_TGT (endlocal & exit /b %SCD_RC%)
endlocal & cd /d "%SCD_TGT%" & exit /b %SCD_RC%
