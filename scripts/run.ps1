param(
    [Parameter(Position = 0)]
    [string]$BinaryPath
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$Workspace = Split-Path -Parent $ScriptDir
$DiskDir = Join-Path $Workspace "build\disk"
$TargetEfi = Join-Path $DiskDir "EFI\BOOT\BOOTX64.EFI"
$VBoxManage = "C:\Program Files\Oracle\VirtualBox\VBoxManage.exe"

if (-not (Test-Path (Split-Path -Parent $TargetEfi))) {
    New-Item -ItemType Directory -Path (Split-Path -Parent $TargetEfi) -Force | Out-Null
}

if ($BinaryPath -and (Test-Path $BinaryPath)) {
    Write-Host "[Aeon] Staging $BinaryPath -> $TargetEfi" -ForegroundColor Cyan
    Copy-Item -Path $BinaryPath -Destination $TargetEfi -Force
}

if (-not (Test-Path $VBoxManage)) {
    Write-Error "[Aeon:ERR] VirtualBox VBoxManage.exe not found at $VBoxManage"
    exit 1
}

# Stop VM first if running so VDI lock is released
$vmState = (& $VBoxManage showvminfo "Aeon" --machinereadable 2>$null | Select-String 'VMState="running"')
if ($vmState) {
    Write-Host "[Aeon] Stopping active VM session..." -ForegroundColor Yellow
    & $VBoxManage controlvm "Aeon" poweroff | Out-Null
    Start-Sleep -Seconds 2
}

# Generate VDI with persistent UUID
Write-Host "[Aeon] Synthesizing FAT16 and updating VirtualBox VDI..." -ForegroundColor Cyan
python "$Workspace\build\update_vdi.py"
if ($LASTEXITCODE -ne 0) {
    Write-Error "[Aeon:ERR] Failed to update aeon.vdi"
    exit 1
}

# Launch VM
Write-Host "[Aeon] Booting Aeon in Oracle VirtualBox (UEFI x86_64)..." -ForegroundColor Green
& $VBoxManage startvm "Aeon"
