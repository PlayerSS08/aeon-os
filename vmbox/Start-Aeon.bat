@echo off
title Starting Aeon OS in VirtualBox (GUI)
echo ========================================================
echo          Starting Aeon OS in Oracle VM VirtualBox
echo ========================================================
echo.

set VBOX_MANAGE="C:\Program Files\Oracle\VirtualBox\VBoxManage.exe"
set VBOX_VM="C:\Program Files\Oracle\VirtualBox\VirtualBoxVM.exe"
set VBOX_APP="C:\Program Files\Oracle\VirtualBox\VirtualBox.exe"

rem Check if VM is already running
%VBOX_MANAGE% list runningvms | findstr /i "Aeon-VM" >nul
if %ERRORLEVEL% equ 0 (
    echo Aeon-VM is already running. Bringing window to foreground...
    start "" %VBOX_APP%
    goto end
)

echo Starting Aeon-VM in GUI mode...
start "" %VBOX_VM% --startvm "Aeon-VM" --type gui

:end
echo.
echo ========================================================
echo  Aeon VM window is now open on your desktop!
echo ========================================================
timeout /t 3 >nul
