@echo off
rem Build the whole workspace. Release profile by default; pass --debug for
rem dev profile. Additional args are forwarded to `cargo build`.
setlocal EnableExtensions EnableDelayedExpansion

rem Capture script dir before any shift -- plain `shift` rewrites %0 too,
rem which would silently corrupt %~dp0 below.
set "SCRIPT_DIR=%~dp0"

call "%SCRIPT_DIR%_lib.cmd"

set "PROFILE=release"
set "CARGO_ARGS="

:parse
if "%~1"=="" goto parsed
if /i "%~1"=="--debug" (
    set "PROFILE=debug"
    shift
    goto parse
)
if /i "%~1"=="--release" (
    set "PROFILE=release"
    shift
    goto parse
)
if /i "%~1"=="-h" goto :usage
if /i "%~1"=="--help" goto :usage
set "CARGO_ARGS=!CARGO_ARGS! %~1"
shift
goto parse

:parsed
call "%SCRIPT_DIR%check-env.cmd"
if errorlevel 1 exit /b 1

cd /d "%REPO_ROOT%"
if /i "%PROFILE%"=="release" (
    echo ==^> Building workspace ^(release^)
    cargo build --workspace --release !CARGO_ARGS!
    if errorlevel 1 exit /b 1
    echo    * binaries at target\release\{web-server,lobby-server,world-server,map-server}.exe
) else (
    echo ==^> Building workspace ^(debug^)
    cargo build --workspace !CARGO_ARGS!
    if errorlevel 1 exit /b 1
    echo    * binaries at target\debug\{web-server,lobby-server,world-server,map-server}.exe
)
exit /b 0

:usage
echo Usage: %~nx0 [--debug^|--release] [cargo args...]
echo.
echo Builds all four workspace crates: common, web-server, lobby-server,
echo world-server, map-server. Default profile is release ^(matches deployment^).
echo Pass --debug for faster rebuilds during development. Any further args
echo are forwarded to cargo.
exit /b 0
