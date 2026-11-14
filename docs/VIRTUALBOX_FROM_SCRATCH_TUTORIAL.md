# Building and Creating Aeon-VM in Oracle VirtualBox from Scratch

**Document Reference:** `docs/VIRTUALBOX_FROM_SCRATCH_TUTORIAL.md`  
**Author:** SREEHARAN  
**Date:** November 13, 2026  
**Target Environment:** Oracle VM VirtualBox 7.x on Windows 11  
**Source Repository:** `S:\Works\Projects\Aeon\OS`  

---

## 1. Overview & Architecture

When developing an operating system kernel targeting modern 64-bit UEFI hardware, you cannot simply boot a raw `.c` or `.rs` file. The execution pipeline follows three physical stages:

```
+-------------------------------------------------------------------------------+
|                       Aeon VM Construction Pipeline                           |
|                                                                               |
|   1. Compiled Binary:  S:\Works\Projects\Aeon\OS\usb\EFI\BOOT\BOOTX64.EFI     |
|                                 |                                             |
|   2. Disk Synthesizer: python build_disks.py                                  |
|                                 |                                             |
|   3. Virtual Disk:     S:\Works\Projects\Aeon\vmbox\Aeon-Boot.vdi (GPT/FAT32) |
|                                 |                                             |
|   4. VirtualBox VM:    New VM Wizard -> EFI Enabled -> VMSVGA Graphics        |
|                                 |                                             |
|   5. Interactive Boot: Dawn UEFI Menu (15s) -> aeon> shell prompt             |
+-------------------------------------------------------------------------------+
```

---

## 2. Step 1: Create the VM Folder and Synthesize `Aeon-Boot.vdi`

Before opening the VirtualBox GUI, we must create the virtual hard drive containing the UEFI partition.

### The Mechanics of `Aeon-Boot.vdi`
UEFI firmware looks for an **EFI System Partition (ESP)** formatted with FAT32 containing `/EFI/BOOT/BOOTX64.EFI`. The script `build_disks.py` automates this:
1. Allocates a 64 MiB raw GPT disk image (`Aeon-Boot.raw`).
2. Creates the GPT partition table with the standard ESP GUID (`C12A7328-F81F-11D2-BA4B-00A0C93EC93B`).
3. Formats the partition as FAT32.
4. Copies `BOOTX64.EFI`, `SHA256.txt`, `aeon-secureboot.cer`, and `aeon.cfg`.
5. Calls `VBoxManage convertfromraw` to convert the raw image into a dynamically expanding `.vdi` virtual disk.

### Execution Command
Run the following PowerShell command from `S:\Works\Projects\Aeon\OS`:
```powershell
mkdir S:\Works\Projects\Aeon\vmbox -Force
cd S:\Works\Projects\Aeon\OS\vmbox
python build_disks.py
```
This generates:
- `S:\Works\Projects\Aeon\vmbox\Aeon-Boot.vdi`
- `S:\Works\Projects\Aeon\vmbox\Aeon-DemoDisk.vdi` (optional FAT32 demo storage disk)

---

## 3. Step 2: Create the Virtual Machine in VirtualBox Manager

1. Open **Oracle VM VirtualBox** from your Windows Start Menu.
2. In the top toolbar, click the blue **New** button (or press `Ctrl + N`).
3. Fill out the **Name and Operating System** page with the following exact values:

| Field | Exact Value to Enter | Why This Value is Required |
|---|---|---|
| **Name** | `Aeon-VM` | The machine identifier in VirtualBox. |
| **Folder** | `S:\Works\Projects\Aeon\vmbox` | Keeps all VM configuration files and disks self-contained within your project. |
| **ISO Image** | *(Leave completely blank / None)* | We are booting directly from a pre-formatted virtual hard disk (`Aeon-Boot.vdi`), not an optical installer ISO. |
| **Type** | `Other` | Aeon is an independent bare-metal microkernel, not a prepackaged commercial Linux distro. |
| **Version** | `Other/Unknown (64-bit)` | Informs VirtualBox to enable 64-bit Long Mode CPU execution. |

4. Click **Next**.

---

## 4. Step 3: Configure Hardware (RAM, CPU & Mandatory EFI)

Fill out the **Hardware** page:

