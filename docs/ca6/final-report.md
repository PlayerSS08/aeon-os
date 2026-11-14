# Aeon: a micro-OS in Rust

**CA-6 final technical report.** Project: Aeon 0.9.2, `S:\Works\Projects\Aeon\OS`. Specialization: Security and Isolation (Track F). Team and individual roles: see `contribution-statement.md` (by SREEHARAN, 25BTRCL257).

## 1. Title

Aeon: a small, tested, preemptive micro-OS in Rust, with a security and isolation specialization.

## 2. Abstract

Aeon is a small x86_64 operating system written in `no_std` Rust with no third-party crates. It boots from UEFI, keeps its tasks in a fixed table and its runnable tasks in a first-in-first-out ready queue, switches between them with round robin on a 100 Hz local-APIC timer, and records every context switch with its reason in a scheduler trace. It provides an interrupt-safe spinlock and a blocking wait queue, allocates memory with a frame bitmap and a first-fit heap, logs through four sinks, and runs ring-3 programs behind a system-call gate. The specialization is security and isolation: a signed and measured boot path, W^X and NX paging, SMEP/SMAP/UMIP, contained user faults, and boot counting with a recovery mode. The work was checked on three levels: 237 unit tests, a QEMU boot suite, and a dedicated evidence run (111 checks) whose Python parser reads the kernel's own scheduler trace. Measured: workers that never yield lose the CPU to the timer in 100% of cases; every time slice is exactly one 10 ms tick; the order of turns repeats perfectly; three jobs of 150, 300 and 600 units finish with turnaround ratios 1.701 and 2.362 against 1.667 and 2.333 predicted by equal-share round robin; two unsynchronized tasks lose exactly half of their additions to a shared counter while the same tasks with a spinlock lose none; and a request for more memory than exists is refused without changing the heap. Not done: real-hardware boot, per-program address spaces, disk writes.

## 3. Problem statement

Production operating systems hide their mechanisms behind millions of lines of code. The assignment asks whether a student can implement the mechanisms themselves: start a kernel, represent tasks, queue them, schedule them with a timer, protect shared data and manage memory, and then show with evidence, not with "it runs", that each one works. The extra problem Aeon sets itself is containment: a program, a failed boot or a stray interrupt must not be able to take the whole system down.

## 4. Objectives

| # | Objective | How it was judged |
|---|---|---|
| 1 | Boot a Rust kernel and make its activity visible | Boot log with numbered stages; levels of messages |
| 2 | Represent tasks and queue them | Task table, ready queue, `ps`, `queue` |
| 3 | Round robin, driven by a timer | Scheduler trace: order, slices, reasons |
| 4 | Show cooperative versus preemptive switching | `spawn coop` against `spawn`: 25 times the switch rate |
| 5 | Protect shared data | `race` and `race lock`: lost updates, then none |
| 6 | Allocate memory and handle failure | Frame and heap statistics, `alloc` refusal path |
| 7 | Contain bad programs (specialization) | Test case TC5; fault-injection boot images |
| 8 | Prove it | Six test cases, repeatability, charts, raw logs |

## 5. OS concepts

