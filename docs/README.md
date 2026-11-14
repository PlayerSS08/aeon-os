# Aeon OS Documentation Hub

**Repository**: `Scrooge-777/aeon`  
**Active Branch**: `feat/create--user`  
**Author**: SREEHARAN (`@Scrooge-777`)  
**Obsidian Vault Hub**: `G:\Brain\AGY\5 Cortex\Comparisons\` (indexed via `Comparisons Map.md`)

---

## 1. Documentation Overview

This directory contains the authoritative technical specifications, architectural designs, performance benchmarks, and comparative analyses for **Aeon OS**—an ultra-minimal, high-assurance bare-metal 64-bit microkernel written in 100% memory-safe Rust (`#![no_std]`).

All major engineering specifications are mirrored and indexed directly inside SREEHARAN's Obsidian Second Brain vault (`G:\Brain\AGY\5 Cortex\Comparisons\`).

---

## 2. Comparative Engineering Specifications

| Specification | Primary Scope | Baseline Targets | Vault Note Equivalent |
|---|---|---|---|
| [AEON_MICRO_OS_SPEC_AND_ISO_PIPELINE.md](AEON_MICRO_OS_SPEC_AND_ISO_PIPELINE.md) | Faculty requirements specification: WiFi/LAN network stack, HP OMEN 16 hardware drivers, Aptitude package manager (aliased to `apt`), Snap Store daemon, boot telemetry benchmarks, and the 7-stage ISO binary packaging pipeline. | Aeon Micro-OS Live Appliance vs Bare-Metal Kernel | `G:\Brain\AGY\5 Cortex\Comparisons\Aeon Micro-OS ISO Pipeline.md` |
| [VIRTUALBOX_FROM_SCRATCH_TUTORIAL.md](VIRTUALBOX_FROM_SCRATCH_TUTORIAL.md) | Ground-up manual creation tutorial for building the virtual disk and configuring the Aeon-VM in Oracle VirtualBox 7.x GUI with EFI and VMSVGA. | Aeon VM Setup Guide | `G:\Brain\AGY\5 Cortex\Comparisons\VirtualBox From Scratch Tutorial.md` |
| [LINUX-COMPARISON.md](LINUX-COMPARISON.md) | Quantitative speed benchmarks (15–45ms boot, 400 KiB binary, 55-cycle syscall latency), 11 implemented Linux x86_64 syscalls, PML4 memory model gap, and 5-bridge roadmap for static Linux binaries. | Aeon OS (v0.9.4) vs Linux Kernel 6.x | `G:\Brain\AGY\5 Cortex\Comparisons\Aeon OS vs Linux.md` |
| [COMPARISON.md](COMPARISON.md) | 14-domain engineering ratings (0–10), measured boot security analysis, scheduler taxonomy, and FAT32 USB mass storage safety rules. | Aeon OS vs Windows 11 vs macOS | `G:\Brain\AGY\5 Cortex\Comparisons\Aeon against Windows 11 and macOS.md` |
| [design/aeon-system-design.md](design/aeon-system-design.md) | Multi-OS architectural matrix, CPU scheduler wait-queue roadmap, minimal footprint benchmarks (64 MB RAM limit), and capability security design. | Aeon vs Linux vs Windows vs macOS vs seL4/Redox | `G:\Brain\AGY\5 Cortex\Comparisons\Aeon System Design & Multi-OS Architecture.md` |

---

## 3. Kernel Architecture & Design Specifications

- [design/aeon-system-design.md](design/aeon-system-design.md): Architectural roadmap for scheduler priority classes, wait queues, tickless idle, and user-space drivers.
- [design/boot-kernel-split.md](design/boot-kernel-split.md): Proposal and modular breakdown for decoupling the monolithic `aeon.efi` binary into a standalone UEFI bootloader (`BOOTX64.EFI` - Dawn) and a pure Long Mode microkernel (`nexus.bin` - Nexus) communicating across a strict `BootInfo` boundary.
- [design/globals.md](design/globals.md): Concurrency and interrupt safety invariants governing all `Global<T>` static variables across the kernel (Rule A: single context, Rule B: interrupts disabled, Rule C: busy flag).

---

## 4. Codebase Inventories & Handoff Logs

- [INVENTORY.md](INVENTORY.md): Exhaustive line-of-code metrics, test coverage, safety comment inventory (235 `// SAFETY` annotations), and module responsibility list across the entire Rust codebase.
- [HANDOFF.md](HANDOFF.md): Engineering state of play, core subsystem invariants, passing QEMU test gates, and physical hardware boundaries.
- [VERSIONS.md](VERSIONS.md): Pinned toolchain, target specifications (`x86_64-unknown-uefi`), and UEFI version baselines.
- [SESSION-2026-11-03.md](SESSION-2026-11-03.md): Engineering log capturing the USB Mass Storage BOT, xHCI keyboard, and write-back cache milestones.

---

## 5. Coursework, Defence & Academic Reports (`docs/ca6/`)

The `docs/ca6/` subdirectory maintains the academic demonstration and viva defence documentation:
- [ca6/architecture.md](ca6/architecture.md): High-level system architecture and 5-stage boot breakdown.
- [ca6/final-report.md](ca6/final-report.md): Final technical delivery report, empirical benchmark results, and evaluation.
- [ca6/design.md](ca6/design.md): Design trade-offs, modular decomposition, and memory layout.
- [ca6/performance.md](ca6/performance.md): Benchmark results covering task switching, frame allocation, and scheduler turnaround.
- [ca6/test-plan.md](ca6/test-plan.md) & [ca6/test-results.md](ca6/test-results.md): Automated QEMU integration test suites.
- [ca6/viva-prep.md](ca6/viva-prep.md): Top 15 technical defence questions, bare-metal failure modes, and memory safety proofs.
- [ca6/evidence/](ca6/evidence/): Serial console logs, CSV/JSON benchmark data, and Gantt scheduler charts.

---

## 6. Engineering Plans (`docs/superpowers/plans/`)

Detailed step-by-step implementation plans used during feature development:
- `2026-11-01-kernel-layers.md`: 10-layer kernel construction plan.
- `2026-11-01-storage.md`: RAM filesystem and block layer plan.
- `2026-11-02-screen-and-menu.md`: GOP framebuffer splash screen and Dawn boot menu plan.
- `2026-11-03-usb-keyboard-xhci.md`: xHCI host controller bringup and USB HID keyboard plan.
- `2026-11-03-pendrive-storage.md`: USB Mass Storage BOT and FAT32 persistence plan.
- `2026-11-03-ahci-disk.md`: AHCI SATA block read support plan.

---

## 7. Synchronization with Obsidian Second Brain

To view these specifications inside Obsidian:
1. Open the vault at `G:\Brain`.
2. Navigate to `AGY/5 Cortex/Comparisons/` for comparative specifications.
3. Navigate to `AGY/7 Prefrontal/Projects/Aeon OS.md` for project roadmap and system architecture.
