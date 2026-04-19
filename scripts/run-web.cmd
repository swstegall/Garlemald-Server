@echo off
rem Run the web (login/signup) server. Uses .\configs\web.toml by default --
rem the same default the binary itself uses. Extra args are passed through
rem to the `web-server` CLI (e.g. --ip 0.0.0.0 --port 54993 --config path).
setlocal EnableExtensions EnableDelayedExpansion

set "SCRIPT_DIR=%~dp0"
call "%SCRIPT_DIR%_lib.cmd"

if /i "%PROFILE%"=="release" (
    set "FLAG=--release"
    set "BIN=%REPO_ROOT%\target\release\web-server.exe"
) else (
    set "FLAG="
    set "BIN=%REPO_ROOT%\target\debug\web-server.exe"
)

call "%SCRIPT_DIR%check-env.cmd"
if errorlevel 1 exit /b 1

cd /d "%REPO_ROOT%"
echo ==^> Building web-server ^(%PROFILE%^)
cargo build -p web-server %FLAG%
if errorlevel 1 exit /b 1

echo ==^> Starting web-server
if not exist "%BIN%" (
    1>&2 echo    X built binary not found at %BIN%
    exit /b 1
)
"%BIN%" %*
exit /b %errorlevel%
