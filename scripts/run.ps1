param(
    [Parameter(Position = 0)]
    [string]$BinaryPath
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$Workspace = Split-Path -Parent $ScriptDir
$DiskDir = Join-Path $Workspace "build\disk"
$TargetEfi = Join-Path $DiskDir "EFI\BOOT\BOOTX64.EFI"

if (-not (Test-Path (Split-Path -Parent $TargetEfi))) {
    New-Item -ItemType Directory -Path (Split-Path -Parent $TargetEfi) -Force | Out-Null
}

if ($BinaryPath -and (Test-Path $BinaryPath)) {
    Write-Host "[Aeon] Staging $BinaryPath -> $TargetEfi" -ForegroundColor Cyan
    Copy-Item -Path $BinaryPath -Destination $TargetEfi -Force
}

# Find QEMU binary
$QemuCandidates = @(
    "C:\Users\SREEHARAN\qemu\qemu-system-x86_64.exe",
    "G:\Aeon\tools\qemu\qemu-system-x86_64.exe",
    "C:\Program Files\qemu\qemu-system-x86_64.exe"
)

$QemuBin = $null
foreach ($cand in $QemuCandidates) {
    if (Test-Path $cand) {
        $QemuBin = $cand
        break
    }
}

if (-not $QemuBin) {
    $cmd = Get-Command qemu-system-x86_64 -ErrorAction SilentlyContinue
    if ($cmd) {
        $QemuBin = $cmd.Source
    }
}

if (-not $QemuBin) {
    Write-Error "[Aeon:ERR] qemu-system-x86_64.exe not found."
    exit 1
}

# Find OVMF firmware
$QemuDir = Split-Path -Parent $QemuBin
$OvmfCandidates = @(
    (Join-Path $QemuDir "share\edk2-x86_64-code.fd"),
    (Join-Path $QemuDir "edk2-x86_64-code.fd"),
    (Join-Path $QemuDir "share\OVMF.fd"),
    (Join-Path $Workspace "ovmf\OVMF.fd"),
    (Join-Path $Workspace "build\OVMF.fd")
)

$OvmfPath = $null
foreach ($cand in $OvmfCandidates) {
    if (Test-Path $cand) {
        $OvmfPath = $cand
        break
    }
}

if (-not $OvmfPath) {
    Write-Error "[Aeon:ERR] OVMF UEFI firmware (.fd) not found."
    exit 1
}

Write-Host "[Aeon] Launching QEMU with OVMF ($OvmfPath)..." -ForegroundColor Green

$SerialLog = Join-Path $Workspace "build\serial.log"

& $QemuBin `
    -cpu max `
    -drive "if=pflash,format=raw,readonly=on,file=$OvmfPath" `
    -drive "format=raw,file=fat:rw:$DiskDir" `
    -serial stdio `
    -net none `
    -m 1024M | Tee-Object -FilePath $SerialLog
