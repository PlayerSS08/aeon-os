# Aeon Micro-OS

An ultra-minimal, trust-anchored 64-bit UEFI microkernel written in pure Rust (`#![no_std]`, `#![no_main]`) with zero heap allocation, cryptographic table verification, CPU hardware defense auditing, and direct 16550 UART serial telemetry.

---

## Architecture Highlights
- **Single-File Zero-Dependency Architecture:** Core runtime and hardware interfaces live entirely within `src/main.rs`.
- **Target:** `x86_64-unknown-uefi` (Native 64-bit Long Mode).
- **Binary Footprint:** ~4.5 KB (`BOOTX64.EFI`) via LLD link-time garbage collection, stripped symbols, and merged sections.
- **Root-of-Trust & Table Integrity:** Bitwise IEEE 802.3 CRC32 verification over UEFI system headers (`IBI SYST`) validating firmware uncorrupted state.
- **Hardware Security Audit:** Live CPUID silicon probing for NX (No-Execute), SMEP, SMAP, AES-NI, RDRAND, and APIC.
- **Physical Memory Geometry:** Real-time UEFI `GetMemoryMap` parsing calculating total physical RAM, usable 4KB conventional page frames, and reserved firmware boundaries.
- **Display Layer:** Centered UCS-2 UTF-16 Unicode block drawing boot splash on UEFI `SimpleTextOutput`.
- **Telemetry Layer:** Hardware-direct 16550 UART serial logger (`COM1` @ `0x03F8`, 115200 8N1) streaming trust scores and cycle benchmarks. Zero heap allocations, direct port I/O streaming.

---

## Project Structure
```
aeon/
├── .cargo/
│   └── config.toml       # LLD linker flags, subsystem EFI, section merging
├── build/
│   ├── disk/EFI/BOOT/    # Bootloader staging directory (BOOTX64.EFI)
│   └── update_vdi.py     # FAT16 disk synthesizer and VDI converter
├── src/
│   └── main.rs           # Pure Rust microkernel implementation
├── Cargo.toml            # opt-level = "z", lto = "fat", panic = "abort"
└── README.md
```

---

## Build & Test

### 1. Prerequisites
- Rust toolchain with UEFI target:
  ```bash
  rustup target add x86_64-unknown-uefi
  ```
- Python 3
- Oracle VirtualBox (for running on modern 64-bit UEFI VM)

### 2. Compilation
```powershell
cargo build --target x86_64-unknown-uefi --release
Copy-Item "target\x86_64-unknown-uefi\release\aeon.efi" "build\disk\EFI\BOOT\BOOTX64.EFI" -Force
python "build\update_vdi.py"
```

### 3. Emulation (VirtualBox)
Attach `build/aeon.vdi` to a VirtualBox VM with EFI enabled (`Firmware: EFI64`) and COM1 serial enabled (`RawFile` mode to capture telemetry):
```powershell
& "C:\Program Files\Oracle\VirtualBox\VirtualBoxVM.exe" --startvm "Aeon"
```
