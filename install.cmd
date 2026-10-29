@echo off
rem Double-click installer for Windows.
rem
rem Runs install.ps1 with an execution policy bypass (needed because the default policy
rem is Restricted, which blocks .ps1 files), then keeps the window open so you can read
rem the result. Extra arguments are passed through, e.g.:
rem     install.cmd -Dest D:\tools\scd
rem     install.cmd -SkipBuild

setlocal
set "HERE=%~dp0"
echo.
echo   scd installer
echo   source : %HERE%
echo   target : %%USERPROFILE%%\scd   (override with -Dest)
echo.
powershell -NoProfile -ExecutionPolicy Bypass -File "%HERE%install.ps1" -AddToPath -AddToProfile %*
set "RC=%ERRORLEVEL%"
echo.
if "%RC%"=="0" (
    echo   Done. Open a NEW terminal and type:  scd
) else (
    echo   install.ps1 failed with exit code %RC% - read the message above.
)
echo.
pause
endlocal & exit /b %RC%
