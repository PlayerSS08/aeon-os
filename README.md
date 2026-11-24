<div align="center">

# Aeon OS

**A small UEFI x86_64 operating system written from scratch in Rust**

`no_std` · no external crates · Secure Boot signed · measured boot · preemptive multitasking

![version](https://img.shields.io/badge/version-0.11.0-blue)
![language](https://img.shields.io/badge/language-Rust%202024-orange)
![platform](https://img.shields.io/badge/platform-x86__64%20UEFI-lightgrey)
![image size](https://img.shields.io/badge/image-%3C640%20KiB-green)

| Splash | Dawn boot menu | Shell |
|---|---|---|
| ![splash](docs/img/splash.png) | ![Dawn menu](docs/img/menu.png) | ![shell](docs/img/shell.png) |

</div>

---

## About

Aeon is a small operating system that boots from a USB stick on any UEFI PC. One signed image (about 572 KB) contains:

- a boot stage with a logo, a progress bar and a boot menu called **Dawn**;
- a security setup that measures the boot into the TPM, seeds its own randomness and turns on CPU memory protections;
- a preemptive multitasking kernel with a shell.

It runs in 64 MB of RAM and is written entirely in Rust with no third-party dependencies.

## Features

| Area | What Aeon has |
|---|---|
| **Boot** | UEFI loader, splash with a progress bar tied to the real boot stages, Dawn boot menu, `aeon.cfg` options, recovery mode, boot counting |
| **Security** | Secure Boot signed image, TPM 2.0 measured boot (PCR 11/12), event-log replay, W^X paging, NX, SMEP/SMAP/UMIP, guard pages, stack canaries, SBAT |
| **Kernel** | Frame allocator, heap, local APIC timer, preemptive round-robin scheduler, ring-3 processes with their own page tables, `fork`/`execve`/`wait4`/`pipe` |
| **Shell** | 100+ commands, pipes, `>` / `>>` redirection, background jobs (`&`), Ctrl+C, Tab completion, history |
| **Storage** | RAM filesystem with folders, read-only FAT16/FAT32, writable FAT32 on a USB stick labelled `AEON`, virtio-blk, AHCI |
| **Networking** | ARP, IPv4, UDP, ICMP, DHCP, DNS, TCP, HTTP (`curl`), Intel e1000, virtio-net, RTL8168, USB tethering (RNDIS / CDC-ECM), Wi-Fi through the host in a VM |
| **Packages** | `apt`-style package manager with an Ed25519-signed repository, SHA-256 checks and rollback protection |
| **Input** | PS/2 keyboard, USB mouse, I2C HID touchpad |
| **OS course tools** | CPU scheduling simulator (FCFS/SJF/SRTF/RR/priority), Banker's algorithm, dining philosophers, page replacement, disk scheduling |

## Quick start

### Run it in QEMU on Windows

```powershell
powershell -ExecutionPolicy Bypass -File tools\run-qemu.ps1 -Build -Disk
```

You need QEMU in `C:\Program Files\qemu`. Useful flags:

- `-Release` runs the signed image in `usb/`.
- `-Cfg "mode=1920x1080"` writes an `aeon.cfg`.
- `-Mem 64` limits the RAM.

### Boot it on a real PC

1. Format a USB stick as **FAT32**.
2. Copy everything inside `usb/` to the root of the stick.
3. Boot the PC from the stick in **UEFI** mode. The boot-menu key is usually F12, F10, F9 or Esc.
4. Either turn **Secure Boot off**, or enroll `usb/aeon-secureboot.cer` in the firmware's `db`.

To check the copy, compare the files against `usb/SHA256.txt`.

## Build from source

```bash
rustup target add x86_64-unknown-uefi
cargo build --release                                  # target/x86_64-unknown-uefi/release/aeon.efi
cargo clippy --release -- -D warnings
cargo test --lib --target x86_64-pc-windows-gnu        # host unit tests (or x86_64-unknown-linux-gnu)
bash tools/release.sh                                  # build, sign and refresh usb/ (needs WSL + sbsigntool)
bash tests/run.sh                                      # lint, unit tests and the QEMU boot tests
```

To run the boot tests on Ubuntu, install `qemu-system-x86 ovmf swtpm sbsigntool openssl python3-virt-firmware`.

## Shell examples

```text
aeon> help                     grouped command list
aeon> security                 active protections, Secure Boot and TPM state
aeon> dhcp                     get a network address
aeon> curl http://example.com  download over HTTP
aeon> apt update && apt list   signed package repository
aeon> ls /bin | wc             real processes connected by a pipe
aeon> sched cmp                compare CPU scheduling algorithms
```

[`cmd.md`](cmd.md) is the full command reference.

## Project layout

```
.
├── src/          kernel and boot stage (Rust, no_std)
├── user/         user-space programs shipped in /bin (initfs)
├── tests/        QEMU boot tests, fixtures, Miri script
├── tools/        release signing, font generator, QEMU runner, package-repo tool, Wi-Fi host helper
├── usb/          signed, ready-to-copy boot image and Secure Boot certificate
├── repo/         sample signed package repository
├── docs/         design notes, release notes, comparisons, course evidence
├── images/       screenshots
├── vmbox/        VirtualBox VM definition and helper scripts
└── cmd.md        shell command reference
```

## Documentation

| Document | Contents |
|---|---|
| [`docs/TECHNICAL.md`](docs/TECHNICAL.md) | Full technical overview: boot stages, security model, test coverage, source map |
| [`cmd.md`](cmd.md) | Every shell command, with examples |
| [`docs/design/aeon-system-design.md`](docs/design/aeon-system-design.md) | System design notes |
| [`docs/RELEASE-0.11.0.md`](docs/RELEASE-0.11.0.md), [`docs/RELEASE-0.10.0.md`](docs/RELEASE-0.10.0.md) | Release notes |
| [`docs/OS-LANDSCAPE.md`](docs/OS-LANDSCAPE.md), [`docs/COMPARISON.md`](docs/COMPARISON.md) | How Aeon compares with Linux, Windows and macOS |
| [`docs/VIRTUALBOX_FROM_SCRATCH_TUTORIAL.md`](docs/VIRTUALBOX_FROM_SCRATCH_TUTORIAL.md) | Running Aeon in VirtualBox |
| [`docs/HANDOFF.md`](docs/HANDOFF.md) | Current state and next steps |

## Status

Aeon is an **educational project**. Every change is tested in QEMU with OVMF, a software TPM (swtpm) and Secure Boot. It has **not** been tested on real hardware. Version 0.11.0 was unit-tested on the host and has been booted in QEMU on Windows.

**Known limits:** one CPU only, no TLS (only `http://` works), and no driver for a real Wi-Fi chip yet.

**Roadmap:** SMP, IOAPIC interrupts, NVMe, a USB keyboard driver and an `aarch64` port.

## Credits

The fonts are Inter and JetBrains Mono, both under the SIL Open Font License. Their license texts are in [`docs/licenses/`](docs/licenses/).
