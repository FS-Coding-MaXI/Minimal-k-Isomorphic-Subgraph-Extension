@echo off
setlocal enabledelayedexpansion

REM Stop on errors
set ERRORLEVEL=0

REM Check if cargo exists
where cargo >nul 2>nul
if errorlevel 1 (
    echo Cargo not found. Installing Rust...

    set RUSTUP_INSTALLER=rustup-init.exe
    powershell -Command "Invoke-WebRequest https://win.rustup.rs -OutFile %RUSTUP_INSTALLER%"
    %RUSTUP_INSTALLER% -y

    REM Update PATH for current session
    set "CARGO_BIN=%USERPROFILE%\.cargo\bin"
    echo %PATH% | find /I "%CARGO_BIN%" >nul
    if errorlevel 1 (
        set "PATH=%PATH%;%CARGO_BIN%"
    )
)

REM Build only (no run)
cargo build --release --bin solver
if errorlevel 1 (
    echo Cargo build failed
    exit /b 1
)

REM Paths
set SOURCE=target\release\solver.exe
set DEST=solver.exe

if not exist "%SOURCE%" (
    echo Build succeeded but binary not found at %SOURCE%
    exit /b 1
)

REM Copy binary
copy /Y "%SOURCE%" "%DEST%" >nul

echo Binary copied to %DEST%
endlocal

