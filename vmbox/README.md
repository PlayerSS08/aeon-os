# Aeon OS — Oracle VM VirtualBox Setup

This directory contains the VirtualBox virtual machine configuration, virtual disk images, and launcher scripts for **Aeon OS** within the project repository.

---

## Files in this Directory

| File | Description |
|---|---|
| `Aeon-VM.vbox` | VirtualBox VM definition file (`Aeon-VM`). |
| `Aeon-Boot.vdi` | Bootable virtual hard disk (GPT + FAT32 EFI partition with `\EFI\BOOT\BOOTX64.EFI`, certificate, and hash). |
| `Aeon-DemoDisk.vdi` | Attached demo FAT32 disk with test files (`HELLO.TXT`, `BIG.BIN`, `DOCS/NOTE.TXT`). |
| `Start-Aeon.bat` | 1-click batch launcher to start the VM directly. |
| `Start-Aeon.ps1` | PowerShell launcher script. |
| `build_disks.py` | Python tool to rebuild the virtual disks from `usb/`. Auto-detects local repository paths. |
| `aeon-serial.log` | Real-time serial console log (COM1 / 0x3F8) capturing all boot & kernel events. |

---

## How to Start and Run the VM

### Method 1: One-Click Launcher
Double-click `Start-Aeon.bat` (or right-click `Start-Aeon.ps1` -> Run with PowerShell).

### Method 2: Command Line
```powershell
& "C:\Program Files\Oracle\VirtualBox\VBoxManage.exe" startvm "Aeon-VM"
```

### Method 3: Oracle VM VirtualBox Manager (GUI)
1. Open **Oracle VM VirtualBox**.
2. Select **`Aeon-VM`** in the left panel.
3. Click the green **Start** button (or press `Enter`).

---

## Operating Aeon OS Inside VirtualBox

1. **Boot Menu (Dawn)**:
   - When the VM starts, the **Dawn** boot menu will appear with a 15-second countdown.
   - Press **`Enter`** to boot immediately (or wait for the countdown to expire).
2. **Keyboard Focus**:
   - Click inside the VirtualBox VM window to give it keyboard focus.
   - Press the **Host Key** (default: `Right Ctrl`) to release keyboard/mouse back to Windows.
3. **Interactive Shell**:
   - Once booted, the `aeon> ` prompt is ready for commands.

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
  - `dls` — List files on attached demo disk.
  - `dcat HELLO.TXT` — Read contents of demo file from disk.
- **RAM Filesystem (VFS)**:
  - `ls` — List files currently in RAM (including files written by usermode).
  - `cat linux.txt` — Read file written by Linux syscall demo.
  - `write test.txt hello world` — Write file from shell.
- **System Diagnostics**:
  - `status` — Memory usage, CPU cores, running tasks.
  - `uptime` — Time since boot.
  - `version` / `uname` — Kernel release and build information.
  - `poweroff` — Clean ACPI shutdown.

---

## Rebuilding Disks After Code Changes

1. Build the updated UEFI release binary:
   ```powershell
   cargo build --release --target x86_64-unknown-uefi
   ```
2. Copy the binary to `usb/EFI/BOOT/BOOTX64.EFI` (and update `usb/SHA256.txt`).
3. Rebuild the virtual disks:
   ```powershell
   python build_disks.py
   ```
4. Start the VM!
