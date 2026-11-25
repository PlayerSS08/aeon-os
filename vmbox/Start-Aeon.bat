@echo off
rem Starts the Aeon VirtualBox VM, then the Aeon Wi-Fi helper (VirtualBox only, no QEMU).
rem First time: powershell -ExecutionPolicy Bypass -File setup-vbox.ps1
rem Usage: Start-Aeon.bat [gui^|headless] [VM name]   (defaults: gui, Aeon-Latest)
rem The VM serves Aeon's COM2 on port 4555; the helper connects to it and closes by itself after the VM is off.
cd /d "%~dp0"
set T=%~1
if "%T%"=="" set T=gui
set VM=%~2
if "%VM%"=="" set VM=Aeon-Latest
"C:\Program Files\Oracle\VirtualBox\VBoxManage.exe" startvm "%VM%" --type %T%
if errorlevel 1 (echo Aeon VM "%VM%" did not start. Run setup-vbox.ps1 first. & pause & exit /b 1)
start "Aeon Wi-Fi helper" /min powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0wifi-host-vbox.ps1" -VmName "%VM%"
