@echo off
set "TARGET_DIR=%~1"
set "MAIN_EXE=%~2"

:: example 1: remove custom url scheme
reg delete "HKCU\Software\Classes\myapp" /f >nul 2>&1

:: example 2: remove custom file association
reg delete "HKCU\Software\Classes\.cool" /f >nul 2>&1
reg delete "HKCU\Software\Classes\MyAwesomeApp.File" /f >nul 2>&1

:: example 3: clean up user configuration or logs cache
if exist "%LOCALAPPDATA%\MyAwesomeApp\Cache" (
    rmdir /s /q "%LOCALAPPDATA%\MyAwesomeApp\Cache" >nul 2>&1
)

exit /b 0
