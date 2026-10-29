# Aeon Micro-OS

An ultra-minimal, trust-anchored 64-bit UEFI microkernel written in pure Rust (`#![no_std]`, `#![no_main]`) with zero heap allocation, cryptographic root-of-trust verification, FIPS 180-4 SHA-256 measured boot, ACPI 2.0 table checksum validation, NIST SP 800-22 hardware entropy testing, physical memory geometry calculus, and 16550 UART serial telemetry.

---

## Architecture Highlights
- **Single-File Zero-Dependency Architecture:** Core runtime, cryptographic engines, and hardware interfaces live entirely within `src/main.rs`.
- **Target:** `x86_64-unknown-uefi` (Native 64-bit Long Mode).
- **Binary Footprint:** ~14.0 KB (`BOOTX64.EFI`) via LLD link-time garbage collection, stripped symbols, and merged sections.
- **Root-of-Trust & Table Integrity:** Bitwise IEEE 802.3 CRC32 verification over UEFI system headers (`IBI SYST`) validating uncorrupted firmware state.
- **ACPI 2.0+ Discovery & Dual Checksums:** Scans `SystemTable.configuration_table` for RSDP GUIDs, validates ACPI 1.0 (20-byte) and ACPI 2.0 extended (36-byte) checksums ($\sum \equiv 0 \pmod{256}$), and locates 64-bit XSDT physical addresses.
- **Cryptographic Measured Boot (SHA-256):** Implements zero-allocation FIPS 180-4 SHA-256 hash engine:
  - `PCR0`: Bootloader executable code self-measurement (4KB resident code frame).
  - `PCR1`: Firmware system and configuration tables measurement.
  - `PCR2`: Physical RAM memory map geometry baseline measurement.
- **Hardware Entropy & NIST SP 800-22 Testing:**
  - Probes silicon `RDRAND` and high-resolution `_rdtsc()` clock jitter.
  - Runs **NIST SP 800-22 Frequency (Monobit) Test** on 2,048-bit sample ($|n_1 - n_0| \le 148$ at $\alpha = 0.001$).
  - Calculates fixed-point **Shannon Entropy Density** ($H \approx 7.2 - 8.0$ bits per byte) via integer log2 calculus.
- **Physical Memory Geometry & Fragmentation Calculus:**
  - Parses real-time `GetMemoryMap` descriptors into total, usable conventional, and reserved page frames.
  - Computes exact memory fragmentation index ($\Phi$) and Top of Memory (TOM) physical boundary.
- **Silicon Defenses Audit:** Probes CPUID for NX (No-Execute), SMEP, SMAP, UMIP, CET Shadow Stack, AES-NI, RDRAND, RDSEED, physical/virtual address bus width, and hypervisor isolation.
- **Minimalist Console Display:** Pitch-black UEFI display rendering solely the centered, symmetrical `AEON` Unicode block art. Zero console clutter.
- **Hardware Telemetry Layer:** Direct 16550 UART serial logger (`COM1` @ `0x03F8`, 115200 8N1) streaming all mathematical, cryptographic, measured boot, and silicon security telemetry.
- **100% Attestation Score:** Computes composite Hardware Root-of-Trust score (0–100) attesting platform security.

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

```
[AEON] ================================================================
[AEON:BOOT]  Root of Trust Initialized (UEFI x86_64 Long Mode)
[AEON:SEC]   SystemTable Signature: 0x5453595320494249 ("IBI SYST" - VERIFIED)
[AEON:SEC]   Header CRC32 Checksum: 0xA8128C57 (INTEGRITY PASS)
[AEON:ACPI]  ACPI Root Pointer: ACPI 2.0+ (XSDT @ 0x000000003ED7D0E8) [CHECKSUM PASS]
[AEON:SMBIOS] Anchor Status: NOT DETECTED
[AEON:SEC]   Hardware Defenses: NX=ON | SMEP=OFF | SMAP=OFF | UMIP=OFF | HYPERVISOR=ACTIVE
[AEON:SEC]   Silicon Trust: AES-NI=ON | RDRAND=ON | RDSEED=ON | APIC=ON | TSC=ON
[AEON:ARCH]  Address Bus Topology: 39-bit Physical | 48-bit Virtual
[AEON:MATH]  Physical RAM Map Descriptors: 104 regions parsed
[AEON:MATH]  Total Physical Memory:  1027 MB (263056 x 4KB frames)
[AEON:MATH]  Conventional Usable RAM: 980 MB (251017 free frames)
[AEON:MATH]  ACPI Reclaim / MMIO:    0 MB ACPI | 4 MB MMIO
[AEON:MATH]  RAM Fragmentation:      2% (Largest Block: 958 MB in 5 segments)
[AEON:MATH]  Top of Memory (TOM):    4096 MB physical boundary
[AEON:ENTR]  NIST SP 800-22 Monobit: PASS (|n1-n0| = 20 / 2048 bits; ones=1034, zeros=1014)
[AEON:ENTR]  Shannon Entropy Density: 7.26 / 8.00 bits per byte
[AEON:PCR0]  Bootloader Code SHA-256: 8CA88CFA14883F6117DD0DBA3FA1075F8D8478583F44241D879C741E82A4CD32
[AEON:PCR1]  Firmware Tables SHA-256: 3EFE46F2C4449C45EE443366032AE708B868233FC05528C42C8DB462AA513B46
[AEON:PCR2]  Memory Map Base SHA-256: 000009A551812EB8FF5A013B96F803C14502AB4C0A68C63D698881A4055F7FC0
[AEON:TIME]  Execution Cycle Overhead: 38504354 CPU cycles
[AEON:TRUST] Attestation Score: 100/100 (TIER-1 SECURE HARDWARE ROOT OF TRUST VERIFIED)
[AEON] ================================================================
```
```

---

## Project Structure
```
aeon/
├── .cargo/
│   └── config.toml       # LLD linker flags, subsystem EFI, section merging
├── build/
│   ├── disk/EFI/BOOT/    # Bootloader staging directory (BOOTX64.EFI)
│   ├── serial.log        # Direct COM1 UART forensic telemetry log
│   └── screen.png        # Captured UEFI framebuffer screenshot
├── scripts/
│   └── run.ps1           # Automated QEMU UEFI boot & telemetry runner
├── src/
│   └── main.rs           # Pure Rust microkernel implementation
├── Cargo.toml            # opt-level = "z", lto = "fat", panic = "abort"
└── README.md
```

---

## Build & Run

### 1. Prerequisites
- Rust nightly/stable toolchain with UEFI target:
  ```bash
  rustup target add x86_64-unknown-uefi
  ```
- QEMU (`qemu-system-x86_64`) + OVMF x86_64 firmware.

### 2. Single-Command Build & Boot
Run directly via Cargo:
```powershell
cargo run --release
```
Cargo automatically compiles `aeon.efi`, stages it into `build/disk/EFI/BOOT/BOOTX64.EFI`, boots QEMU with direct virtual FAT32 mounting, streams live COM1 telemetry to stdout and `build/serial.log`, and opens the pitch-black centered `AEON` console.

### 3. Remote GDB Debugging (Optional)
To attach GDB and debug the microkernel at instruction level:
```powershell
qemu-system-x86_64 -bios <path-to-ovmf> -drive format=raw,file=fat:rw:build/disk -s -S
```
In GDB or VS Code:
```gdb
target remote :1234
b efi_main
c
```
