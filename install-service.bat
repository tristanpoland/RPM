@echo off
echo Installing RPM as Windows Service...
echo This requires administrator privileges.
echo.

net session >nul 2>&1
if %errorLevel% == 0 (
    echo Running as Administrator - installing service...
    rpm daemon
    pause
) else (
    echo ERROR: This script must be run as Administrator!
    echo.
    echo Right-click this file and select "Run as administrator"
    echo Or run from an Administrator command prompt: rpm daemon
    pause
)