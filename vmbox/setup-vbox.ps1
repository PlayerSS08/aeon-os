# Creates the Aeon VirtualBox VM from this folder (VirtualBox only, no QEMU).
#
#   powershell -ExecutionPolicy Bypass -File vmbox\setup-vbox.ps1            build disks + create VM "Aeon-Latest"
#   powershell -ExecutionPolicy Bypass -File vmbox\setup-vbox.ps1 -Force     replace an existing VM of that name
#
# Then start it with vmbox\Start-Aeon.bat (VM + Wi-Fi helper).
param(
    [string]$VmName = 'Aeon-Latest',
    [switch]$Force,
    [switch]$NewUuids   # fresh disk UUIDs, for a second Aeon VM on the same machine
)
$ErrorActionPreference = 'Stop'
$vb = 'C:\Program Files\Oracle\VirtualBox\VBoxManage.exe'
if (-not (Test-Path $vb)) { throw "VirtualBox not found at $vb (install: winget install --id Oracle.VirtualBox -e --source winget)" }
$here = $PSScriptRoot

function Invoke-VBox { & $vb @args; if ($LASTEXITCODE -ne 0) { throw "VBoxManage $($args -join ' ') failed" } }

$exists = (& $vb list vms) -match "^`"$([regex]::Escape($VmName))`" "
if ($exists) {
    if (-not $Force) { Write-Host "VM '$VmName' already exists. Use -Force to replace it, or just run Start-Aeon.bat."; exit 0 }
    $state = ((& $vb showvminfo $VmName --machinereadable) -match '^VMState=') -replace '^VMState="(.*)"$', '$1'
    if ($state -notin @('poweroff', 'aborted')) {
        if ($state -eq 'saved') { & $vb discardstate $VmName } else { & $vb controlvm $VmName poweroff; Start-Sleep 3 }
    }
    # Detach the disks so the rebuilt .vdi files can replace them, then unregister (the VM's files are deleted;
    # the disks in this folder are rebuilt below).
    & $vb storageattach $VmName --storagectl SATA --port 0 --device 0 --medium none 2>$null
    & $vb storageattach $VmName --storagectl SATA --port 1 --device 0 --medium none 2>$null
    foreach ($d in 'Aeon-Boot.vdi', 'Aeon-DemoDisk.vdi') { & $vb closemedium disk (Join-Path $here $d) 2>$null }
    Invoke-VBox unregistervm $VmName --delete
}

# 1. Disks (python 3, no third-party packages).
$pyArgs = @('-I', (Join-Path $here 'build_disks.py'))
if ($NewUuids) { $pyArgs += '--new-uuids' }
& python @pyArgs
if ($LASTEXITCODE -ne 0) { throw 'build_disks.py failed' }

# 2. The VM.
Invoke-VBox createvm --name $VmName --ostype Other_64 --basefolder (Join-Path $here 'VMs') --register
Invoke-VBox modifyvm $VmName --firmware efi64 --memory 1024 --cpus 2 --ioapic on --vram 64 `
    --large-pages on --pae on --nested-hw-virt on --audio-enabled off `
    --boot1 disk --boot2 none --boot3 none --boot4 none `
    --nic1 nat --nictype1 82540EM --nat-dns-proxy1 on --nat-dns-host-resolver1 on `
    --uart1 0x3F8 4 --uartmode1 file (Join-Path $here 'aeon-serial.log') `
    --uart2 0x2F8 3 --uartmode2 tcpserver 4555
# Without this VirtualBox 7.2's UEFI firmware crashes (#GP in CpuDxe) before Aeon even loads.
Invoke-VBox setextradata $VmName VBoxInternal/CPUM/MtrrVarCountIsVirtual 0
Invoke-VBox setextradata $VmName VBoxInternal2/EfiGraphicsResolution 1280x800
Invoke-VBox storagectl $VmName --name SATA --add sata --controller IntelAhci --portcount 2 --bootable on
Invoke-VBox storageattach $VmName --storagectl SATA --port 0 --device 0 --type hdd --medium (Join-Path $here 'Aeon-Boot.vdi')
Invoke-VBox storageattach $VmName --storagectl SATA --port 1 --device 0 --type hdd --medium (Join-Path $here 'Aeon-DemoDisk.vdi')

Write-Host ""
Write-Host "VM '$VmName' is ready. Start it with:  vmbox\Start-Aeon.bat"
