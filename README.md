# Aeon

A 74 KB UEFI x86_64 boot stage and small kernel in Rust (`no_std`, no dependencies). It boots from a USB
stick on any UEFI PC, shows a splash and a structured boot log, leaves firmware control, sets up its own
CPU/memory protections, and drops into a preemptive multitasking kernel with a shell.

## Boot from a USB stick (any UEFI PC)

Format a USB stick as **FAT32**, then copy the contents of the `usb` folder (the `EFI` folder) to the root of the stick. Verify the copy with `usb/SHA256.txt`.

Then boot the PC from the stick through its boot menu (usually F12, F10, Esc or F9) in **UEFI** mode.
The image is unsigned, so **turn Secure Boot off** in the firmware setup first (or enroll the image's hash
in `db`). Aeon shows "press any key for the boot menu" for 2 seconds, then boots.

Boot menu keys: `Enter` boot, `D` debug logging, `F` reboot into firmware setup, `R` reboot, `S` shut down.
Boot options (image command line / UEFI load options): `quiet`, `debug`, `nomenu`, `timeout=N`.

Shell keys use a PS/2 keyboard (laptop keyboards and most USB keyboards in firmware "legacy" mode).

## What happens at boot

| Phase | What happens |
|---|---|
| firmware entry | stop the 5-minute firmware watchdog, calibrate the TSC against firmware `Stall`, start log sinks, publish `LoaderInfo` (systemd Boot Loader Interface) |
| early init | bounds-checked CPUID, control-register state, GOP framebuffer mode and splash |
| platform discovery | firmware vendor/revision, image base/size, ACPI RSDP checksums, SMBIOS, Secure Boot / Setup Mode, TPM 2.0 presence |
| boot menu | wait for a key through the firmware (works with USB keyboards) |
| ExitBootServices | size the memory-map buffer from the firmware, fetch map, exit, retry on a stale key; no logging in between |
| kernel environment | copy the memory map, enable NX, GDT/TSS (double-fault stack), IDT, identity-mapped page tables with W^X, unmapped null page and stack guard pages, CR0.WP/SMEP/SMAP/UMIP when supported, own stack |
| kernel | frame allocator, 32 MB heap, PIT tick, scheduler with `status` and `shell` tasks; the boot thread idles on `hlt` |

Shell commands: `help info uptime mem ps spawn counters kill memmap log echo sort clear panic reboot
shutdown`. `spawn` starts two busy workers that never yield; `ps`/`counters` show both advancing, i.e. the
timer interrupt preempting them. Shutdown uses ACPI S5, then firmware `ResetSystem`.

Log lines: `[seconds.microseconds] LEVEL target key=value ...`, sent to the screen, COM1 (if present), QEMU
debugcon port `0xE9` and a 64 KiB ring.

## Security

- W^X page tables from the PE section headers, NX everywhere else, unmapped null page, `CR0.WP`, and
  SMEP/SMAP/UMIP when the CPU has them. Registers are read back and logged, not assumed from CPUID.
- Guard pages under the kernel stack and every task stack (overflow faults instead of corrupting memory),
  plus a canary check; a separate double-fault stack.
- Memory freed by the firmware is zeroed before reuse, and freed heap blocks are scrubbed.
- Firmware input is bounded and validated: ACPI tables checked for signature, length and checksum and
  required to lie inside firmware-described memory; command-line options parsed from a fixed grammar.
- Reports Secure Boot / Setup Mode and TPM presence honestly; makes no attestation claims.
- Not yet: signed image (Secure Boot), measured boot into TPM PCRs, kernel signature check, ring 3.

## Compared with other boot systems

| Idea | Linux (shim, GRUB / systemd-boot, UKI) | Windows (bootmgfw, winload) | Aeon |
|---|---|---|---|
| Signed chain under Secure Boot | shim (Microsoft-signed) then signed loader and kernel | `bootmgfw.efi` then `winload.efi`, all Microsoft-signed | unsigned for now: Secure Boot must be off or the image hash enrolled |
| Measured boot | PCRs 4, 7, 9, 11, 12 via TPM2 | PCR 7 and the TCG log, used by BitLocker | detects the TPM; extending PCRs not done yet |
| Boot menu with timeout | GRUB menu, systemd-boot menu | Windows Boot Manager | 2 s menu through firmware input |
| Boot options | kernel command line | BCD store | `quiet debug nomenu timeout=N` |
| Firmware setup from the menu | `systemctl reboot --firmware-setup` (OsIndications) | "UEFI Firmware Settings" | `F` in the menu, same OsIndications mechanism |
| Tell the OS who booted it | Boot Loader Interface variables (`LoaderInfo`) | BCD / `bootmgr` state | publishes `LoaderInfo` |
| Free boot memory after exit | `init_on_free`, bootmem teardown | memory manager takes over | zeroes reclaimed memory |
| Memory protection from the start | kernel W^X, guard pages (`CONFIG_VMAP_STACK`) | HVCI / VBS, kernel DEP | W^X, guard pages |

## Build

```
rustup target add x86_64-unknown-uefi
cargo run --release          # builds and refreshes usb/ (EFI/BOOT/BOOTX64.EFI + SHA256.txt)
cargo clippy --release -- -D warnings
cargo test --lib --target x86_64-pc-windows-gnu     # or x86_64-unknown-linux-gnu: 50 host unit tests
```

`cargo run` does not start a VM; it only prepares `usb/`. Set `AEON_GIT_HASH` at build time to show a
revision in the banner. Features that trigger one fault after boot, for testing the handlers:
`fault-ud fault-pf fault-stack fault-wx fault-nx fault-panic`.

## Source layout

| File | Role |
|---|---|
| `main.rs` | boot flow, boot menu, kernel start, panic handler |
| `uefi.rs` | hand-written UEFI bindings, every used table offset asserted at compile time |
| `arch.rs` | x86-specific code: port I/O, CPUID, control registers, GDT/TSS/IDT, exception and switch stubs, PIC/PIT, UART |
| `paging.rs` | identity-mapped page tables, W^X, guard pages |
| `mm.rs`, `frames.rs`, `heapalloc.rs` | frame bitmap allocator, free-list heap, `GlobalAlloc` |
| `sched.rs` | preemptive round-robin tasks (registers + FXSAVE on each stack) |
| `shell.rs`, `keymap.rs`, `power.rs` | shell and status bar, scancode decoder, reboot and ACPI power-off |
| `fb.rs`, `font.rs`, `log.rs` | framebuffer console, public-domain 8x8 font, logger and sinks |
| `logfmt.rs`, `memmap.rs`, `acpi.rs`, `cpuid.rs`, `pixel.rs`, `bootopts.rs`, `bootinfo.rs`, `boot.rs` | pure logic (unit-tested) and boot state |

## Tested and not tested

Tested on VirtualBox 7.2 (EFI64, EDK II, 1 CPU, 1 GB): normal boot, GPT/FAT32 USB image boot, shell,
spawn/kill, shutdown, reboot, and all fault builds before the latest changes. **Not tested:** real hardware,
QEMU/OVMF, Hyper-V, VMware, Secure Boot, a TPM, the boot menu keys (VirtualBox did not deliver injected
keys to the firmware input), SMEP/SMAP/UMIP enabling (the VM CPU lacks them), USB-only keyboards.

## Limits and roadmap

Single CPU, identity mapping only, kernel threads only (no ring 3, syscalls, filesystem or networking).
Next: a signed image and Secure Boot test, TCG2 measured boot, user mode with syscalls, a ramdisk
filesystem, a USB keyboard driver, APIC/SMP, and an `aarch64` port.
