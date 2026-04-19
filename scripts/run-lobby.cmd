@echo off
rem Run the lobby server. Uses .\configs\lobby.toml by default -- the same
rem default the binary itself uses. Extra args are passed through to the
rem `lobby-server` CLI (e.g. --ip 0.0.0.0 --port 54994 --config path).
setlocal EnableExtensions EnableDelayedExpansion

set "SCRIPT_DIR=%~dp0"
call "%SCRIPT_DIR%_lib.cmd"

if /i "%PROFILE%"=="release" (
    set "FLAG=--release"
    set "BIN=%REPO_ROOT%\target\release\lobby-server.exe"
) else (
    set "FLAG="
    set "BIN=%REPO_ROOT%\target\debug\lobby-server.exe"
)

call "%SCRIPT_DIR%check-env.cmd"
if errorlevel 1 exit /b 1

cd /d "%REPO_ROOT%"
echo ==^> Building lobby-server ^(%PROFILE%^)
cargo build -p lobby-server %FLAG%
if errorlevel 1 exit /b 1

echo ==^> Starting lobby-server
if not exist "%BIN%" (
    1>&2 echo    X built binary not found at %BIN%
    exit /b 1
)
"%BIN%" %*
exit /b %errorlevel%
