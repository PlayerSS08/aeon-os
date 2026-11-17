# Aeon among other operating systems (v0.10.0)

Written 2026-11-16. Aeon facts come from this repository's code and tests. Facts about other systems are general
knowledge plus the sources listed at the end; anything not verified is marked *(unverified)*. No source-line counts
are quoted for other systems because the numbers found online disagreed.

## What kind of system Aeon is

A UEFI x86_64 micro-OS in Rust (`no_std`, **no third-party crates**) that boots from a USB stick, sets up its own
memory protections (NX, W^X, SMEP/SMAP/UMIP, guard pages), runs a preemptive scheduler on the local APIC timer, has a
ring-3 user mode with a syscall allow-list, a RAM filesystem plus FAT32 on a USB stick, a USB (xHCI) keyboard, mouse
and mass-storage driver, Ethernet drivers (Intel e1000, VirtIO, Realtek RTL8168), a real TCP/IP client stack with DHCP,
DNS and HTTP, and a signed package manager.

## Comparison

| | Aeon 0.10 | Linux | Windows (NT) | macOS (XNU) | FreeBSD | Redox | SerenityOS | Haiku | MINIX 3 | xv6 |
|---|---|---|---|---|---|---|---|---|---|---|
| Kernel design | small monolithic-style kernel, single address space | monolithic | hybrid | hybrid (Mach + BSD) | monolithic | microkernel | monolithic *(unverified)* | hybrid *(unverified)* | microkernel | monolithic, teaching |
| Main language | Rust | C (+ Rust growing) | C/C++ | C/C++ | C | Rust | C++ | C++ | C | C |
| Purpose | learn + run on one laptop | general | general | general | general | general-purpose Rust OS | from-scratch desktop Unix-like | BeOS-style desktop | research/teaching | teaching |
| Per-process address spaces | yes since 0.11.0 (own page tables per program; **untested on hardware**) | yes | yes | yes | yes | yes | yes | yes | yes | yes |
| SMP scheduling | **no** (CPUs are listed, one runs the scheduler) | yes | yes | yes | yes | yes | yes | yes | yes | yes (simple) |
| Run foreign binaries | static ELF64 programs built for Aeon's Linux-numbered calls (`fork`, `execve`, pipes); no dynamic linking, no Linux userland | ELF | PE | Mach-O | ELF (+Linux compat) | own programs | own programs | own programs | own programs | own user programs |
| Filesystems | RAM, FAT16/32 read, FAT32 write on one USB volume | ext4, XFS, Btrfs, ... | NTFS, ReFS, FAT | APFS | UFS, ZFS | RedoxFS | ext2, FAT | BFS | MINIX FS | simple log FS |
| Networking | IPv4, TCP client, UDP, DHCP, DNS, HTTP; **no TLS, no listening sockets, no IPv6** | full | full | full | full | partial | partial | partial | partial | none |
| Wi-Fi | stack tested in simulation; **no chip driver** | many drivers + firmware | many | many | limited | limited *(see below)* | none *(unverified)* | limited *(unverified)* | none | none |
| Package manager | signed, own format (`apt`) | apt, dnf, pacman, ... | winget, MSI | brew (3rd party), App Store | pkg, ports | pkg (`redoxer`/`pkg`) | ports | pkgman | pkgin | none |
| Security model | W^X, SMEP/SMAP, canaries, TPM-measured boot, Secure Boot signed image | DAC, LSM, namespaces | ACLs, VBS, etc. | sandbox, SIP | jails, capsicum | capability-like schemes | pledge/unveil style | basic | isolation by design | none |
| Hardware reach | a few chips, tested in QEMU | enormous | enormous | Apple hardware | wide | growing, "many consumer machines still lack Wi-Fi/GPU/sound drivers" | QEMU-first | x86 desktops | modest | QEMU/RISC-V |

## Where Aeon stands, level by level

**Beginner (done and tested):** UEFI boot, framebuffer console, keyboards (PS/2 + USB), memory map and frame allocator,
heap, GDT/IDT/page tables, timer interrupts, a shell.

**Intermediate (done):** preemptive scheduler with APIC timer, wait queues, spinlocks, ring-3 programs with syscalls,
FAT32 read/write, USB host controller + storage + mouse, PCI scan, NIC drivers, TCP/IP with DNS/HTTP, signed packages,
measured boot (TPM), 570+ host tests and QEMU boot tests.

**Intermediate (not done, in rough priority order):**
1. ~~Per-process page tables, `execve`, `fork`/`wait`, pipes~~ done in 0.11.0 (see `docs/RELEASE-0.11.0.md`; not yet booted). Next here: copy-on-write `fork`, signals, a real VFS.
2. NVMe driver (the OMEN's internal disk; read-only first, because Windows/BitLocker lives there).
3. I2C-HID touchpad (ELAN07F0 over the Intel serial-IO I2C controller, found through ACPI).
4. RTL8852BE Wi-Fi driver (firmware, MAC/PHY tables, DMA rings): needs the laptop in the loop.
5. TLS 1.3 (X25519, ChaCha20-Poly1305 or AES-GCM, certificate validation) so `https://` works.
6. SMP, a journaling or crash-safe writable filesystem beyond FAT32, ACPI sleep, audio, GPU modesetting.

**Advanced:** none of it is attempted. The honest distance to "daily driver" is large: Linux and Windows have decades of
driver and application ecosystem. Aeon's value is that all of it is small enough to read, test and explain.

## Specific notes about the HP OMEN 16 (this project's target)

| Part | Chip | Aeon 0.10 |
|---|---|---|
| Wi-Fi | Realtek RTL8852BE-VT (`10EC:B520`) | detected, **no driver**. Linux uses `rtw89`, needs `rtw8852bt` firmware, and has had firmware-version and crash bugs on this chip |
| Ethernet | Realtek RTL8168 (`10EC:8168`) | driver written from public Linux/iPXE sequences, **never run on this hardware** |
| Touchpad | ELAN07F0 over I2C-HID | **not supported** (a USB mouse works) |
| USB | Intel xHCI `8086:7A60` | generic xHCI driver, tested only in QEMU |
| Internal disk | Samsung NVMe | **not touched** (no driver; deliberate) |
| Firmware | HP BIOS F.14, Secure Boot off | boots unsigned or signed |

## Sources

- Redox status reports and coverage: [Phoronix, April 2026](https://phoronix.com/news/redox-os-april-2026), [Phoronix, January 2026](https://phoronix.com/news/redox-os-january-2026). The "many laptops still lack Wi-Fi/GPU/sound" remark comes from a low-quality aggregator and is marked as such.
- xv6 and MINIX 3 descriptions: [Wikipedia: Xv6](https://en.wikipedia.org/wiki/Xv6), [OSDev wiki: Xv6](https://wiki.osdev.org/Xv6).
- RTL8852B firmware issues in Linux: [kernel bugzilla 217207](https://bugzilla.kernel.org/show_bug.cgi?id=217207), [Red Hat bug 2182001](https://bugzilla.redhat.com/show_bug.cgi?id=2182001), [LKML, 2026, RTL8852BE-VT firmware crashes](https://lkml.iu.edu/2607.3/03978.html).
