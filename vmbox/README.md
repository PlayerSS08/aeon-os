# Aeon OS — Oracle VM VirtualBox Setup (with Wi-Fi)

Runs the newest Aeon build in **Oracle VM VirtualBox** (VirtualBox only, no QEMU), with the boot-menu and shell
Wi-Fi picker working through the Windows host. Tested on Windows 11 with VirtualBox 7.2.22.

## Files

| File | What it is |
|---|---|
| `setup-vbox.ps1` | One-time setup: builds the disks and creates the VM `Aeon-Latest` with every setting it needs. |
| `Start-Aeon.bat` | Starts the VM, then the Wi-Fi helper. Double-click it. |
| `wifi-host-vbox.ps1` | Wi-Fi helper (host side of `src/wifihost.rs`) for VirtualBox. |
| `build_disks.py` | Builds `Aeon-Boot.vdi` / `Aeon-DemoDisk.vdi`. `--efi`, `--mode` (default `1280x800`), `--new-uuids`. |
| `latest/BOOTX64.EFI` | Newest Aeon 0.11.0 build (with the Wi-Fi host picker, 19 Nov). Used instead of `usb/` when present. The signed release in `usb/` is unchanged. |
| `Aeon-VM.vbox` | Older hand-made VM definition (kept for reference; `setup-vbox.ps1` replaces it). |

Made at run time (git-ignored): `*.vdi`, `VMs/`, `aeon-serial.log` (COM1 kernel log), `wifi-helper.log`.

## Quick start

```powershell
powershell -ExecutionPolicy Bypass -File vmbox\setup-vbox.ps1     # once (needs VirtualBox + Python 3)
vmbox\Start-Aeon.bat                                               # every time
```

At the boot menu press **Enter** (or wait 15 s). Click inside the window to type; the **Host key** (right Ctrl)
gives the keyboard and mouse back to Windows. Turn Aeon off with `shutdown`.

## Wi-Fi

A VM has no Wi-Fi radio, so Aeon asks Windows to scan and join networks over its second serial port (COM2):

* the VM serves COM2 on `127.0.0.1:4555` (`--uartmode2 tcpserver 4555`), so it starts with or without the helper;
* `wifi-host-vbox.ps1` connects to it, answers `HELLO` / `SCAN` / `CONNECT`, and closes itself when the VM is off;
* Aeon's own traffic goes through its e1000 network card and its own TCP/IP stack (DHCP, DNS, TCP, HTTP).

**Boot menu:** Down to *Wi-Fi networks*, Enter, pick a network, type the password, Enter, then *Boot Aeon*.
**Shell:** `wifi` (numbered list), `wifi scan`, `wifi status`, `wifi connect <name> <password>`, then e.g.
`ping 8.8.8.8` or `curl http://example.com` (`ping` takes IP addresses only; `curl` is http only).

Choosing a different network really switches Windows to it; a wrong password puts Windows back where it was.

## Fixes needed for VirtualBox

| Problem | Fix (all done by `setup-vbox.ps1` / the helper) |
|---|---|
| VirtualBox 7.2's UEFI firmware crashes (#GP in CpuDxe) before Aeon loads | extradata `VBoxInternal/CPUM/MtrrVarCountIsVirtual = 0` |
| With no `mode=` Aeon picks 5120x3200 (unreadable text) | `aeon.cfg` gets `mode=1280x800` |
| VM refused to power on when the helper was late (COM2 in client mode) | COM2 in **server** mode; the helper connects and retries |
| The firmware writes ANSI escape codes to COM2, garbling Aeon's first `HELLO` | the helper strips escape sequences |
| Aeon waits at most 15 s for a scan; a fresh Windows scan can take longer | the helper answers from Windows' current list at once and refreshes it in the background |
| VirtualBox NAT gives the guest only the host's *first* DNS server; if that one is broken, names fail | DNS proxy + host resolver on; if the network's DNS is broken, set the host's Wi-Fi DNS (e.g. 8.8.8.8) |

---
## Key Commands in Aeon Shell

- **Linux Syscall & Usermode Ring 3 Demo**:
  - `run linux` — Launches the Linux-compatible Ring 3 user process:
    1. Expands heap dynamically via `brk(new_brk)`.
    2. Creates and opens file `/ram/linux.txt` via `open(O_WRONLY | O_CREAT)`.
    3. Writes payload to disk via `write(fd, buf, len)`.
    4. Closes descriptor via `close(fd)`.
    5. Reopens file via `open(O_RDONLY)`.
    6. Reads disk data back into heap memory via `read(fd, heap_buf, len)`.
    7. Closes descriptor via `close(fd)`.
    8. Prints confirmation banner to stdout via `write(1, ...)`: `[user] linux: disk read/write & brk verified`.
    9. Exits cleanly with return code 0 via `exit(0)`.
  - `run hello` — Built-in greeting user demo.
  - `run fault` — Verifies page fault / memory protection isolation.
  - `run gp` — Verifies General Protection Fault ring-3 instruction isolation.
- **Disk & Storage**:
  - `disks` — List detected storage drives (SATA AHCI).
  - `dls` — List files on the boot disk (Aeon 0.11.0 only sees the disk on SATA port 0, so the demo disk is not read).
  - `dcat <file>` — Read a file from that disk.
- **RAM Filesystem (VFS)**:
  - `ls` — List files currently in RAM (including files written by usermode).
  - `cat linux.txt` — Read file written by Linux syscall demo.
  - `write test.txt hello world` — Write file from shell.
- **System Diagnostics**:
  - `mem` / `ps` — Memory usage and running tasks.
  - `uptime` — Time since boot.
  - `version` / `uname` — Kernel release and build information.
  - `shutdown` — Clean ACPI shutdown (Aeon ignores VirtualBox's ACPI power button, so use this).

---

## Rebuilding Disks After Code Changes

1. Build the updated UEFI release binary:
   ```powershell
   cargo build --release --target x86_64-unknown-uefi
   ```
2. Put it where the setup looks first, then rebuild the disks and the VM:
   ```powershell
   copy target\x86_64-unknown-uefi\release\aeon.efi vmbox\latest\BOOTX64.EFI
   powershell -ExecutionPolicy Bypass -File vmbox\setup-vbox.ps1 -Force
   ```
3. Start the VM with `vmbox\Start-Aeon.bat`.
