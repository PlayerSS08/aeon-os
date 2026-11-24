# Aeon Wi-Fi helper (Windows host side of src/wifihost.rs).
#
# Aeon in QEMU has no Wi-Fi radio of its own; like every VM it reaches the network through the host. This helper
# lets Aeon's boot menu and `wifi` command scan for real Wi-Fi networks and choose which one Windows joins. It
# listens on 127.0.0.1:4555; QEMU connects Aeon's second serial port (COM2) to it. Protocol (tab-separated lines):
#   HELLO                      -> AEONWIFI 1
#   SCAN                       -> NET <signal> <secure 0/1> <current 0/1> <ssid> ... then END   (or ERR <text>)
#   CONNECT <ssid> <password>  -> OK <text> | ERR <text>
# Passwords are never printed or kept; a temporary profile file is deleted at once. If a wrong password replaces a
# saved Windows profile, the old profile is put back.
param([int]$Port = 4555)
$ErrorActionPreference = 'Continue'

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class AeonWlanScan {
    [DllImport("wlanapi.dll")] static extern int WlanOpenHandle(uint v, IntPtr r, out uint neg, out IntPtr h);
    [DllImport("wlanapi.dll")] static extern int WlanEnumInterfaces(IntPtr h, IntPtr r, out IntPtr list);
    [DllImport("wlanapi.dll")] static extern int WlanScan(IntPtr h, ref Guid g, IntPtr ssid, IntPtr ie, IntPtr r);
    [DllImport("wlanapi.dll")] static extern void WlanFreeMemory(IntPtr p);
    [DllImport("wlanapi.dll")] static extern int WlanCloseHandle(IntPtr h, IntPtr r);
    // Asks every Wi-Fi interface for a fresh scan. Returns how many accepted.
    public static int Scan() {
        uint neg; IntPtr h;
        if (WlanOpenHandle(2, IntPtr.Zero, out neg, out h) != 0) return -1;
        int ok = 0; IntPtr list;
        if (WlanEnumInterfaces(h, IntPtr.Zero, out list) == 0) {
            int count = Marshal.ReadInt32(list);
            for (int i = 0; i < count; i++) {
                // WLAN_INTERFACE_INFO_LIST: two DWORDs, then items of GUID + WCHAR[256] + state = 532 bytes.
                IntPtr p = new IntPtr(list.ToInt64() + 8 + i * 532);
                Guid g = (Guid)Marshal.PtrToStructure(p, typeof(Guid));
                if (WlanScan(h, ref g, IntPtr.Zero, IntPtr.Zero, IntPtr.Zero) == 0) ok++;
            }
            WlanFreeMemory(list);
        }
        WlanCloseHandle(h, IntPtr.Zero);
        return ok;
    }
}
"@

function Get-Current {
    $state = ''; $ssid = ''
    foreach ($l in (netsh wlan show interfaces)) {
        if ($l -match '^\s+State\s+:\s*(.*)$') { $state = $matches[1].Trim() }
        elseif ($l -match '^\s+SSID\s+:\s*(.*)$') { $ssid = $matches[1].Trim() }
    }
    if ($state -eq 'connected') { return $ssid } else { return '' }
}

function Get-Networks {
    $nets = @(); $cur = $null
    foreach ($l in (netsh wlan show networks mode=bssid)) {
        if ($l -match '^SSID \d+ :\s*(.*)$') {
            if ($cur) { $nets += $cur }
            $cur = [pscustomobject]@{ Ssid = $matches[1].Trim(); Auth = ''; Signal = 0 }
        } elseif ($cur -and $l -match '^\s+Authentication\s+:\s*(.*)$') { $cur.Auth = $matches[1].Trim() }
        elseif ($cur -and $l -match '^\s+Signal\s+:\s*(\d+)%') { $s = [int]$matches[1]; if ($s -gt $cur.Signal) { $cur.Signal = $s } }
    }
    if ($cur) { $nets += $cur }
    $nets | Where-Object { $_.Ssid -and [Text.Encoding]::UTF8.GetByteCount($_.Ssid) -le 32 } | Sort-Object Signal -Descending | Select-Object -First 12
}

function Invoke-Scan {
    [void][AeonWlanScan]::Scan()
    Start-Sleep -Seconds 4   # Windows fills its list while the scan runs
    $current = Get-Current
    $lines = @()
    foreach ($n in Get-Networks) {
        $secure = if ($n.Auth -eq 'Open') { 0 } else { 1 }
        $isCur = if ($n.Ssid -eq $current) { 1 } else { 0 }
        $lines += "NET`t$($n.Signal)`t$secure`t$isCur`t$($n.Ssid)"
    }
    Write-Host "  scan: $($lines.Count) network(s)"
    return $lines + 'END'
}

