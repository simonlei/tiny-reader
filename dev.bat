@echo off
setlocal EnableDelayedExpansion
REM ===========================================================================
REM  Tiny Reader - start the desktop client in dev mode
REM
REM  Prerequisites:
REM    * dependencies installed:  npm install
REM    * the server is running:   start-server.bat
REM
REM  NOTE: this file is intentionally ASCII-only. Non-ASCII bytes in a .bat
REM  file get garbled by cmd.exe and break parsing.
REM ===========================================================================

cd /d "%~dp0"

if not exist "node_modules" (
    echo [tiny-reader] node_modules not found, installing dependencies...
    call npm install
    if errorlevel 1 goto fail
)

REM ---------------------------------------------------------------- locate MSVC
set "VCVARS="
for %%D in ("C:\Program Files (x86)\Microsoft Visual Studio\2022" "C:\Program Files\Microsoft Visual Studio\2022") do (
    for %%E in (BuildTools Community Professional Enterprise) do (
        if exist "%%~D\%%E\VC\Auxiliary\Build\vcvarsall.bat" (
            set "VCVARS=%%~D\%%E\VC\Auxiliary\Build\vcvarsall.bat"
        )
    )
)

if defined VCVARS (
    echo [tiny-reader] Loading MSVC environment:
    echo   %VCVARS%
    call "%VCVARS%" x64 >nul
) else (
    echo [tiny-reader] WARNING: vcvarsall.bat not found.
    echo   If the build fails, open a "x64 Native Tools Command Prompt" and rerun this script.
)

echo [tiny-reader] Launching Tauri dev window...
call npm run tauri dev
set "RC=%ERRORLEVEL%"

if not "%RC%"=="0" (
    echo.
    echo [tiny-reader] Dev session exited with code %RC%
    pause
)
endlocal
exit /b 0

:fail
echo [tiny-reader] npm install failed.
pause
endlocal
exit /b 1
