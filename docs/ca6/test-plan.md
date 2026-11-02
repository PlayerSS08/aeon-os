# Aeon: test plan (CA-6)

## 1. Three levels of testing

| Level | What it tests | How to run | Size |
|---|---|---|---|
| Unit tests (host) | Pure logic: allocators, wait list, file system, FAT, parsers, policies, Banker's algorithm | `cargo test --lib --target x86_64-pc-windows-gnu` (from `G:\Aeon\OS`) | 225 tests |
| Boot tests (QEMU) | The real kernel in a virtual PC: boot stages, shell, tasks, faults, Secure Boot, TPM, recovery | `bash tests/run.sh` (about 10 minutes) | 322 checks |
| CA-6 evidence run | The six required test cases with measurements, logs and charts | `python3 tests/ca6_evidence.py` inside WSL (about 10 minutes) | See `docs/ca6/evidence/results.csv` |

Lint gates that must stay clean: `cargo clippy --release -- -D warnings` and `cargo clippy --target x86_64-pc-windows-gnu --lib -- -D warnings`.

## 2. Milestones and where each is tested

| # | Milestone | Test that proves it |
|---|---|---|
| 1 | Boot | Evidence run `M1`; boot-test groups `boot`, `modes`, `menu` |
| 2 | Diagnostics | Evidence `M2` (boot log stages, `version`, `info`, `cpu`, `security`); boot-test group `commands` |
| 3 | Tasks | TC1, TC2 (`spawn`, `ps`) |
| 4 | Scheduler (round robin) | TC1 fairness ratio, TC2, TC4 |
| 5 | Timer | Evidence `M5` (timer source) and the measured tick rate in TC1 and TC6; boot-test group `apic` for the LAPIC and PIT paths |
| 6 | Synchronization | Evidence `M6` (blocked task uses no CPU, wake releases it); host tests of `WaitList` and `SpinLock` |
| 7 | Memory | TC4 (no frame leak, heap returns to baseline); host tests of the frame bitmap and heap |
| 8 | Specialization (Security) | TC5 plus boot-test groups `faults`, `secureboot`, `bootcount`, `user` |
| 9 | Integration | TC4: all of the above at once |
| 10 | Validation | This plan, `results.csv`, `test-results.md` |

## 3. The six required test cases

| TC | Name | Setup | Pass criteria |
|---|---|---|---|
| TC1 | Minimum configuration | Boot, `spawn` (two busy workers that never yield), 6 s window | Both workers get CPU; both counters advance; the two workers' CPU ticks are within 25% of each other |
| TC2 | Multiple tasks | Two `spawn`, one `run spin` (ring 3), one `block` (blocked waiter); 6 s window | 4 workers and the ring-3 task all get CPU; the blocked task gets none; runnable tasks within 40% of each other; `wake` releases the waiter |
| TC3 | Boundary | Spawn two, kill one: a lone worker. Then `kill 0`, `kill 99`, `kill x` | The lone worker gets about twice the share it had in TC1; the three bad kills are refused with the right message |
| TC4 | Stress | 4 x `spawn` (8 workers) + `run spin` = 9 busy tasks; 10 x `run hello` under load; 20 files written and removed | Every busy task gets CPU; shell still answers; 10 of 10 ring-3 launches work; free frames after = free frames before; heap returns to baseline |
| TC5 | Failure | 12 invalid or hostile requests (unknown program, page fault, general-protection fault, bad syscall, missing file, bad name, missing folder, bad number, mistyped command, read with no disk) | Each is refused or contained with the expected message; the kernel and shell still answer; no frames leak; no ring-3 task is left behind |
| TC6 | Repeatability | TC1 repeated in 3 fresh boots; a deterministic command in 2 fresh boots | Fairness ratio above 0.75 every time; measured timer rate within 10% across repeats; boot time within 25%; identical output for the deterministic command |

## 4. What is measured (performance evidence)

| Metric | Source | Method |
|---|---|---|
| CPU share per task | `ps` before and after a window | Difference in per-task ticks divided by the total ticks of all tasks in the window |
| Fairness ratio | Same | Smallest runnable task's ticks divided by the largest's (1.0 = perfectly equal) |
| Timer rate | Same plus the host clock | Total ticks in the window divided by seconds between the two `ps` commands. The host clock has about 3% error because the commands are typed by a script |
| Kernel boot time | Boot log | Timestamp of the line "task shell started" (microseconds since the kernel logger started) |
| Frames free, heap used | `mem` | Before and after each scenario |
| Image size | File size | 250,408 bytes signed, 248,832 bytes unsigned |

## 5. Limits of the evidence

| Limit | Effect |
|---|---|
| All runs are in QEMU with software emulation | Absolute speeds are not real-hardware speeds; ratios and counts are meaningful |
| The timer rate is measured from outside | About 3% error |
| Each window is 6 s | Short windows show a few hundred ticks per task; the fairness ratio is stable at this size, but a longer window would tighten it |
| No real-PC run yet | The boot on real hardware is listed as future work |

## 6. How to repeat everything

```
cd G:\Aeon\OS
cargo clippy --release -- -D warnings
cargo test --lib --target x86_64-pc-windows-gnu
bash tests/run.sh                                       # boot tests, about 10 minutes
wsl -d Ubuntu -- python3 tests/ca6_evidence.py          # CA-6 evidence, about 10 minutes
```

Outputs: `docs/ca6/evidence/results.csv`, `results.json`, `logs/*.serial.log` (the raw kernel output of every VM), and the charts `*.png`.
