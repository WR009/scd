@echo off
rem Double-click installer for Windows.
rem
rem Asks where to install (default %USERPROFILE%\scd), then runs install.ps1 with an
rem execution policy bypass - the machine default is Restricted, which blocks .ps1 files -
rem and keeps the window open so the result can be read.
rem
rem Passing any argument skips the prompt and forwards everything to install.ps1:
rem     install.cmd -Dest D:\applications\scd
rem     install.cmd -Dest D:\applications\scd -SkipBuild

setlocal EnableExtensions
set "HERE=%~dp0"
set "DEST=%USERPROFILE%\scd"

echo.
echo   scd installer
echo   source : %HERE%
if not "%~1"=="" goto args

echo.
set /p "DEST=  install into (press Enter for %DEST%) : "
set DEST=%DEST:"=%
if "%DEST%"=="" set "DEST=%USERPROFILE%\scd"
echo   target : %DEST%
echo.
powershell -NoProfile -ExecutionPolicy Bypass -File "%HERE%install.ps1" -Dest "%DEST%" -AddToPath -AddToProfile
goto report

:args
echo   target : taken from arguments %*
echo.
powershell -NoProfile -ExecutionPolicy Bypass -File "%HERE%install.ps1" -AddToPath -AddToProfile %*

:report
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