# Aeon: design document (CA-6)

This document explains the choices behind each mechanism, what was rejected, and where the risky (`unsafe`) code lives. Numbers come from the 0.9.2 source tree.

## 1. Design principles

| Principle | What it means in the code | Why |
|---|---|---|
| Pure logic, thin hardware layer | Allocators, wait list, file system, FAT reader, parsers, scheduler policies and the Banker's algorithm take plain data and return plain data. Hardware access is a thin wrapper around them. | The logic can be unit-tested on the PC in milliseconds. 237 host tests run without a VM. |
| One CPU, so interrupts are the only concurrency | The only way two pieces of code touch the same data at once is an interrupt arriving in the middle of the first. | Keeps locking simple, and makes the rule checkable (section 5). |
| Small and readable over fast | Round robin over an array, a first-fit heap, a bitmap frame allocator. | The goal is to make the mechanisms visible, as the assignment asks. |
| Fail loudly | A broken invariant stops the kernel with a message (panic screen) instead of continuing. | A silent wrong answer in a kernel is harder to debug than a stop. |

## 2. Scheduler (milestones 3 to 5)

| Item | Design | Alternative not taken, and why |
|---|---|---|
| Task storage | Fixed array of 16 slots (`MAX_TASKS`) with a 32 KiB stack and a guard page each | A linked list: needs the heap before the scheduler exists, and a heap bug would hide a scheduler bug |
| Ready queue | `ReadyQueue<16>` (`readyq.rs`): a first-in-first-out ring of task ids holding exactly the tasks in state `Ready`; insert at the back, select from the front, remove on `kill` | Scanning the task array for the next ready slot: it works, but hides insertion, selection, removal and ordering, which the assignment asks to see. The invariant "queue = ready tasks" is checkable with the `queue` command |
| Policy | Round robin, one 10 ms slice (100 Hz tick), no priorities | MLFQ and priorities are written and tested as pure code in `schedpolicy.rs` but are not wired in: a priority scheduler needs tickless idle and priority classes first (see the limits section of the report) |
| Preemption | LAPIC timer interrupt calls `sched::tick`; the switch routine saves FPU/SSE state, 15 general registers and the interrupt frame on the old stack and restores them from the new one | Cooperative only: a task that never yields would hang the system. Test TC1 shows two never-yielding workers sharing the CPU |
| Timer | LAPIC timer calibrated against the TSC; the legacy PIT is the fallback (`nolapic` option) | PIT only: slower to program and it cannot be per-core later |
| Sleeping | `sleep_ms` sets a wake tick and yields; granularity is one tick (10 ms) | A timer wheel: unnecessary for 16 tasks |

The context-switch frame layout is fixed by the code in `sched.rs`: 512 bytes of `fxsave` area, 15 general registers and a 5-word `iretq` frame (`FRAME_QWORDS = 512/8 + 15 + 5`). A new task is built by writing such a frame by hand, so starting a task and resuming one use the same path.

## 3. Synchronization (milestone 6)

| Mechanism | File | Behaviour |
|---|---|---|
| `SpinLock<T>` | `spin.rs` | Switches interrupts off, takes the lock, restores the old interrupt state when the guard drops. On one CPU, finding it already taken means a bug, so the kernel stops after 4 million spins instead of hanging. |
| `WaitQueue` | `sched.rs` with `WaitList` in `waitq.rs` | `wait()` puts the current task on the list, marks it `Blocked` and yields. `wake_all()` marks every listed task `Ready`. A blocked task is never picked, so it uses no CPU. The list holds at most 16 ids and a task appears once. |
| Disk busy flag | `vblk.rs` | A slow disk request cannot run with interrupts off, so an atomic flag says who owns the disk. A second task sleeps 10 ms and retries. |

Why a flag for the disk and a spinlock for the rest: a spinlock holds interrupts off for the whole critical section. The disk driver polls for up to 2 seconds, which would stop the 100 Hz timer and the keyboard. A spinlock is right for short sections (a few instructions); the flag is right for long ones.

## 4. Memory (milestone 7)

