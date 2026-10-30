# Aeon

An 85 KB UEFI x86_64 boot stage and small kernel in Rust (`no_std`, no dependencies). It boots from a USB
stick on a UEFI PC, shows a logo with a progress bar, measures its command line into the TPM, seeds its
own randomness, leaves firmware control, sets up its own CPU and memory protections, and drops into a
preemptive multitasking kernel with a shell.

## Boot from a USB stick

1. Format a USB stick as **FAT32**.
2. Copy everything inside the `usb` folder to the root of the stick (you get `EFI/`, `SHA256.txt` and
   `aeon-secureboot.cer`).
3. Boot the PC from the stick through its boot menu (F12, F10, Esc or F9) in **UEFI** mode.

The image is **signed** with the Aeon release key. Two ways to run it:

- **Secure Boot off** in the firmware setup: nothing else to do.
- **Secure Boot on**: add `aeon-secureboot.cer` to the firmware's `db` (Secure Boot key management,
  "Append"/"Enroll" from file; the menu name differs per vendor), or enroll the image hash from
  `SHA256.txt`. Aeon is not signed by Microsoft, so firmware with only the factory keys refuses it.

Check the copy with `SHA256.txt` (`sha256sum -c SHA256.txt` or `Get-FileHash`).

While booting you see the logo fade in and a thin bar fill as the real boot stages complete. Press any key
in the first 2 seconds for the boot menu. The menu leaves the splash and shows the full boot log.

Menu keys: `Enter` boot, `D` debug logging, `F` reboot into firmware setup, `R` reboot, `S` shut down.
Boot options (image command line / UEFI load options): `quiet`, `debug`, `verbose` (log instead of
splash), `nomenu`, `timeout=N`. `debug` implies `verbose`.

Shell keys use a PS/2 keyboard (laptop keyboards and most USB keyboards in firmware "legacy" mode).

## What happens at boot

| Stage | What happens |
|---|---|
| firmware-entry | stop the 5-minute firmware watchdog, calibrate the TSC against firmware `Stall`, start log sinks, publish `LoaderInfo` (systemd Boot Loader Interface) |
| early-init | bounds-checked CPUID, control-register state, GOP framebuffer in the firmware's own (native) mode, logo fade-in |
| platform-discovery | firmware vendor/revision, image base/size, ACPI RSDP checksums, SMBIOS, Secure Boot / Setup Mode, TPM 2.0 presence |
| security-init | boot random seed from the firmware RNG and RDRAND, randomised stack canary, command line measured into TPM PCR 12 and read back to verify |
| boot menu | wait for a key through the firmware (works with USB keyboards) |
| exit-boot-services | size the memory-map buffer from the firmware, fetch map, exit, retry on a stale key; no logging in between |
| kernel-env | copy the memory map, enable NX, GDT/TSS (double-fault stack), IDT, identity-mapped page tables with W^X, unmapped null page and stack guard pages, CR0.WP/SMEP/SMAP/UMIP when supported, own stack |
| kernel-memory, kernel-tasks | frame allocator, 32 MB heap, PIT tick, scheduler with `status` and `shell` tasks; the boot thread idles on `hlt` |

Shell commands: `help info uptime mem ps spawn counters kill memmap log echo sort clear panic reboot
shutdown`. `spawn` starts two busy workers that never yield; `counters` shows both advancing, i.e. the
timer interrupt preempting them. `log` prints the boot log, which the splash held back. Shutdown uses ACPI
S5, then firmware `ResetSystem`.

Log lines: `[seconds.microseconds] LEVEL target key=value ...`, sent to the screen (when not in splash),
COM1 (if present), QEMU debugcon port `0xE9` and a 64 KiB ring.

A fatal error (panic or CPU exception) shows a stop screen with the cause and nothing else: no addresses
or register values. Those go to serial and debugcon only.

## Security

- **Secure Boot ready**: the release image is signed; its certificate ships in `usb/` for enrollment.
- **Measured boot**: the boot command line is hashed into TPM PCR 12 (event type `EV_IPL`) through
  `EFI_TCG2_PROTOCOL`, then the PCR is read back and compared with the value computed from the previous
  one. The firmware's own event log stays valid. The firmware measures this image into PCR 4 itself.
- **Boot random seed**: SHA-256 over the firmware RNG, four RDRAND words and timing. It seeds the kernel
  stack-overflow canary. The seed is never logged. If neither hardware source exists, the log warns.
- W^X page tables from the PE section headers, NX everywhere else, unmapped null page, `CR0.WP`, and
  SMEP/SMAP/UMIP when the CPU has them. Registers are read back and logged, not assumed from CPUID.
- Guard pages under the kernel stack and every task stack (overflow faults instead of corrupting memory),
  plus a canary check; a separate double-fault stack.
- Memory freed by the firmware is zeroed before reuse, and freed heap blocks are scrubbed.
- Firmware input is bounded and validated: ACPI tables checked for signature, length and checksum and
  required to lie inside firmware-described memory; memory-map sizes saturate instead of overflowing;
  command-line options parsed from a fixed grammar. The parsers are fuzzed in the unit tests.
- Reports Secure Boot / Setup Mode and TPM presence honestly; makes no attestation claims.
- Not yet: kernel signature check beyond the firmware's, ring 3.

## Compared with other boot systems

