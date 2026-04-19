@echo off
rem Forcefully stop every garlemald server process. Useful as a manual
rem cleanup after Ctrl-C'ing run-all.cmd, or to clear stale processes
rem that survived an earlier run.
setlocal EnableExtensions

echo ==^> Stopping servers
taskkill /F /IM web-server.exe   >nul 2>&1
taskkill /F /IM lobby-server.exe >nul 2>&1
taskkill /F /IM world-server.exe >nul 2>&1
taskkill /F /IM map-server.exe   >nul 2>&1
echo    * all garlemald server processes stopped
exit /b 0