function New-ProfileXml($ssid, $auth, $pass) {
    $e = [Security.SecurityElement]::Escape($ssid)
    if ($auth -eq 'Open') {
        $sec = '<authEncryption><authentication>open</authentication><encryption>none</encryption><useOneX>false</useOneX></authEncryption>'
    } else {
        $a = if ($auth -eq 'WPA3-Personal') { 'WPA3SAE' } else { 'WPA2PSK' }
        $k = [Security.SecurityElement]::Escape($pass)
        $sec = "<authEncryption><authentication>$a</authentication><encryption>AES</encryption><useOneX>false</useOneX></authEncryption><sharedKey><keyType>passPhrase</keyType><protected>false</protected><keyMaterial>$k</keyMaterial></sharedKey>"
    }
    return "<?xml version=`"1.0`"?><WLANProfile xmlns=`"http://www.microsoft.com/networking/WLAN/profile/v1`"><name>$e</name><SSIDConfig><SSID><name>$e</name></SSID></SSIDConfig><connectionType>ESS</connectionType><connectionMode>auto</connectionMode><MSM><security>$sec</security></MSM></WLANProfile>"
}

function Invoke-Connect($ssid, $pass) {
    if ((Get-Current) -eq $ssid) { Write-Host "  connect: already on '$ssid'"; return "OK`talready connected" }
    $net = Get-Networks | Where-Object { $_.Ssid -eq $ssid } | Select-Object -First 1
    if (-not $net) { return "ERR`t'$ssid' is not in range any more; scan again" }
    $tmp = Join-Path $env:TEMP ("aeon-wifi-" + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory $tmp | Out-Null
    $backup = $null
    $previous = Get-Current
    $added = $false
    $ok = $false
    try {
        $known = (netsh wlan show profiles) -match ":\s*$([regex]::Escape($ssid))\s*$"
        if ($net.Auth -eq 'Open' -or $pass) {
            if ($known) {
                netsh wlan export profile name="$ssid" key=clear folder="$tmp" | Out-Null
                $backup = Get-ChildItem $tmp -Filter *.xml | Select-Object -First 1
            }
            $file = Join-Path $tmp 'new.xml'
            Set-Content -Path $file -Value (New-ProfileXml $ssid $net.Auth $pass) -Encoding UTF8
            $r = netsh wlan add profile filename="$file" user=current
            Remove-Item $file -Force
            if ($LASTEXITCODE -ne 0) { return "ERR`tWindows refused the network settings" }
            $added = -not $known
        } elseif (-not $known) {
            return "ERR`ta password is needed for '$ssid'"
        }
        Write-Host "  connect: joining '$ssid'..."
        netsh wlan connect name="$ssid" ssid="$ssid" | Out-Null
        for ($i = 0; $i -lt 25; $i++) {
            Start-Sleep -Seconds 1
            if ((Get-Current) -eq $ssid) { Write-Host "  connect: on '$ssid'"; $backup = $null; $ok = $true; return "OK`tconnected" }
        }
        Write-Host "  connect: failed"
        return "ERR`tcould not join '$ssid' (wrong password?)"
    } finally {
        if ($backup) {
            netsh wlan add profile filename="$($backup.FullName)" user=current | Out-Null   # put the working profile back
            Write-Host "  restored the saved Windows profile for '$ssid'"
        }
        if (-not $ok -and $added) {
            netsh wlan delete profile name="$ssid" | Out-Null   # do not leave a profile with a wrong password behind
        }
        if (-not $ok -and $previous -and $previous -ne $ssid) {
            netsh wlan connect name="$previous" ssid="$previous" | Out-Null   # back to the network Windows was on
            Write-Host "  reconnecting Windows to '$previous'"
        }
        Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
    }
}

$listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, $Port)
try { $listener.Start() } catch { Write-Host "Port $Port is busy: is another Wi-Fi helper running?"; Start-Sleep 5; exit 1 }
Write-Host "Aeon Wi-Fi helper: waiting for Aeon on 127.0.0.1:$Port (keep this window open)."
$started = Get-Date
$utf8 = [Text.UTF8Encoding]::new($false)
while ($true) {
    if (-not $listener.Pending()) {
        Start-Sleep -Milliseconds 250
        if (((Get-Date) - $started).TotalSeconds -gt 30 -and -not (Get-Process qemu-system-x86_64 -ErrorAction SilentlyContinue)) { break }
        continue
    }
    $client = $listener.AcceptTcpClient()
    Write-Host "Aeon connected."
    $stream = $client.GetStream()
    $reader = [IO.StreamReader]::new($stream, $utf8)
    $writer = [IO.StreamWriter]::new($stream, $utf8); $writer.NewLine = "`n"; $writer.AutoFlush = $true
    try {
        while ($null -ne ($line = $reader.ReadLine())) {
            $f = $line.TrimEnd("`r").Split("`t")
            switch ($f[0]) {
                'HELLO' { $writer.WriteLine('AEONWIFI 1') }
                'SCAN' { Write-Host 'Aeon asked for a scan.'; foreach ($l in Invoke-Scan) { $writer.WriteLine($l) } }
                'CONNECT' {
                    $ssid = if ($f.Count -gt 1) { $f[1] } else { '' }
                    $pass = if ($f.Count -gt 2) { $f[2] } else { '' }
                    Write-Host "Aeon asked to join '$ssid'."
                    $writer.WriteLine((Invoke-Connect $ssid $pass))
                    $pass = $null
                }
                default { }
            }
        }
    } catch { }
    $client.Close()
    Write-Host 'Aeon disconnected.'
    if (-not (Get-Process qemu-system-x86_64 -ErrorAction SilentlyContinue)) { break }
}
$listener.Stop()
