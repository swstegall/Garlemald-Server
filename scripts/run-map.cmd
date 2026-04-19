@echo off
rem Run the map server. Extra args are forwarded to the `map-server` CLI.
setlocal EnableExtensions EnableDelayedExpansion

set "SCRIPT_DIR=%~dp0"
call "%SCRIPT_DIR%_lib.cmd"

if /i "%PROFILE%"=="release" (
    set "FLAG=--release"
    set "BIN=%REPO_ROOT%\target\release\map-server.exe"
) else (
    set "FLAG="
    set "BIN=%REPO_ROOT%\target\debug\map-server.exe"
)

call "%SCRIPT_DIR%check-env.cmd"
if errorlevel 1 exit /b 1

cd /d "%REPO_ROOT%"
echo ==^> Building map-server ^(%PROFILE%^)
cargo build -p map-server %FLAG%
if errorlevel 1 exit /b 1

echo ==^> Starting map-server
if not exist "%BIN%" (
    1>&2 echo    X built binary not found at %BIN%
    exit /b 1
)
"%BIN%" %*
exit /b %errorlevel%
