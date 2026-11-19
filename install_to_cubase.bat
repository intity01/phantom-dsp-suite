@echo off
setlocal enabledelayedexpansion

echo =======================================================
echo   Phantom EQ - VST3 Installer for Cubase / Windows
echo =======================================================

:: 1. Check if Cubase is running
tasklist /FI "IMAGENAME eq Cubase*" 2>NUL | find /I /N "Cubase" >NUL
if "%ERRORLEVEL%"=="0" (
    echo [WARNING] Cubase is currently running!
    echo Please save your project and close Cubase to unlock the VST3 file.
    pause
)

:: 2. Determine target directory (User Local VST3 or Common VST3)
set "TARGET_DIR=%LOCALAPPDATA%\Programs\Common\VST3\Phantom EQ.vst3\Contents\x86_64-win"
set "SYSTEM_DIR=%CommonProgramFiles%\VST3\Phantom EQ.vst3\Contents\x86_64-win"
set "SOURCE_FILE=%~dp0dist\windows\vst3\Phantom EQ.vst3\Contents\x86_64-win\Phantom EQ.vst3"

if not exist "%SOURCE_FILE%" (
    echo [ERROR] Compiled plugin not found at:
    echo "%SOURCE_FILE%"
    pause
    exit /b 1
)

echo.
echo Installing to User VST3 directory:
echo "%TARGET_DIR%"
mkdir "%TARGET_DIR%" 2>NUL
copy /Y "%SOURCE_FILE%" "%TARGET_DIR%\Phantom EQ.vst3"

if "%ERRORLEVEL%"=="0" (
    echo.
    echo [SUCCESS] Phantom EQ.vst3 successfully installed!
    echo Location: %TARGET_DIR%\Phantom EQ.vst3
) else (
    echo [FAILED] Could not copy to User VST3 directory. Trying System VST3 (Requires Admin)...
    mkdir "%SYSTEM_DIR%" 2>NUL
    copy /Y "%SOURCE_FILE%" "%SYSTEM_DIR%\Phantom EQ.vst3"
)

echo.
echo =======================================================
echo Ready! You can now open Cubase and insert Phantom EQ.
echo =======================================================
pause
