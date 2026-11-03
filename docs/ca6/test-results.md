# Aeon: test results (CA-6)

All results below were produced on 2026-11-03 by running the commands in `test-plan.md` on the 0.9.2 build. Raw data: `evidence/results.csv`, `evidence/results.json`, `evidence/logs/*.serial.log`. The measured runs used QEMU with software emulation; no real PC was used.

## 1. Summary

| Level | Result |
|---|---|
| Lint, kernel build (`cargo clippy --release -- -D warnings`) | clean |
| Lint, library (`cargo clippy --target x86_64-pc-windows-gnu --lib -- -D warnings`) | clean |
| Host unit tests | 375 passed, 0 failed |
| QEMU boot suite, groups `editor` (25), `workflow` (61), `sweep` (72), `power` (8), `user` (17) | all passed, 0 failed (the full suite was last run on 0.9.1: 359 passed) |
| CA-6 evidence run (`tests/ca6_evidence.py`) | **111 passed, 0 failed**, 60 metrics, 9 charts, 489 s (a first run of the same build had one timer-rate check fail at 81 Hz in one of three repeats: host jitter under software emulation, the repeat passed) |

## 2. CA-6 evidence run, by test case

| Case | Checks | Result | Key numbers |
|---|---|---|---|
| M1/M2/M5 boot, diagnostics, timer | 4 | all pass | Kernel reaches the shell at 2.37 s of kernel time; timer source `lapic` |
| TC1 minimum configuration | 5 | all pass | Two workers: 306 and 306 ticks; fairness 1.000; share 49.0% each; timer 96.7 Hz |
| TC2 multiple tasks | 8 | all pass | 5 runnable tasks (4 workers + 1 ring-3), shares 19.6% to 19.8%; the blocked task 0 ticks; fairness 0.992; `wake` released it |
| TC3 boundary | 8 | all pass | Lone worker 98.2% (about 2x the 49.0% of TC1); `kill 0`, `kill 99`, `kill x` refused with the right messages |
| TC4 stress | 7 | all pass | 9 busy tasks each 10.9%, fairness 0.986; shell still answered; 10 of 10 ring-3 launches worked under load; free frames 55086 before and after; heap 0 KiB, 2 KiB with 20 files, 1 KiB after removal |
| TC5 failure | 16 | all pass | 12 hostile or invalid requests refused or contained; kernel alive after each; no frame leak; no ring-3 task left behind |
| M6 synchronization | 6 | all pass | Blocked task: 0 ticks over 3 s; `wake` woke exactly 1; a second `wake` woke 0 |
| TC6 repeatability | 19 | all pass | Three fresh boots: ticks 306/306, 306/306 and 303/303, fairness 1.000 each; timer 96.7 / 96.7 / 95.8 Hz (spread 0.96%); boot 2.428 / 2.297 / 2.403 s (spread 5.5%); a deterministic command gave identical output twice |
| L4/L5 ready queue, round robin, trace | 9 | all pass | 1094 switches in 6 s (182 per second); workers left the CPU by timer 100% of 830 departures; 100% of stretches one tick; order repeats 100%; latency mean 3.07, max 4 ticks |
| L6 cooperative against timer | 4 | all pass | 26,586 switches (4431 per second), 95.9% `yield`; 24.3 times the timer-driven rate |
| L5 jobs that finish | 8 | all pass | Jobs 150, 300, 600 units finished in that order; turnaround 127, 212, 297 ticks; response 2, 1, 0 ticks; ratios 1.669 and 2.339 against 1.667 and 2.333 predicted; 353 units per second |
| L7 race and lock | 7 | all pass | Without a lock 20,000 and 19,985 of 40,000 additions lost; with the lock 0 lost, twice; with the lock 0 lost, twice |
| L8 allocation | 10 | all pass | 1, 64, 1024, 8192, 16384 KiB took 42, 6, 4, 3, 31 microseconds; a 99,999,999 KiB request refused and nothing changed; heap 0 of 32,768 KiB at rest |

## 3. TC5 in detail: every hostile request

