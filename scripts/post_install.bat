@echo off
set "TARGET_DIR=%~1"
set "MAIN_EXE=%~2"

if "%TARGET_DIR%"=="" exit /b 0

:: example 1: register custom url scheme (e.g. myapp://)
reg add "HKCU\Software\Classes\myapp" /ve /t REG_SZ /d "URL:MyAwesomeApp Protocol" /f >nul 2>&1
reg add "HKCU\Software\Classes\myapp" /v "URL Protocol" /t REG_SZ /d "" /f >nul 2>&1
reg add "HKCU\Software\Classes\myapp\DefaultIcon" /ve /t REG_SZ /d "\"%TARGET_DIR%\app.exe\",0" /f >nul 2>&1
reg add "HKCU\Software\Classes\myapp\shell\open\command" /ve /t REG_SZ /d "\"%TARGET_DIR%\app.exe\" -- \"%%1\"" /f >nul 2>&1

:: example 2: associate custom file extension (e.g. .cool)
reg add "HKCU\Software\Classes\.cool" /ve /t REG_SZ /d "MyAwesomeApp.File" /f >nul 2>&1
reg add "HKCU\Software\Classes\MyAwesomeApp.File\shell\open\command" /ve /t REG_SZ /d "\"%TARGET_DIR%\app.exe\" \"%%1\"" /f >nul 2>&1

:: example 3: run silent dependency installer / vcredist
if exist "%TARGET_DIR%\redist\vc_redist.x64.exe" (
    "%TARGET_DIR%\redist\vc_redist.x64.exe" /install /passive /norestart >nul 2>&1
)

exit /b 0
