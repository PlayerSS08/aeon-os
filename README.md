# Aeon Micro-OS

An ultra-minimal 64-bit UEFI microkernel bootloader written in pure Rust (`#![no_std]`, `#![no_main]`) with zero heap allocation, standalone PE32+ binary generation, direct UEFI text output, and COM1 UART 16550 serial telemetry.

---

## Architecture Highlights
- **Single-File Zero-Dependency Architecture:** Core boot entry point and hardware interfaces live entirely within `src/main.rs`.
- **Target:** `x86_64-unknown-uefi` (Native 64-bit Long Mode).
- **Binary Footprint:** Exactly 2.0 KB (`BOOTX64.EFI`) via LLD link-time garbage collection, stripped symbols, and merged sections.
- **Minimalist Console Display:** Pitch-black UEFI display rendering solely the centered, symmetrical `AEON` Unicode block art.
- **Hardware Telemetry Layer:** Direct 16550 UART serial logger (`COM1` @ `0x03F8`, 115200 8N1).

---

## Live Boot Screen (UEFI Display)

```text
                  █████╗  ███████╗  ██████╗  ███╗   ██╗
                 ██╔══██╗ ██╔════╝ ██╔═══██╗ ████╗  ██║
                 ███████║ █████╗   ██║   ██║ ██╔██╗ ██║
                 ██╔══██║ ██╔══╝   ██║   ██║ ██║╚██╗██║
                 ██║  ██║ ███████╗ ╚██████╔╝ ██║ ╚████║
                 ╚═╝  ╚═╝ ╚══════╝  ╚═════╝  ╚═╝  ╚═══╝
```

---

## Live Boot Telemetry (COM1 Serial Log)

```text
[AEON] Bootloader initialized (UEFI x86_64).
[AEON] Boot complete. Entering low-power halt.
```

---

## Project Structure
```text
aeon/
├── .cargo/
│   └── config.toml       # LLD linker flags, subsystem EFI, section merging
├── build/
│   ├── disk/EFI/BOOT/    # Bootloader staging directory (BOOTX64.EFI)
│   ├── aeon.vdi          # VirtualBox VDI hard disk image
│   ├── update_vdi.py     # FAT16 synthesizer & VDI converter (fixed UUID)
│   └── serial.log        # Direct COM1 UART forensic telemetry log
├── scripts/
│   └── run.ps1           # Automated VirtualBox UEFI runner
├── src/
│   └── main.rs           # Pure Rust UEFI boot application
├── Cargo.toml            # opt-level = "z", lto = "fat", panic = "abort"
└── README.md
```

---

## Build & Run

### 1. Prerequisites
- Rust toolchain with UEFI target:
  ```bash
  rustup target add x86_64-unknown-uefi
  ```
- Python 3.x
- Oracle VirtualBox with registered `Aeon` VM (EFI64, 1024 MB RAM, COM1 file redirect to `build/serial.log`).

### 2. Single-Command Build & Boot
Run directly via Cargo:
```powershell
cargo run --release
```
Cargo compiles `aeon.efi`, stages it into `build/disk/EFI/BOOT/BOOTX64.EFI`, synthesizes the bootable FAT16 disk, converts it to `build/aeon.vdi`, restarts the `Aeon` VM in Oracle VirtualBox, captures COM1 telemetry into `build/serial.log`, and renders the pitch-black centered `AEON` console.