| Request | What the kernel said | Contained |
|---|---|---|
| `run nosuch` | unknown program, lists the six that exist | yes |
| `run fault` | task killed: `#PF` page fault | yes, only that task |
| `run gp` | task killed: `#GP` general protection | yes |
| `run badcall` | exits with code 0: the bad system call was refused, not obeyed | yes |
| `run denied` | exits with code 0: a call outside the program's allow-list (`uptime`) was refused (added in 0.9.2, checked in the `user` group) | yes |
| `cat nothere` | no such file | yes |
| `write` (no name) | prints its usage (`usage: write [-f] <f> <t>`) | yes |
| `write a/b x` | no such folder for 'a/b' | yes |
| `rmdir nosuch` | no such folder | yes |
| `rm nothere` | no such file | yes |
| `sort 5 a` | not a number: a | yes |
| `hlep` | unknown command, "Did you mean `help`?" | yes |
| `dread 99999999` | read failed: BadRequest (the VM now has a SATA disk, so the sector must be out of range to fail) | yes |

## 4. Findings from the testing itself

| # | Finding | What it means |
|---|---|---|
| 1 | The first evidence run reported a frame "leak" of 16 frames (64 KiB) after ring-3 programs. Reading `usermode.rs` showed this is the 64 KiB user window, allocated once on the first `run` and reused | Not a leak: a one-time cost. The scenarios now warm up the window before taking their baseline. After that, free frames are identical before and after (55124 and 55124) |
| 2 | The first TC4 run could not launch `hello` ten times while `run spin` was alive | A real design limit: one ring-3 program at a time. The scenario now stops `spin` first. It is listed in the limits |
| 3 | The heap is not exactly back to 0 KiB after removing the 20 files: it reads 1 KiB (baseline 0, peak 2) | The check allows 1 KiB of slack. The residue is probably vector capacity kept by the file system; not investigated further |
| 4 | The timer measures about 96 Hz, not 100 | The measurement is made from outside the VM with `ps` typed by a script, so the two snapshots are separated by typing time. The value is stable (96.2 to 96.7) but its cause was not isolated; a kernel-side counter against the TSC would settle it |
| 5 | TC1 and the three TC6 repeats give nearly identical tick counts (303 to 306 each) | Software emulation is very repeatable; on real hardware expect more spread |
| 6 | The first analysis of the trace counted the shell as a worker and gave 82% timer switches | A mistake in the measurement, not in the kernel (debugging log entry 9) |
| 7 | The jobs measured first never overlapped, because typing three commands took longer than a job | `job` now starts several sizes in one command (debugging log entry 10) |
| 8 | The measured turnaround ratios are 2.1% and 1.2% above the equal-share prediction | Expected: the prediction ignores the `shell` and `status` visits and the one-tick rounding. The check allows 15% |
| 9 | The first allocation (1 KiB) took 42 microseconds (419 in the 0.9.1 run), the later ones 3 to 31 | Cause not investigated (probably the first touch of heap memory) |

## 5. Earlier failures, for the record

| Run | Result | Cause | Action |
|---|---|---|---|
| Full boot suite after the final-week changes | 314 passed, 2 failed | Two tests still expected the old `ls` help text, changed on purpose | Tests updated |
| One `sched` group run | 1 failed ("with the lock, no update is lost"), next run passed | Probably a timing race in the test (it read the line before it was complete); the failing line was not captured, so this is a hypothesis | Test now waits for the complete line; 3 clean reruns, then the full suite |
| Evidence run 8 | 2 checks failed | One VM took over 180 s to reach its prompt (firmware or host delay; the kernel log was normal) | The script restarts a slow VM once and prints a note |

## 6. What was not tested

| Not tested | Why |
|---|---|
| Real hardware | Only QEMU was used for the measured runs; the owner booted the 0.9.x image on a real PC once and it worked (no measurements taken) |
| Synchronization wait time, I/O latency | A spinlock never waits on one CPU; the disk driver is read-only and was not timed |
| Long runs | Measurement windows are 6 s |
| SMP, networking | Not built |

## 7. Re-running

```
cd G:\Aeon\OS
cargo clippy --release -- -D warnings
cargo clippy --target x86_64-pc-windows-gnu --lib -- -D warnings
cargo test --lib --target x86_64-pc-windows-gnu
bash tests/run.sh
wsl -d Ubuntu -- python3 tests/ca6_evidence.py
```
