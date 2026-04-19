@echo off
rem Shared init for the garlemald-server .cmd scripts. Source via:
rem
rem   call "%~dp0_lib.cmd"
rem
rem Sets:
rem   REPO_ROOT          absolute path to the workspace root
rem   MIN_RUST_VERSION   the toolchain channel pinned in rust-toolchain.toml
rem   PROFILE            "release" unless caller already set "debug"
rem
rem Inline echo prefixes (consistent with the bash scripts):
rem   ==^>   step / "say"
rem    *    success / "ok"
rem    !    warning / "warn"
rem    X    error   / "err"   (write to stderr: 1>&2 echo    X message)

set "REPO_ROOT=%~dp0.."
for %%i in ("%REPO_ROOT%") do set "REPO_ROOT=%%~fi"
set "MIN_RUST_VERSION=1.95.0"
if not defined PROFILE set "PROFILE=release"
goto :eof
