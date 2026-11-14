# Launch Aeon OS Virtual Machine in Oracle VirtualBox
$vbox = "C:\Program Files\Oracle\VirtualBox\VirtualBoxVM.exe"
if (-not (Test-Path $vbox)) {
    Write-Error "VirtualBox not found at $vbox"
    exit 1
}
Write-Host "Launching AEON-VM..." -ForegroundColor Cyan
Start-Process -FilePath $vbox -ArgumentList '--startvm "AEON-VM"'
