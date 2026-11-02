# Viva preparation (CA-6)

The assignment says it provides 20 sample viva questions; I could not read them (the document was only available as a summary). These are the questions an examiner would most likely ask about this particular kernel, with short answers tied to the code. Learn the reasons, not the wording.

## Scheduling and timer

| Question | Short answer | Where to point |
|---|---|---|
| How does preemption work? | The LAPIC timer fires 100 times a second. The handler calls `sched::tick`, which saves the running task's registers on its stack, picks the next ready task round robin, and returns on that task's stack. | `apic.rs`, `sched.rs`, switch stub in `arch.rs` |
| Why can two tasks that never yield still share the CPU? | Only the timer interrupt takes the CPU away; the tasks do not have to cooperate. TC1 runs two such workers and gets a fairness ratio near 1. | TC1 in `results.csv` |
| What is saved in a context switch? | 512 bytes of FPU/SSE state (`fxsave`), 15 general registers and the 5-word interrupt return frame. | `FRAME_QWORDS` in `sched.rs` |
| What is the time slice? | 10 ms (100 Hz). | `TIMER_HZ` in `arch.rs` |
| What happens if no task is ready? | Task 0, `idle`, runs and halts the CPU until the next interrupt. | `sched.rs` |
| Why not a priority scheduler? | Round robin is the required baseline. FCFS, SJF, SRTF, priority and RR are implemented and tested as pure code in `schedpolicy.rs` against the course's worked examples, but wiring priorities in needs tickless idle and priority classes first. | `schedpolicy.rs` |

## Synchronization

| Question | Short answer |
|---|---|
| Why does the spinlock switch interrupts off? | Otherwise the timer could interrupt the holder and run code that wants the same lock, which on one CPU would spin forever. |
| What stops a deadlock on the spinlock? | On one CPU, finding it taken is a bug, so after about 4 million spins the kernel stops with a message instead of hanging. |
| How does a blocked task avoid using CPU? | `wait()` marks it `Blocked` and yields; the scheduler skips `Blocked` tasks until `wake_all()` marks them `Ready`. M6 shows 0 ticks over 3 seconds. |
| Why a flag, not a spinlock, for the disk? | A request can poll for 2 seconds. A spinlock would keep interrupts off for all of it, stopping the timer. |

## Memory

| Question | Short answer |
|---|---|
| How are frames allocated? | A bitmap, one bit per 4 KiB frame, next-fit search. |
| How does the heap work? | First-fit free list, with neighbouring free blocks merged on free. `mem` shows used bytes and the number of free holes. |
| How do you know nothing leaks? | TC4 compares free frames before and after a heavy run, and heap use before and after writing and removing 20 files. |
| What is W^X? | A page is never writable and executable at the same time, so data cannot be run as code. |
| What is the guard page below each stack? | An unmapped page; overflowing the stack faults instead of silently overwriting memory. |

## Security (specialization)

| Question | Short answer |
|---|---|
| What does ring 3 give you? | The CPU refuses privileged instructions and memory the page tables do not allow. The only way back is the `syscall` gate. |
| What if a program passes a bad pointer to a system call? | `usercopy` checks that the range is inside the user window, with no overflow, before the kernel touches it; SMAP is lifted only during the copy. |
| What happens when a user program faults? | `user_fault` reports it and ends that task; the kernel carries on (TC5: `run fault`, `run gp`). |
| What is measured boot? | The firmware and the loader record hashes of what started into TPM registers, so a later check can see if the path changed. |
| What is recovery mode? | After 3 failed boots in a row, the next boot is a minimal one: no timer, no scheduler, a single-task shell. |

## Rust and `unsafe`

| Question | Short answer |
|---|---|
| Why `unsafe` at all? | Port I/O, page tables, raw memory and assembly cannot be expressed in safe Rust. |
| How do you limit it? | 239 sites in about 14,500 lines, each with a `SAFETY:` comment; the compiler setting `deny(unsafe_op_in_unsafe_fn)` forces every unsafe operation into its own block. |
| What is the rule for global state? | See `global.rs` and `docs/design/globals.md`: nobody else, interrupts off (checked at run time), or an owner flag. |
| Why `no_std`? | There is no operating system under the kernel to provide the standard library. |

## Honest limits to be ready to state

One CPU. One shared address space. RAM-only files. Read-only virtio disk, so a real PC's disk is not seen. Real-hardware boot not yet tested. Image is 250,408 bytes, near the 256 KiB test limit.
