# Aeon: a micro-OS in Rust. Final technical report (CA-6)

Project: Aeon 0.9.0, `G:\Aeon\OS` (commit `42fd956`). Specialization: Security. Team and roles: see `contribution-statement.md` (to be completed by the student).

## 1. Abstract

Aeon is a small x86_64 operating system written in `no_std` Rust with no third-party crates. It boots from UEFI, runs preemptive round-robin tasks on a 100 Hz local-APIC timer, provides a spinlock and a blocking wait queue, allocates memory with a frame bitmap and a first-fit heap, logs to four sinks, and runs ring-3 programs behind a system-call gate. The chosen specialization is security: a signed, measured boot path, W^X and NX paging, SMEP/SMAP/UMIP, contained user faults, and boot counting with a recovery mode. The work was validated by 225 unit tests, 322 QEMU boot checks and a dedicated evidence run of the six required test cases (73 checks, 0 failed). Measured results: CPU shares follow `1/n` exactly (two workers got 304 and 304 ticks), a blocked task uses no CPU, memory returns to its starting value after a 9-task stress, and 12 hostile requests were all contained. The kernel image is 250,408 bytes signed. Real-hardware boot, per-program address spaces and disk writes are not done.

## 2. Architecture

See `architecture.md` for the layer table and the data-and-control-flow diagram. In short: UEFI loads a signed image; the boot stage builds one `BootInfo` structure and leaves the firmware; the kernel sets up descriptor tables, paging, frames, heap and logging; the boot thread becomes task 0; the timer interrupt drives the scheduler; tasks reach services through function calls (kernel) or the `syscall` gate (ring 3).

## 3. Rust implementation

