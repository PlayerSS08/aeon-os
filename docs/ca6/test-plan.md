# Aeon: test plan (CA-6)

## 1. Three levels of testing

| Level | What it tests | How to run | Size |
|---|---|---|---|
| Unit tests (host) | Pure logic: ready queue, scheduler trace, allocators, wait list, file system, FAT, parsers, scheduler policies, Banker's algorithm | `cargo test --lib --target x86_64-pc-windows-gnu` (from `G:\Aeon\OS`) | 237 tests |
| Boot tests (QEMU) | The real kernel in a virtual PC: boot stages, shell, tasks, faults, Secure Boot, TPM, recovery, and the new `sched` group for layers 4 to 8 | `bash tests/run.sh` (about 12 minutes) | 359 checks |
| CA-6 evidence run | The six required test cases and the layer measurements, with logs, a Python trace parser and charts | `wsl -d Ubuntu -- python3 tests/ca6_evidence.py` (about 8 minutes) | 111 checks, 60 metrics |

Lint gates that must stay clean: `cargo clippy --release -- -D warnings` and `cargo clippy --target x86_64-pc-windows-gnu --lib -- -D warnings`.

## 2. The ten layers: what is built and what proves it

| Layer | Required by the assignment | Unit tests | Boot test (`sched` group and others) | Evidence run |
|---|---|---|---|---|
| 1 Boot | Kernel starts, prints a message, stays up | n/a | groups `boot`, `modes`, `menu`, `shell` | M1 |
| 2 Console and diagnostics | Information, warning/error and structured task/debug messages | `logfmt` tests | `commands` | M2 (stages and diagnostic commands) |
| 3 Task representation | id, state, context, scheduling info, stack | n/a | `ps` checks | TC1, TC2 |
| 4 Ready queue | Insertion, selection, removal, ordering | 7 tests in `readyq.rs`, including 5000 random operations against a reference queue | `queue` command, queue/state consistency after each scenario | L4/L5 |
| 5 Round robin | Task order, time-slice behaviour, repeated scheduling, trace, completion | 5 tests in `schedtrace.rs` | trace lines, `job` completion | L4/L5, L5: parsed scheduler trace |
| 6 Timer-driven scheduling | Timer connected to the scheduler; cooperative against timer-driven | n/a | `spawn coop` against `spawn`: reason counts | L6 |
| 7 Synchronization | Shared-resource problem, then the fix, then corrected execution | `SpinLock`, `WaitList` tests | `race`, `race lock` | L7, M6 |
| 8 Memory management | Allocator, memory statistics | frame bitmap and heap tests | `alloc`, `mem` | L8, TC4 |
| 9 System interface (optional) | request, kernel transition, service, return | `usercopy`, `syscall` tests | group `user` | TC5 |
| 10 Integration | boot, init, tasks, queue, timer, scheduler, execution, sync/memory, completion | n/a | all groups | TC4 |

## 3. The six required test cases

| TC | Name | Setup | Pass criteria |
|---|---|---|---|
| TC1 | Minimum configuration | Boot, `spawn` (two busy workers that never yield), 6 s window | Both workers get CPU; both counters advance; the two workers' ticks within 25% |
| TC2 | Multiple tasks | Two `spawn`, one `run spin` (ring 3), one `block`; 6 s window | 4 workers and the ring-3 task all get CPU; the blocked task gets none; runnable tasks within 40%; `wake` releases the waiter |
| TC3 | Boundary | Spawn two, kill one: a lone worker. Then `kill 0`, `kill 99`, `kill x` | The lone worker gets about twice the share it had in TC1; the three bad kills are refused with the right message |
| TC4 | Stress | 4 x `spawn` (8 workers) + `run spin` = 9 busy tasks; 10 x `run hello` under load; 20 files written and removed | Every busy task gets CPU; the shell still answers; 10 of 10 launches work; free frames after = before; heap returns to within 1 KiB |
| TC5 | Failure | 12 invalid or hostile requests | Each refused or contained with the expected message; kernel and shell answer; no frames leak; no ring-3 task left |
| TC6 | Repeatability | TC1 repeated in 3 fresh boots; a deterministic command in 2 | Fairness above 0.75 each time; timer rate within 10%; boot time within 25%; identical output |

## 4. Layer measurements (evidence run, from the scheduler trace)

| Scenario | Pass criteria |
|---|---|
| L4 queue | Empty system: queue empty and consistent. Four workers: queue lists 3 or 4 of them and is consistent |
| L5 round robin | The four workers' departures are at least 99% timer; at least 90% of their stretches last exactly one tick; their order repeats the same cycle in at least 90% of turns; mean scheduling latency within 1 tick of (workers - 1) |
| L6 cooperative | At least 90% of switches are `yield`; the switch rate is more than 3 times the timer-driven rate |
| L5 jobs | `job 600 300 150` finishes in the order 150, 300, 600; every job first runs within 3 ticks; turnaround ratios within 15% of the equal-share round-robin prediction (1.667 and 2.333) |
| L7 race | Every unprotected run loses updates; every locked run reaches the expected total with none lost |
| L8 allocator | Allocations of 1 KiB to 16 MiB succeed and are freed; an impossible request is refused with no change to the heap's used size or hole count; the kernel carries on |

## 5. What is measured (performance evidence)

| Metric (assignment section 18) | Source | Method |
|---|---|---|
| Task-switch count | `trace` summary | Switches in a 6 s window, by reason |
| Scheduling latency | Trace events | Ticks between a worker leaving the CPU and getting it back |
| Response time | `job` output | Ticks from creation to first run |
| Throughput | `job` output | Work units finished per second |
| Fairness | `ps` before and after | Smallest share over largest among runnable tasks |
| Allocation time | `alloc` output | TSC cycles converted to microseconds |
| Memory utilization | `mem` | Frames free and heap used before and after |
| Timer rate | `ps` and the host clock | Ticks in a window over host seconds (about 3% error) |
| Not measured | | Synchronization wait time (a spinlock never waits on one CPU); I/O latency (the disk driver is read-only) |

The pipeline the assignment describes is implemented end to end: the kernel writes the scheduler trace, the script saves it as a log file (`evidence/logs/trace-preemptive.txt`, `trace-cooperative.txt`), a Python parser (`ca6_layers.parse_trace`) reads it, the analysis computes the metrics, charts are drawn, and the pass criteria above are the conclusions.

## 6. Limits of the evidence

| Limit | Effect |
|---|---|
| All runs are in QEMU with software emulation | Absolute speeds are not real-hardware speeds; ratios, orders and counts are meaningful |
| The timer rate is measured from outside | About 3% error |
| 6 s windows | Tick-level properties are stable at this size |
| The emulator is very repeatable | Identical tick counts across repeats; real hardware will show more spread |
| No real-PC run yet | Real-hardware boot is listed as future work |

## 7. How to repeat everything

```
cd G:\Aeon\OS
cargo clippy --release -- -D warnings
cargo test --lib --target x86_64-pc-windows-gnu
bash tests/run.sh                                      # boot tests
wsl -d Ubuntu -- python3 tests/ca6_evidence.py         # CA-6 evidence
```

Outputs: `docs/ca6/evidence/results.csv`, `results.json`, `logs/*` (raw kernel output of every VM and the two trace logs), and the charts `*.png`.
