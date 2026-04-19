@echo off
rem Verify that the local machine can build garlemald-server. Exits 0 on
rem success, non-zero with a list of missing tools otherwise.
setlocal EnableExtensions EnableDelayedExpansion

call "%~dp0_lib.cmd"

echo ==^> Checking build environment

set "FAIL=0"

where cargo >nul 2>&1
if errorlevel 1 (
    1>&2 echo    X missing required command: cargo
    1>&2 echo    ! install Rust from https:^/^/rustup.rs
    set "FAIL=1"
)

where rustc >nul 2>&1
if errorlevel 1 (
    1>&2 echo    X missing required command: rustc
    1>&2 echo    ! install Rust from https:^/^/rustup.rs
    set "FAIL=1"
)

if "!FAIL!"=="0" (
    set "RUST_VERSION="
    for /f "tokens=2" %%v in ('rustc --version 2^>nul') do set "RUST_VERSION=%%v"
    if defined RUST_VERSION (
        echo    * rustc !RUST_VERSION! [workspace pins !MIN_RUST_VERSION! via rust-toolchain.toml]
    ) else (
        1>&2 echo    X rustc returned no version string
        set "FAIL=1"
    )
)

cargo clippy --version >nul 2>&1
if errorlevel 1 (
    echo    ! clippy not installed -- run: rustup component add clippy
) else (
    echo    * clippy available
)

cargo fmt --version >nul 2>&1
if errorlevel 1 (
    echo    ! rustfmt not installed -- run: rustup component add rustfmt
) else (
    echo    * rustfmt available
)

rem MSVC C compiler (needed by cc-rs for mlua's vendored Lua, libsqlite3-sys, etc.)
set "VSWHERE=%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe"
set "FOUND_CL="
where cl.exe >nul 2>&1
if not errorlevel 1 (
    echo    * cl.exe on PATH
) else if exist "!VSWHERE!" (
    for /f "usebackq delims=" %%c in (`""!VSWHERE!" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -find VC\Tools\MSVC\**\bin\Hostx64\x64\cl.exe"`) do set "FOUND_CL=%%c"
    if defined FOUND_CL (
        echo    * MSVC found at !FOUND_CL!
    ) else (
        1>&2 echo    X no MSVC C compiler -- install Visual Studio Build Tools with the C++ workload
        set "FAIL=1"
    )
) else (
    1>&2 echo    X no MSVC and no vswhere -- install Visual Studio Build Tools with the C++ workload
    set "FAIL=1"
)

if not exist "%REPO_ROOT%\Cargo.toml" (
    1>&2 echo    X Cargo.toml not found at %REPO_ROOT% -- script must live under ^<repo^>\scripts
    set "FAIL=1"
)

if "!FAIL!"=="0" (
    echo    * environment ready
    endlocal & exit /b 0
)
1>&2 echo    X fix the issues above and re-run
endlocal & exit /b 1
