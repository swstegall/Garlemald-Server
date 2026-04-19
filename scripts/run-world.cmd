@echo off
rem Run the world server. Extra args are forwarded to the `world-server` CLI.
setlocal EnableExtensions EnableDelayedExpansion

set "SCRIPT_DIR=%~dp0"
call "%SCRIPT_DIR%_lib.cmd"

if /i "%PROFILE%"=="release" (
    set "FLAG=--release"
    set "BIN=%REPO_ROOT%\target\release\world-server.exe"
) else (
    set "FLAG="
    set "BIN=%REPO_ROOT%\target\debug\world-server.exe"
)

call "%SCRIPT_DIR%check-env.cmd"
if errorlevel 1 exit /b 1

cd /d "%REPO_ROOT%"
echo ==^> Building world-server ^(%PROFILE%^)
cargo build -p world-server %FLAG%
if errorlevel 1 exit /b 1

echo ==^> Starting world-server
if not exist "%BIN%" (
    1>&2 echo    X built binary not found at %BIN%
    exit /b 1
)
"%BIN%" %*
exit /b %errorlevel%