| Layer | Algorithm | Cost | Notes |
|---|---|---|---|
| Frames (`frames.rs`) | One bit per 4 KiB frame, next-fit search with a cursor | O(n) worst case, usually O(1) after the first allocation | Allocating `n` contiguous frames scans for a run. `free_frames` clears bits. Free count is kept, so `mem` is O(1). |
| Heap (`heapalloc.rs`) | First-fit free list with coalescing of neighbours; 16-byte blocks | O(holes) per allocation | Carved from up to 32 MiB of frames. Implements `GlobalAlloc`, so `Vec` and `String` work in the kernel. |
| Task stacks | 8 frames plus a guard page, wiped before the frames are freed | | Stops stack overflow from silently overwriting a neighbour and stops old kernel data leaking to the next owner |
| Paging (`paging.rs`) | Identity map to 1 TiB with W^X, an unmapped null page and guard pages | | No per-program tables yet (limit) |

Fragmentation is visible: `mem` prints the number of free holes in the heap and the largest free run of frames.

## 5. The interrupts-off rule

On one CPU, a `static` that an interrupt handler can reach is the main source of bugs. Aeon states one rule and checks it:

1. If no handler and no other task touches a static, it needs no protection (rule A).
2. If one can, interrupts must be off while the `&mut` lives (rule B). `Global::get_irqs_off()` stops the kernel with a message when they are on.
3. If the work is slow, use an owner flag and sleep while waiting (rule C).

`docs/design/globals.md` lists every global static and its rule. Before this check existed, the rule was only a comment, and one fault-path function (`log::reset_after_fault`) cleared a busy flag without stopping interrupts. It now switches interrupts off itself.

## 6. Where the `unsafe` code is

Rust forbids unsafe operations inside an `unsafe fn` unless they sit in their own `unsafe` block (`#![deny(unsafe_op_in_unsafe_fn)]`). Counted with `grep` over `src/`: 245 unsafe blocks, functions and impls; 212 `SAFETY:` comments, with the rest covered by `# Safety` sections in function docs.

| File | Sites | Why unsafe is needed there |
|---|---|---|
| `arch.rs` | 62 | Port I/O, control registers, descriptor tables, inline assembly for the switch and interrupt stubs |
| `main.rs` | 47 | Calling firmware (UEFI) functions through raw tables |
| `sched.rs` | 24 | Building and switching stack frames, the scheduler and trace statics |
| `mm.rs`, `heapalloc.rs` | 16 each | Raw memory handed out and taken back |
| `paging.rs` | 14 | Writing page tables |
| `vblk.rs` | 12 | Device registers and DMA memory |

Every site carries a `SAFETY:` comment naming the invariant it relies on. The invariants are tested where a test can reach them: the allocators, wait list and file system run under unit tests on the PC; the hardware wrappers are exercised by the 359 QEMU checks, which include deliberate faults (invalid opcode, page fault, stack overflow, W^X violation, NX violation, panic).

## 7. Specialization: Security (containment and boot trust)

The assignment lets a team pick one specialization. Aeon's is Security. It was chosen because the mechanisms already existed and are testable end to end, so the evidence is real rather than staged.

| Question | Mechanism | Evidence |
|---|---|---|
| Can a bad program damage the kernel? | Ring 3, user window, pointer checks, SMEP/SMAP | TC5: `run fault`, `run gp`, `run badcall` end only the program; frames and tasks are intact afterwards |
| Can data run as code? | W^X page tables, NX | Fault-injection images `fault-wx` and `fault-nx` must stop with a clear message (boot-test group `faults`) |
| Is the boot path trustworthy? | Signed image, measured boot, event-log replay, SBAT | Boot tests with a software TPM and Secure Boot firmware |
| What if the kernel keeps failing to boot? | Boot counter, recovery mode after 3 failures | Boot-test group `bootcount` |

## 8. Known design limits

| Limit | Consequence |
|---|---|
| One shared address space | A ring-3 program is contained by page permissions and pointer checks, but not by a private page table |
| One CPU | No parallelism; the scheduler is simple because of it |
| Round robin only | No priorities; a busy task slows the shell (see TC4) |
| RAM file system | Files vanish on reboot |
| Read-only disk driver, virtio only | A real PC's SATA or NVMe disk is not seen |