| Concept | Where it appears in Aeon |
|---|---|
| Process / task | Task table; states free, ready, running, sleeping, blocked, dead (the assignment's READY, RUNNING, BLOCKED, TERMINATED) |
| Context switch | Registers and FPU state saved on the task's own kernel stack |
| Scheduling policy | Round robin (live); FCFS, SJF, SRTF, priority as a tested simulator |
| Preemption and the timer interrupt | LAPIC timer, 100 Hz |
| Race condition and critical section | `race`; `SpinLock` |
| Blocking and wake-up | `WaitQueue` |
| Physical and kernel memory | Frame bitmap; first-fit heap with coalescing |
| Protection rings, system call | Ring 3 window and the `syscall` gate |
| W^X, NX, guard pages, SMEP/SMAP | Paging and CPU feature setup |
| Secure and measured boot | Signed image, TPM PCRs, boot counting, recovery |

**How Aeon differs from Linux** (the comparison the assignment asks for; the Linux column is general knowledge of the mainline kernel, not measured here):

| Area | Linux | Aeon |
|---|---|---|
| Task / process | `task_struct`; processes and threads share one structure; thousands of tasks; many states | 16 fixed slots; one structure; six states |
| Scheduler | Several classes (real-time, fair, deadline); the fair class has been CFS and, in the 6.x kernels, EEVDF; one run queue per CPU; priorities and nice values | One CPU, one FIFO ready queue, round robin, 10 ms slice, no priorities |
| Memory | Virtual memory per process, demand paging, buddy allocator under slab allocators, page cache, swapping | One shared identity-mapped address space; frame bitmap under one first-fit heap; no swapping |
| Synchronization | Atomics, spinlocks, mutexes, semaphores, RCU, futexes | One interrupt-safe spinlock, one wait queue, one owner flag |
| I/O | Virtual file system, block layer with I/O schedulers, thousands of drivers | Polling virtio disk (read-only), read-only FAT, RAM file system |
| Security | Users and permissions, capabilities, namespaces, seccomp, security modules, address-space randomization | Signed and measured boot, ring 3 with pointer checks, W^X, SMEP/SMAP; no users |
| Hardware support | Many architectures, SMP, power management, ACPI | x86_64 UEFI, one CPU, QEMU-tested |

The purpose of the comparison is the one the assignment gives: Linux's complexity grows out of the same simple mechanisms Aeon shows in miniature.

## 6. Architecture

See `architecture.md` for the layer table, the data-and-control-flow diagram (boot, kernel entry, task management, scheduler, timer, synchronization, memory, system interface, specialization, emulator) and the task-state table. In short: UEFI loads a signed image; the boot stage builds one `BootInfo` and leaves the firmware; the kernel sets up tables, paging, memory and logging; the boot thread becomes task 0; the timer interrupt calls the scheduler, which pops the ready queue, records the switch and resumes the next task.

## 7. Design decisions

| Decision | Alternative | Why this one |
|---|---|---|
| Pure logic in small modules (queue, trace, allocators, wait list, policies), thin hardware wrappers | Logic written directly against hardware | Unit tests on the PC in milliseconds; 237 tests |
| Ready queue as a ring of ids that holds exactly the `Ready` tasks | Scan the task array for the next ready slot | The assignment's layer 4 asks for insertion, selection, removal and ordering to be visible; scanning hides them. Invariant checkable at run time (`queue`) |
| Scheduler trace in a ring of 1024 events, reason recorded | Counters only | A trace lets Python check order, slices and latency instead of trusting totals |
| One CPU | SMP | Keeps the scheduler simple; makes interrupts the only source of concurrency, and that rule is checkable |
| Interrupts-off rule enforced in code (`get_irqs_off`) | A comment | A rule that is only a comment gets broken |
| Disk guarded by an owner flag, not a spinlock | Spinlock | A disk request can poll for 2 s; a spinlock would stop the timer for that long |
| Trace and other big arrays in `.bss` | In the image | Image size budget (256 KiB) |

## 8. Rust implementation

| Aspect | How it is done |
|---|---|
| No standard library, no dependencies | `no_std` plus `alloc` backed by Aeon's own heap; `Cargo.toml` lists no dependencies |
| Testability | Pure modules also compile for the PC; kernel-only modules are gated with `cfg(target_os = "uefi")` |
| Types carry the rules | Enums for task state, trace reason and every error (`FsError`, `RunError`) force each case to be handled; const generics fix queue and list sizes at compile time (`ReadyQueue<16>`, `WaitList<16>`, `TraceRing<1024>`) |
| Unsafe | 245 sites in about 15,300 lines, 212 `SAFETY:` comments, the rest covered by `# Safety` sections; `deny(unsafe_op_in_unsafe_fn)`; by file: `arch.rs` 62, `main.rs` 47, `sched.rs` 24, `mm.rs` and `heapalloc.rs` 16 each |
| Lint and builds | `clippy -D warnings` clean in kernel and host modes; two clean builds give the identical hash |

## 9. Scheduling

Tasks join the back of the ready queue when created, woken from sleep or from a wait queue, or preempted. On each timer tick the scheduler wakes due sleepers, requeues the running task, pops the front, records `(tick, from, to, reason)`, and resumes the chosen task; with an empty queue it runs the idle task. Measured from the scheduler trace of four busy workers over 6 s:

| Property | Result |
|---|---|
| Task order | The workers take turns in the same cycle: 100% of turns match the turn one cycle earlier |
| Time slice | 100% of worker stretches last exactly one tick (10 ms) |
| Why a worker leaves the CPU | 100% timer |
| Scheduling latency | Mean 3.07 ticks, max 4 (4 workers: expected about 3) |
| Share of CPU | 1/n each (98.1%, 49.0%, 19.6% to 19.8%, 10.9% for 1, 2, 5, 9 busy tasks); fairness ratio (smallest share over largest) 1.000, 0.992 and 0.986 for 2, 5 and 9 tasks |
| Cooperative alternative | `spawn coop`: 95.9% of switches are `yield`, 4431 switches per second against 182, 24 times more, because the tick no longer limits them |
| Task completion | Jobs of 150, 300, 600 units end in that order; turnaround 127, 216, 300 ticks; ratios 1.701 and 2.362 against 1.667 and 2.333 predicted; response time at most 2 ticks; finished jobs leave the task table |

## 10. Synchronization

Three mechanisms: `SpinLock` (interrupts off for a short critical section; stops the kernel after about 4 million spins on a lock it already holds), `WaitQueue` (a blocked task is out of the ready queue and uses no CPU), and an owner flag for the disk. The shared-resource problem was demonstrated with `race`: two tasks each add 1 to a counter 20,000 times, each addition being a read, some work, then a write.

| Run | Expected | Got | Lost |
|---|---|---|---|
| no lock, run 1 | 40000 | 20000 | 20000 (50%) |
| no lock, run 2 | 40000 | 20000 | 20000 (50%) |
| with spinlock, run 1 | 40000 | 40000 | 0 |
| with spinlock, run 2 | 40000 | 40000 | 0 |

The result is exactly 50% twice because the emulator is deterministic: each time a task is preempted in the middle of its read-modify-write, the other task's slice of work is overwritten when it resumes. A blocked task used 0 ticks over a 3 s window; `wake` released exactly one waiter.

## 11. Memory management

A frame bitmap (4 KiB frames, next-fit) under a first-fit coalescing heap that implements `GlobalAlloc`. Stacks have guard pages and are wiped when freed. Measured: free frames were 55124 before and after a stress of nine busy tasks, ten ring-3 launches and twenty files; heap use returned from 2 KiB (20 files) to 1 KiB against a 0 KiB start. Allocation of 1, 64, 1024, 8192 and 16384 KiB took 419, 10, 8, 9 and 6 microseconds: time does not grow with size because first-fit takes a hole without touching the memory; the first call is much slower (419 microseconds; the cause was not investigated). Failure path: `alloc 99999999` is refused with "nothing changed", and the heap's used size and hole count are identical before and after; code using the fallible `try_reserve` gets an error, a failed frame allocation makes `spawn` return `None`, and only code that uses the infallible `Vec::push` on exhaustion would panic.

## 12. Specialization component: Security and Isolation (Track F)

Trust boundaries, the controlled interfaces that cross them, and what enforces each are tabulated in `architecture.md` section 5 (firmware and kernel; ring 0 and ring 3; user pointer and kernel memory; task and task; interrupt and task context; boot attempt and next boot). Evidence:

| Claim | Evidence |
|---|---|
| A faulting or hostile ring-3 program cannot harm the kernel | TC5: page fault, general-protection fault and a bad system call each ended only the program; the kernel kept answering; no frames leaked; no task left behind |
| Data cannot run as code | Fault-injection images for W^X and NX violations stop with a clear message (boot-suite group `faults`) |
| The boot path is checkable | Signed image, SBAT, measured boot with event-log replay (groups `secureboot`, `release`) |
| A repeatedly failing boot recovers | After 3 failed boots, a minimal recovery boot (group `bootcount`) |
| Shared kernel state is protected | The checked interrupts-off rule; `docs/design/globals.md` |

## 13. Testing

Three levels (`test-plan.md`): unit tests (237), a QEMU boot suite (new group `sched`, 37 checks, for layers 4 to 8), and the CA-6 evidence run. The six required cases are TC1 (two tasks), TC2 (five runnable tasks and a blocked one), TC3 (a lone task, three refused kills), TC4 (nine busy tasks, ten ring-3 launches, twenty files), TC5 (twelve hostile requests), TC6 (three fresh boots).

## 14. Results

111 evidence checks passed, 0 failed; the boot suite and unit tests pass; the release image checks pass. The testing found things the code alone did not show: a one-time 64 KiB ring-3 window that first looked like a leak, a limit of one ring-3 program at a time, a 1 KiB heap residue, an arithmetic error in the course notes (see the debugging log), and the fact that the first version of several of my own tests measured the wrong thing.

## 15. Performance analysis

See `performance.md`. Metrics requested by the assignment and what was done with each:

| Metric | Measured | Value |
|---|---|---|
| Task-switch count | yes | 182 per second timer-driven, 4431 per second cooperative |
| Scheduling latency | yes | mean 3.07 ticks, max 4 |
| Response time | yes | at most 2 ticks for three simultaneous jobs |
| Throughput | yes | 350 work units per second (about 2.9 ms per unit in QEMU) |
| Fairness | yes | ratio 0.986 to 1.000 across configurations |
| Allocation time | yes | 6 to 10 microseconds after the first call |
| Memory utilization | yes | heap 0 of 32768 KiB at rest; frames identical before and after stress |
| Synchronization wait time | no | the spinlock never waits on one CPU; not measured |
| I/O latency | no | the disk driver is read-only and was not timed |

## 16. Limitations

| Limit | Effect |
|---|---|
| One CPU | No parallelism |
| One address space | Ring-3 programs are contained by permissions and pointer checks, not private page tables |
| One ring-3 program at a time | A second `run` is refused while one is alive |
| Programs are built in | No ELF loader |
| RAM-only files, read-only virtio disk | Files vanish on reboot; a real PC's disk is not seen |
| Round robin only | No priorities |
| QEMU only for measurements | Absolute speeds are not meaningful; no real-hardware data |
| Timer rate measured from outside | 96 Hz seen against a nominal 100; kernel-side check not done |
| 100 Hz tick never stops | Idle power is not minimized |
| Image size | 259,072 bytes unsigned, about 260.6 KB signed, within 1.5 KB of the 256 KiB limit |
| Infallible allocation panics | Plain `Vec::push` on exhaustion stops the kernel |

## 17. Lessons learned

1. A test that fails is information. Several failures were my own measurement mistakes (counting the shell as a worker, starting "simultaneous" jobs by typing three commands slower than a job runs), and one was the course notes being wrong.
2. A rule that lives only in a comment will be broken: the interrupts-off rule is now checked by the machine.
3. Check the cheap invariant first: turnaround = waiting + burst; free frames before = after; the ready queue = the ready tasks.
4. A trace beats a total. "Switches: 1044" says little; "every worker departure was a timer switch and the order repeated 100%" proves the scheduler.
5. Match the instrument to the effect. Typing a command takes longer than a short job; a 6-second window is plenty for tick-level fairness.

## 18. Future work

In dependency order: boot on real PCs; AHCI and NVMe drivers; IOAPIC and keyboard interrupts; per-program address spaces; an ELF loader; disk writes; USB keyboard; priority classes and tickless idle (the policy simulator is ready to guide them); a kernel-side timer check; fuzzing and Miri on the parsers; a semaphore and a producer-consumer demonstration.

## 19. References

Check each against what you actually read before submitting.

1. A. Silberschatz, P. Galvin, G. Gagne, *Operating System Concepts*, 10th ed. The course notes for Chapters 5 and 8 supplied the worked examples used as test data.
2. R. Arpaci-Dusseau, A. Arpaci-Dusseau, *Operating Systems: Three Easy Pieces*.
3. Intel, *64 and IA-32 Architectures Software Developer's Manual*, Volume 3.
4. UEFI Forum, *UEFI Specification*.
5. Trusted Computing Group, *TCG EFI Protocol Specification*.
6. OASIS, *Virtual I/O Device (VIRTIO) Specification* 1.x.
7. R. Cox, F. Kaashoek, R. Morris, *xv6: a simple, Unix-like teaching operating system*.
8. Project documents: `docs/HANDOFF.md`, `docs/design/`, and the research report `G:\Aeon\reports\Aeon and clg improvements.md`.

## 20. AI usage disclosure

See `ai-disclosure.md`. Claude Code (Anthropic) wrote and edited most of the code, tests and documents under the student's direction; the student reviewed and ran the work. The disclosure must be checked and signed by the student.
