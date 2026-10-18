@echo off
setlocal enabledelayedexpansion

echo building drop installer...

set "RUST_BIN=D:\Other\Projects\TelegramAir-Win\external\ThirdParty\rust\rustup\toolchains\1.96.1-x86_64-pc-windows-msvc\bin"
set "MSVC_BIN=C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC\14.44.35229\bin\Hostx64\x64"
set "SDK_BIN=C:\Program Files (x86)\Windows Kits\10\bin\10.0.22621.0\x64"

set "PATH=%RUST_BIN%;%MSVC_BIN%;%SDK_BIN%;%PATH%"
set "RUSTC=%RUST_BIN%\rustc.exe"

cargo build --release
if errorlevel 1 (
    echo build failed.
    exit /b 1
)

if not exist "dist" mkdir "dist"

set "OUT_NAME=setup.exe"
for /f "tokens=2 delims==" %%i in ('findstr /i "output_name" drop.toml') do (
    for /f "tokens=1 delims= " %%j in ("%%~i") do (
        set "TEMP_NAME=%%j"
        set "TEMP_NAME=!TEMP_NAME:"=!"
        set "OUT_NAME=!TEMP_NAME!"
    )
)

copy /y "target\release\drop.exe" "dist\!OUT_NAME!" >nul
echo built: dist\!OUT_NAME!

echo done.
exit /b 0
