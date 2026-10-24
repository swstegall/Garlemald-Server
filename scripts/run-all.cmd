@echo off
rem Build all four servers once, then launch them together. Each process's
rem stdout / stderr is tee'd to .\logs\{web,lobby,world,map}.log. Press any
rem key in this window (or close it) to stop the whole stack.
rem
rem Note: unlike the bash run-all.sh, this script cannot trap Ctrl-C cleanly
rem in batch -- if you Ctrl-C here and answer "Y" to the "Terminate batch
rem job?" prompt, the four child server processes are leaked. Either press
rem any key (preferred) or run scripts\stop-all.cmd from another window.
setlocal EnableExtensions EnableDelayedExpansion

set "SCRIPT_DIR=%~dp0"
call "%SCRIPT_DIR%_lib.cmd"

if /i "%PROFILE%"=="release" (
    set "FLAG=--release"
    set "BIN_DIR=%REPO_ROOT%\target\release"
) else (
    set "FLAG="
    set "BIN_DIR=%REPO_ROOT%\target\debug"
)

call "%SCRIPT_DIR%check-env.cmd"
if errorlevel 1 exit /b 1

cd /d "%REPO_ROOT%"

rem Pre-flight: every service's port must be free. A stack that is still
rem running would otherwise make the new services die on "address already
rem in use" while the old ones keep serving. (#211)
set "_BUSY=0"
call :check_port web   54993
call :check_port lobby 54994
call :check_port world 54992
call :check_port map   1989
if "%_BUSY%"=="1" (
    1>&2 echo    X a previous stack is probably still running; run scripts\stop-all.cmd and try again
    exit /b 1
)

echo ==^> Building workspace ^(%PROFILE%^)
cargo build --workspace %FLAG%
if errorlevel 1 exit /b 1

if not exist "%REPO_ROOT%\logs" mkdir "%REPO_ROOT%\logs"

rem Default the packet-log directory under .\logs and ensure it exists. set (under
rem the active setlocal) is inherited by the child servers launched below, matching
rem run-all.sh's exported default; an externally preset value is left untouched.
if not defined GARLEMALD_PACKET_LOG_DIR set "GARLEMALD_PACKET_LOG_DIR=%REPO_ROOT%\logs\packet-logs"
if not exist "%GARLEMALD_PACKET_LOG_DIR%" mkdir "%GARLEMALD_PACKET_LOG_DIR%"

rem Web first (so signup is reachable before the rest of the stack is warm),
rem then lobby / world / map. The processes are independent -- no startup
rem barrier is required between them.
call :start_one web-server   web
call :start_one lobby-server lobby
call :start_one world-server world
call :start_one map-server   map

rem Startup gate: a service that dies on a bad config, DB error or bind
rem failure does so within a few seconds; report it and stop the rest. (#211)
timeout /t 5 /nobreak >nul
set "_DEAD="
for %%N in (web-server lobby-server world-server map-server) do (
    tasklist /FI "IMAGENAME eq %%N.exe" 2>nul | find /i "%%N.exe" >nul || set "_DEAD=!_DEAD! %%N"
)
if defined _DEAD (
    1>&2 echo    X exited during startup:!_DEAD! -- see the matching logs\*.log and logs\*.err.log
    call :stop_all
    exit /b 1
)

echo.
echo ==^> All four servers running.
echo    * tail logs with: powershell -Command "Get-Content -Wait logs\*.log"
echo    * press any key to stop, or run scripts\stop-all.cmd from another window.
echo.
pause >nul

call :stop_all
exit /b 0

:check_port
rem %1 = service, %2 = default port; the first "port = N" in configs\%1.toml wins
set "_PORT=%~2"
for /f "tokens=2 delims== " %%P in ('findstr /b /r /c:"port *=" "%REPO_ROOT%\configs\%~1.toml" 2^>nul') do if "!_PORT!"=="%~2" set "_PORT=%%P"
for /f "tokens=5" %%I in ('netstat -ano ^| findstr /r /c:":!_PORT! .*LISTENING"') do (
    1>&2 echo    X %~1 port !_PORT! is already in use by pid %%I
    set "_BUSY=1"
)
goto :eof

:start_one
rem %1 = exe basename (without .exe), %2 = log basename
set "_NAME=%~1"
set "_LOG=%REPO_ROOT%\logs\%~2.log"
set "_LOG_ERR=%REPO_ROOT%\logs\%~2.err.log"
set "_BIN=%BIN_DIR%\%_NAME%.exe"
echo ==^> Starting %_NAME% ^(log: %_LOG%^)
if not exist "%_BIN%" (
    1>&2 echo    X built binary not found at %_BIN%
    exit /b 1
)
rem Start-Process spawns a detached process and returns immediately, with
rem stdout and stderr captured to separate files (PowerShell limitation --
rem they cannot share a single file).
powershell.exe -NoProfile -Command ^
    "Start-Process -FilePath '%_BIN%' -WorkingDirectory '%REPO_ROOT%' -RedirectStandardOutput '%_LOG%' -RedirectStandardError '%_LOG_ERR%' -WindowStyle Hidden" >nul
if errorlevel 1 (
    1>&2 echo    X failed to launch %_NAME%
    exit /b 1
)
echo    * %_NAME% launched
goto :eof

:stop_all
echo ==^> Stopping servers
taskkill /F /IM web-server.exe   >nul 2>&1
taskkill /F /IM lobby-server.exe >nul 2>&1
taskkill /F /IM world-server.exe >nul 2>&1
taskkill /F /IM map-server.exe   >nul 2>&1
echo    * all servers stopped
goto :eof