| Setting | Value | Critical Instruction |
|---|---|---|
| **Base Memory (RAM)** | `1024 MB` (1 GB) | Aeon requires only ~2.5 MiB to boot, but 1024 MB allows ample space for memory frame tests and heap allocations. |
| **Processors (CPU)** | `2 CPUs` | Allows timer and preemptive multitasking tests across virtual cores. |
| **Enable EFI (special OSes only)** | **CHECK THIS BOX** *(Mandatory)* | **CRITICAL:** Aeon compiles directly to an `x86_64-unknown-uefi` PE32+ binary. If EFI is unchecked, VirtualBox defaults to legacy 16-bit BIOS mode, causing an immediate `FATAL: No bootable medium found` error. |

Click **Next**.

---

## 5. Step 4: Attach the Existing Virtual Hard Disk

On the **Virtual Hard Disk** page:

1. Select the option: **"Use an Existing Virtual Hard Disk File"**.
2. Click the small **Folder icon** with the green arrow next to the dropdown menu.
3. In the Hard Disk Selector window, click the **Add** button at the top.
4. Navigate to:
   ```text
   S:\Works\Projects\Aeon\vmbox\
   ```
5. Select **`Aeon-Boot.vdi`** and click **Open**.
6. Ensure `Aeon-Boot.vdi` is highlighted in the list, then click **Choose**.
7. Click **Next**, review the summary, and click **Finish**.

`Aeon-VM` will now appear in your VirtualBox Manager left sidebar.

---

## 6. Step 5: Adjust Settings (Display & Serial Telemetry)

Before starting the VM, two adjustments are required to guarantee that the display renders smoothly on Windows 11.

1. In VirtualBox Manager, click **`Aeon-VM`** on the left, then click the yellow gear icon **Settings** (or press `Ctrl + S`).

### Display Settings
- Click the **Display** tab in the left sidebar.
- **Video Memory**: Drag the slider to **`128 MB`**.
- **Graphics Controller**: Select **`VBoxVGA`**.
  > **Why VBoxVGA?** VirtualBox 7.x's `VMSVGA` requires the guest operating system to send VMware SVGA II FIFO command packets to update the screen. Because Aeon is an ultra-minimal `#![no_std]` Rust microkernel that writes directly to the UEFI Graphics Output Protocol (GOP) linear framebuffer in VRAM, `VMSVGA` receives no blit commands and leaves the Windows display surface black. `VBoxVGA` provides direct unmediated linear GOP framebuffer rendering.
- **EFI Resolution Setting**: Ensure the UEFI GOP resolution is explicitly set to 1024x768:
  ```powershell
  & "C:\Program Files\Oracle\VirtualBox\VBoxManage.exe" setextradata "AEON-VM" "VBoxInternal2/EfiGraphicsResolution" "1024x768"
  ```
- **Acceleration**: Leave *Enable 3D Acceleration* **unchecked**.

### Attach Demo Storage Disk (Optional)
- Click the **Storage** tab.
- Under the **SATA** controller, click the **Add Hard Disk** icon (the small blue hard drive with a green plus sign).
- Select `Aeon-DemoDisk.vdi` from `S:\Works\Projects\Aeon\vmbox\`. This allows you to test the `disks`, `dls`, and `dcat` commands inside the Aeon shell.

### Serial Port Telemetry (Optional but Recommended)
- Click the **Serial Ports** tab.
- Select **Port 1**: Check **Enable Serial Port**.
- **Port Number**: `COM1`.
- **Port Mode**: `Raw File`.
- **Path/Address**: `S:\Works\Projects\Aeon\vmbox\aeon-serial.log`.
  *(This writes a real-time record of all boot stages and kernel prints).*

Click **OK** to save settings.

---

## 7. Step 6: Power On and Interacting with the Shell

1. Select **`Aeon-VM`** in VirtualBox Manager.
2. Click the large green **Start** arrow at the top.
3. The VirtualBox display window will open in the center of your screen.
4. **Boot Flow**:
   - The **Dawn** boot menu will appear with a 15-second countdown timer.
   - Press **`Enter`** on your keyboard to boot immediately.
5. **Keyboard & Mouse Focus**:
   - Click anywhere inside the VM window to lock keyboard focus.
   - Press the **Host Key** (default: `Right Ctrl`) whenever you need to release the mouse cursor back to Windows.
6. **Operating the Shell**:
   - The green `aeon> ` prompt will appear.
   - Try typing:
     - `help` — Lists all built-in commands.
     - `version` / `uname` — Kernel version and compiler build.
     - `disks` — Lists detected AHCI storage controllers.
     - `dls` — Lists files on the demo disk.
     - `dcat HELLO.TXT` — Reads file contents from the virtual disk.
     - `run linux` — Runs the Linux x86_64 usermode demo (`brk`, `open`, `write`, `read`, `exit`).
     - `poweroff` — Clean ACPI system shutdown.
