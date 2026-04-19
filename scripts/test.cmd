@echo off
rem Run the full verification suite: cargo test, clippy (warnings = error),
rem and rustfmt check. Individual stages can be skipped via flags.
setlocal EnableExtensions EnableDelayedExpansion

rem Capture script dir before any shift (plain `shift` also rewrites %0).
set "SCRIPT_DIR=%~dp0"

call "%SCRIPT_DIR%_lib.cmd"

set "RUN_TESTS=1"
set "RUN_CLIPPY=1"
set "RUN_FMT=1"

:parse
if "%~1"=="" goto parsed
if /i "%~1"=="--no-test"   ( set "RUN_TESTS=0"  & shift & goto parse )
if /i "%~1"=="--no-clippy" ( set "RUN_CLIPPY=0" & shift & goto parse )
if /i "%~1"=="--no-fmt"    ( set "RUN_FMT=0"    & shift & goto parse )
if /i "%~1"=="--only-test" ( set "RUN_CLIPPY=0" & set "RUN_FMT=0" & shift & goto parse )
if /i "%~1"=="-h"     goto :usage
if /i "%~1"=="--help" goto :usage
1>&2 echo    X unknown arg: %~1
exit /b 2

:parsed
call "%SCRIPT_DIR%check-env.cmd"
if errorlevel 1 exit /b 1

cd /d "%REPO_ROOT%"

if "%RUN_TESTS%"=="1" (
    echo ==^> cargo test --workspace
    cargo test --workspace
    if errorlevel 1 exit /b 1
    echo    * tests passed
)

if "%RUN_CLIPPY%"=="1" (
    echo ==^> cargo clippy ^(warnings = errors^)
    cargo clippy --workspace --all-targets -- -D warnings
    if errorlevel 1 exit /b 1
    echo    * clippy clean
)

if "%RUN_FMT%"=="1" (
    echo ==^> cargo fmt --check
    cargo fmt --all -- --check
    if errorlevel 1 exit /b 1
    echo    * formatting clean
)

echo    * all verification stages passed
exit /b 0

:usage
echo Usage: %~nx0 [--no-test^|--no-clippy^|--no-fmt^|--only-test]
echo.
echo Runs the full verification pipeline:
echo   1. cargo test --workspace
echo   2. cargo clippy --workspace --all-targets -- -D warnings
echo   3. cargo fmt --all -- --check
exit /b 0
