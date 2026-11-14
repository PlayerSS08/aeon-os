# AI usage disclosure (CA-6)

Student Name: SREEHARAN  
Roll Number: 25BTRCL257  
Project: Aeon Micro-OS (`S:\Works\Projects\Aeon\OS`)  
Specialization: Security and Isolation (Track F)  

## Tools used

| Tool | Used for |
|---|---|
| Claude Code & Antigravity (Anthropic & DeepMind coding assistants) | Drafting code skeletons, hardware register tables, unit tests, and technical documentation under direct guidance |
| QEMU, OVMF, VirtualBox 7.x, swtpm, `cargo`, `clippy`, Python 3 | Building, testing, verifying, and measuring kernel and device driver execution |

## What the AI did

| Area | AI contribution |
|---|---|
| Kernel code | Assisted with boilerplate Rust source definitions under student direction, including register structures for e1000/VirtIO, RFC checksum algorithms, and driver trait implementations. |
| Tests | Drafted host unit test assertions and test fixture helpers. |
| Documents | Formatted initial technical reports and viva question catalogs from project telemetry. |
| Review | Performed static analysis sweeps for unsafe code invariants and documentation consistency. |

## What the student did

- Formulated the entire system design, architectural constraints, and roadmap for Aeon.
- Selected the zero-dependency bare-metal `no_std` architecture, refusing all third-party runtime crates.
- Directed the implementation of the UEFI boot flow, TPM 2.0 measured boot, and W^X memory paging.
- Implemented and debugged Ring 3 user mode privilege separation, SMAP validation, and Linux system call ABI.
- Designed and verified the universal network driver subsystem (Intel e1000 and VirtIO-Net) and protocol stack (ARP, IPv4, ICMP, DHCP, TCP, HTTP).
- Configured and debugged Oracle VirtualBox and QEMU test environments, analyzing hardware traces and crash dumps.
- Audited all code for correctness, safety invariants, and performance, ensuring 100% test pass rates and zero clippy warnings.

## How the AI output was checked

| Check | How verified |
|---|---|
| Lint & Safety | `cargo clippy --target x86_64-unknown-uefi -- -D warnings` with zero warnings allowed |
| Unit Logic | Host unit tests executing all algorithmic parsers, queue structures, and network arithmetic |
| Runtime Integrity | Live UEFI boot verification on VirtualBox 7.x and QEMU with serial log telemetry |
| Invariant Verification | Every single `unsafe` block verified with documented `SAFETY:` justification comments |
| Operational Reality | Verified on real UEFI firmware specifications and hardware datasheets |

## Statement

I certify that the code and documentation in this repository were designed, reviewed, tested, and validated by me. I understand and can defend every architectural decision and low-level mechanism implemented in Aeon Micro-OS.

Name: SREEHARAN (25BTRCL257)  Date: November 14, 2026
