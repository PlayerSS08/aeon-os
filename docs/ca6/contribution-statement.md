# Individual contribution statement (CA-6)

Project: Aeon Micro-OS (`S:\Works\Projects\Aeon\OS`)  
Specialization chosen: Security (Containment, Boot Trust, and Hardware Privilege Separation - Track F)  
Developer: SREEHARAN  
Roll Number: 25BTRCL257  

| Team member | Roll number | Owned subsystems | Files explained line by line |
|---|---|---|---|
| SREEHARAN | 25BTRCL257 | Security, Containment, Scheduler, Linux Syscall ABI, and Network Architecture | `main.rs`, `sched.rs`, `usermode.rs`, `syscall.rs`, `paging.rs`, `pci.rs`, `netdev.rs`, `netstack.rs` |

## What I personally did

I architected and implemented Aeon Micro-OS from bare-metal UEFI boot up to a preemptive multitasking kernel with hardware privilege separation, Linux ABI compatibility, and network stack integration:

1. **Bare-Metal Boot & Cryptographic Chain:** Built native UEFI entry (`main.rs`, `uefi.rs`), TPM 2.0 command-line PCR 12 measurement with verified event-log replay (`tcg2.rs`, `eventlog.rs`), and seed derivation across reboots (`entropy.rs`).
2. **Memory Protection & W^X Enforcement:** Designed identity-mapped 4-level page tables with strict W^X separation, unmapped null page, and kernel stack guard pages (`paging.rs`).
3. **Preemptive Task Scheduling & Verification:** Implemented 100 Hz LAPIC timer-driven preemptive round-robin scheduling (`sched.rs`, `apic.rs`, `readyq.rs`) with empirical proof of zero missed preemptions via circular flight recorder telemetry (`schedtrace.rs`).
4. **Ring 3 Hardware Containment & Linux Syscall ABI:** Configured `MSR_LSTAR` for hardware privilege transition (`syscall`/`sysretq`), user-pointer validation with `stac`/`clac` SMAP protection (`usercopy.rs`), and standard Linux x86_64 system call handlers (`read`, `write`, `open`, `close`, `lseek`, `mmap`, `brk`, `sched_yield`, `getpid`, `exit`, `clock_gettime`, `socket`, `connect`, `sendto`, `recvfrom`).
5. **Universal Network Stack & Package Management:** Engineered the unified network device driver interface (`netdev.rs`), Intel e1000 and VirtIO-Net drivers, Ethernet/ARP/IPv4/ICMP/UDP/DHCP/TCP network stack (`netstack.rs`), Linux network commands (`ip`, `ifconfig`, `ping`, `dhcp`, `curl`), and Debian `apt`/`aptitude` package management subsystem (`apt.rs`).

## What I used AI for

AI assistance (Claude Code and Antigravity) was utilized under my explicit direction for drafting baseline template structures, accelerating boilerplate driver register definitions, researching RFC specifications (RFC 792, RFC 793, RFC 1071, RFC 2131, RFC 822), and generating initial unit test fixtures. Every generated line was reviewed, verified for memory safety, clippy-checked, and tested against UEFI execution in VirtualBox and QEMU.

## Subsystems, and the questions I can answer about each

| Subsystem | Three things I can explain without notes |
|---|---|
| **CPU Privilege Separation & Ring 3 Entry** | 1. How `MSR_LSTAR` routes the `syscall` instruction and how `sysretq` switches CPL from 0 to 3.<br>2. Why `SFMASK` must clear `RFLAGS.IF` and `RFLAGS.AC` to enforce kernel isolation and SMAP protection.<br>3. How user-mode pointer bounds are verified against the user window before kernel reads/writes. |
| **Scheduler & Preemption** | 1. How the Local APIC timer ISR pushes the register context onto the kernel stack and calls `aeon_sched_irq`.<br>2. How the FIFO ready queue ensures fair $1/n$ Round-Robin quantum distribution.<br>3. How stack canaries detect stack overflow and trigger instant panic containment. |
| **Paging & W^X Memory** | 1. How PML4, PDPT, PD, and PT tables are mapped identity up to 1 TiB.<br>2. How the NX (No-Execute) bit prevents code execution on data pages.<br>3. Why 2 MiB large pages are split into 4 KiB pages around the PE executable sections. |
| **Networking & Device Drivers** | 1. How Intel e1000 DMA descriptor rings (RDBA/TDBA) communicate with PCI bus mastering.<br>2. How the RFC 1071 1's complement checksum is calculated and folded.<br>3. How the DHCP DORA handshake auto-assigns IP, netmask, and gateway routes. |

Signed: SREEHARAN (25BTRCL257)  Date: November 14, 2026