| Aspect | How it is done |
|---|---|
| No standard library | `#![no_std]` with `alloc` for `Vec` and `String`, backed by Aeon's own heap |
| No dependencies | `Cargo.toml` lists none; the font data is generated once and committed |
| Testability | Pure modules (allocators, wait list, file system, FAT, parsers, policies, Banker's) compile for the PC too; 225 unit tests run there. The kernel-only modules are gated with `cfg(target_os = "uefi")` |
| Unsafe | 239 sites in about 14,500 lines; each has a `SAFETY:` comment; `deny(unsafe_op_in_unsafe_fn)`; the invariants are in `design.md`, section 6 |
| Lint | `clippy -D warnings` is clean in both build modes |
| Reproducible build | Two clean builds give the identical file hash |

Choices that use Rust's strengths: fixed-size arrays with const generics for the wait list (`WaitList<N>`), enums for task state and errors (`FsError`, `RunError`) so every case must be handled, and `Option`/`Result` instead of null and error codes at module boundaries.

## 4. Scheduling

Round robin over a fixed array of 16 task slots, 10 ms slices, preemption from the timer interrupt, voluntary yield and sleep. A context switch saves the FPU/SSE state, 15 general registers and the interrupt frame on the old task's kernel stack. Measured: with `n` equal busy tasks each receives `1/n` of the CPU (98.2%, 49.0%, 19.7%, 10.9% for 1, 2, 5, 9 tasks) and the fairness ratio is 1.000 in every case. The other classic policies (FCFS, SJF, SRTF, priority, round robin with a quantum) are implemented as a pure simulator and reproduce the course's worked numbers, except one example in the notes that contains an arithmetic error.

## 5. Synchronization

A spinlock that switches interrupts off and stops the kernel when it finds itself waiting for itself; a wait queue that takes tasks out of scheduling until woken; and an owner flag for work too slow to run with interrupts off (the disk). The interrupts-off rule that makes these safe on one CPU is now checked at run time (`Global::get_irqs_off`), and `docs/design/globals.md` lists every global and its rule. Measured: a blocked task used 0 ticks over 3 s; `wake` released exactly one waiter.

## 6. Memory

A frame bitmap (4 KiB frames, next-fit) under a first-fit coalescing heap that implements `GlobalAlloc`. Task stacks have guard pages and are wiped when freed. Paging is an identity map with W^X. Measured: free frames were 55131 before and after a stress of nine busy tasks, ten ring-3 launches and twenty files; heap use returned from 2 KiB to 1 KiB against a 0 KiB start (1 KiB residue not explained).

## 7. Specialization: Security

| Claim | Evidence |
|---|---|
| A faulting or hostile ring-3 program cannot harm the kernel | TC5: page fault, general-protection fault and a bad system call each ended only the program; kernel and shell kept answering; no frames leaked; no task left behind |
| Data cannot run as code | Boot-suite groups `faults` (W^X and NX violation images stop with a clear message) |
| The boot path is checkable | Signed image, SBAT, measured boot with event-log replay; boot-suite groups `secureboot` and `release` |
| A repeatedly failing boot recovers | Boot counting: 3 failures lead to a minimal recovery boot; boot-suite group `bootcount` |
| Shared kernel state is protected | The checked interrupts-off rule, plus the audit fixes listed in the debugging log |

## 8. Testing

Three levels (`test-plan.md`): unit, boot suite, evidence run. The six required cases and their pass criteria are in the test plan; results are in `test-results.md`.

## 9. Results

73 of 73 evidence checks pass; the boot suite is 322 of 322; the unit tests are 225 of 225. The evidence run found three things worth noting: a "leak" that was a one-time 64 KiB window (the tests now warm it up), a design limit of one ring-3 program at a time, and a 1 KiB heap residue.

## 10. Performance

See `performance.md`: shares follow `1/n`; fairness 1.000; timer measured at 96.1 to 96.7 Hz from outside the VM (nominal 100); boot to shell 3.25 s of kernel time; repeatability within 1.5%.

## 11. Limitations

| Limit | Effect |
|---|---|
| One CPU | No parallelism |
| One address space | Ring-3 programs are contained by permissions and pointer checks, not by private page tables |
| One ring-3 program at a time | A second `run` is refused while one is alive |
| Programs are built in | No ELF loader, so no program can be loaded from a file |
| RAM-only files, read-only virtio disk | Files vanish on reboot; a real PC's SATA or NVMe disk is not seen |
| Round robin only | No priorities; a busy task slows others equally (TC4) |
| QEMU only for measurements | No real-hardware data; absolute speeds are not meaningful |
| 100 Hz tick never stops | Idle power use is not minimized |
| Heap residue of 1 KiB | Unexplained, within the test's tolerance |
| Timer rate measured from outside | A kernel-side check against the TSC would be cleaner |

## 12. Future work

In dependency order, from the research report (`G:\Aeon\reports`): boot on real PCs; AHCI and NVMe drivers so a real disk appears; IOAPIC and keyboard interrupts; per-program address spaces; an ELF loader; disk writes; USB keyboard; priority classes and tickless idle (the policy simulator is ready to guide them); fuzzing and Miri on the parsers.

## 13. References

Check each against what you actually read before submitting.

1. A. Silberschatz, P. Galvin, G. Gagne, *Operating System Concepts*, 10th ed. The course notes for Chapters 5 (CPU scheduling) and 8 (deadlocks) supplied the worked examples used as test data.
2. R. Arpaci-Dusseau, A. Arpaci-Dusseau, *Operating Systems: Three Easy Pieces* (scheduling, locks, address spaces).
3. Intel, *64 and IA-32 Architectures Software Developer's Manual*, Volume 3 (paging, interrupts, APIC, SMEP/SMAP).
4. UEFI Forum, *UEFI Specification* (boot services, Simple Text, GOP, TCG2 protocol).
5. Trusted Computing Group, *TCG EFI Protocol Specification* (measured boot).
6. OASIS, *Virtual I/O Device (VIRTIO) Specification* 1.x (the disk driver).
7. R. Cox, F. Kaashoek, R. Morris, *xv6: a simple, Unix-like teaching operating system* (design reference for the wait queue and the scheduler).
8. Project documents: `docs/HANDOFF.md`, `docs/design/aeon-system-design.md`, `docs/design/globals.md`, `docs/design/boot-kernel-split.md`, the research report `G:\Aeon\reports\Aeon and clg improvements.md`.

## 14. AI usage disclosure

See `ai-disclosure.md`. Claude Code (Anthropic) wrote and edited most of the code, tests and documents under the student's direction; the student reviewed and ran the work. The disclosure must be checked and signed by the student.
