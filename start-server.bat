@echo off
setlocal EnableDelayedExpansion
REM ===========================================================================
REM  Tiny Reader - start the data server
REM
REM  NOTE: this file is intentionally ASCII-only. Non-ASCII bytes in a .bat
REM  file get garbled by cmd.exe and break parsing.
REM ===========================================================================

cd /d "%~dp0"

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

if not exist "server\config.toml" (
    echo [tiny-reader] No server\config.toml yet - it will be generated on first run.
)

echo [tiny-reader] Starting server on http://127.0.0.1:8787
echo [tiny-reader] Press Ctrl+C to stop.
echo.

cargo run --manifest-path server\Cargo.toml
set "RC=%ERRORLEVEL%"

if not "%RC%"=="0" (
    echo.
    echo [tiny-reader] Server exited with code %RC%
    pause
)
endlocal