| Idea | Linux (shim, GRUB / systemd-boot, UKI) | Windows (bootmgfw, winload) | macOS (Apple silicon) | Aeon |
|---|---|---|---|---|
| Signed chain | shim (Microsoft-signed), then signed loader and kernel | `bootmgfw.efi`, `winload.efi`, boot drivers, ELAM | Boot ROM, LLB, iBoot, kernel; signed system volume | one signed image; certificate to enroll |
| Measured boot | PCRs 4, 7, 9, 11, 12 | PCR 7 and the TCG log, used by BitLocker | Secure Enclave policy, no TPM | command line into PCR 12, verified by read-back |
| Boot menu | GRUB, systemd-boot | Windows Boot Manager | Startup Options | 2 s menu through firmware input |
| Boot options | kernel command line | BCD store | boot-args | `quiet debug verbose nomenu timeout=N` |
| Firmware setup from the menu | `systemctl reboot --firmware-setup` | "UEFI Firmware Settings" | n/a | `F` in the menu (OsIndications) |
| Tell the OS who booted it | Boot Loader Interface variables | BCD / `bootmgr` state | iBoot handoff | publishes `LoaderInfo` |
| Entropy handoff | random seed file and `LoaderRandomSeed` | boot entropy block | boot-time entropy | firmware RNG + RDRAND seed in `BootInfo` |
| Free boot memory after exit | `init_on_free` | memory manager takes over | n/a | zeroes reclaimed memory |
| Memory protection from the start | kernel W^X, guard pages | HVCI / VBS, kernel DEP | KTRR/PPL | W^X, guard pages, SMEP/SMAP |
| Boot screen | Plymouth, firmware logo (BGRT) | logo and spinner | logo and bar | logo, fade, bar tied to real stages |
| Automatic fallback | boot counting in systemd-boot | recovery environment | recoveryOS | none (single image, nothing to fall back to) |

## Build and test

```
rustup target add x86_64-unknown-uefi
cargo build --release              # target/x86_64-unknown-uefi/release/aeon.efi
cargo clippy --release -- -D warnings
cargo test --lib --target x86_64-pc-windows-gnu     # or x86_64-unknown-linux-gnu: host unit tests
bash tools/release.sh              # build, sign, refresh usb/ (signing needs WSL with sbsigntool)
bash tests/run.sh                  # lint, unit tests, then the QEMU boot tests below
```

The boot tests (`tests/boot_test.py`) need QEMU, OVMF, swtpm, sbsigntool and virt-firmware (on Ubuntu:
`qemu-system-x86 ovmf swtpm sbsigntool openssl python3-virt-firmware`). They boot the real image and check:

- all boot stages, the seed, the splash (pixel checks on screenshots), the menu, the shell and preemption;
- measured boot with a software TPM: PCR 12 equals `SHA256(before || SHA256(cmdline))` computed
  independently, and the no-TPM path;
- Secure Boot: a signed image boots, an unsigned image and an image from an unknown key are refused;
- the shipped `usb/` folder: checksums, contents, and that it boots after enrolling its certificate;
- every `fault-*` build ends in the expected exception or panic and shows the stop screen.

Screenshots land in `target/test-shots/`. Set `AEON_GIT_HASH` at build time to show a revision in the
banner. The fault features (`fault-ud fault-pf fault-stack fault-wx fault-nx fault-panic`) each trigger one
fault after boot; build them with `cargo build`, never into `usb/`.

## Source layout

| File | Role |
|---|---|
| `main.rs` | boot flow, boot menu, seed and measured boot, kernel start, panic handler |
| `uefi.rs` | hand-written UEFI bindings, every used table offset asserted at compile time |
| `arch.rs` | x86-specific code: port I/O, CPUID, RDRAND, control registers, GDT/TSS/IDT, exception and switch stubs, PIC/PIT, UART |
| `paging.rs` | identity-mapped page tables, W^X, guard pages |
| `mm.rs`, `frames.rs`, `heapalloc.rs` | frame bitmap allocator, free-list heap, `GlobalAlloc` |
| `sched.rs` | preemptive round-robin tasks (registers + FXSAVE on each stack) |
| `shell.rs`, `keymap.rs`, `power.rs` | shell and status bar, scancode decoder, reboot and ACPI power-off |
| `fb.rs`, `font.rs`, `log.rs` | framebuffer console, splash and stop screen, public-domain 8x8 font, logger and sinks |
| `sha256.rs`, `entropy.rs`, `tcg2.rs`, `splash.rs`, `logfmt.rs`, `memmap.rs`, `acpi.rs`, `cpuid.rs`, `pixel.rs`, `bootopts.rs`, `bootinfo.rs`, `boot.rs` | pure logic (unit-tested) and boot state |
| `tests/`, `tools/` | boot tests; release signing |

## Tested and not tested

Run on every change: QEMU 10.2 with EDK II OVMF (Ubuntu, WSL2, software emulation), a software TPM 2.0
(swtpm), OVMF Secure Boot with throwaway and release keys, and the host unit tests.

**Not tested:** real hardware, USB-only keyboards, VirtualBox/Hyper-V/VMware, a hardware TPM, enrolling the
certificate through a real firmware setup screen, SMEP/SMAP/UMIP effects on a CPU that has them under
emulation limits, and screens other than 1280x800 (the 4K scale path is unit-tested but not seen).

## Limits and roadmap

Single CPU, identity mapping only, kernel threads only (no ring 3, syscalls, filesystem or networking).
Boot counting with fallback and a kernel hash check need the kernel split from the loader into its own
file; today both are one image, so a self-check would prove nothing. Next: split loader and kernel, user
mode with syscalls, a ramdisk filesystem, a USB keyboard driver, APIC/SMP, and an `aarch64` port.
