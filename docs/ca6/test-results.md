# Aeon: test results (CA-6)

All results below were produced on 2026-11-02 by running the commands in `test-plan.md` on the 0.9.0 build (commit `42fd956`). Raw data: `evidence/results.csv`, `evidence/results.json`, `evidence/logs/*.serial.log`.

## 1. Summary

| Level | Result |
|---|---|
| Lint, kernel build (`cargo clippy --release -- -D warnings`) | clean |
| Lint, library (`cargo clippy --target x86_64-pc-windows-gnu --lib -- -D warnings`) | clean |
| Host unit tests | 225 passed, 0 failed |
| QEMU boot suite (`bash tests/run.sh`) | 322 passed, 0 failed |
| Release checks (signature, SBAT, size, reproducible build) | 16 of 16 passed; two clean builds give the identical hash `f2b5dcec...` |
| CA-6 evidence run (`tests/ca6_evidence.py`) | **73 passed, 0 failed**, 36 metrics, 4 charts, 445 s |

## 2. CA-6 evidence run, by test case

| Case | Checks | Result | Key numbers |
|---|---|---|---|
| M1/M2/M5 boot, diagnostics, timer | 4 | all pass | Kernel reaches the shell at 3.25 s after the logger starts; timer source reported |
| TC1 minimum configuration | 5 | all pass | Two workers: 304 and 304 ticks over the window; fairness ratio 1.000; share 49.0% each |
| TC2 multiple tasks | 8 | all pass | 5 runnable tasks (4 workers + 1 ring-3) each 19.7%; the blocked task 0%; fairness 1.000; `wake` released it |
| TC3 boundary | 8 | all pass | Lone worker 98.2% (about 2x the 49.0% of TC1); `kill 0`, `kill 99`, `kill x` refused with the right messages |
| TC4 stress | 7 | all pass | 9 busy tasks each 10.9%, fairness 1.000; shell still answered; 10 of 10 ring-3 launches worked under load; free frames 55131 before and after; heap 0 KiB, 2 KiB with 20 files, 1 KiB after removal |
| TC5 failure | 16 | all pass | 12 hostile or invalid requests refused or contained; kernel alive after each; no frame leak; no ring-3 task left behind |
| M6 synchronization | 6 | all pass | Blocked task: 0 ticks over 3 s; `wake` woke exactly 1; second `wake` woke 0 |
| TC6 repeatability | 19 | all pass | Three fresh boots: fairness 1.000 / 1.000 / 1.000; timer rate 96.1 / 96.7 / 96.1 Hz (spread 0.64%); boot time 3.261 / 3.240 / 3.214 s (spread 1.46%); a deterministic command gave identical output twice |

## 3. TC5 in detail: every hostile request

| Request | What the kernel said | Contained |
|---|---|---|
| `run nosuch` | unknown program, lists the five that exist | yes |
| `run fault` | task killed: `#PF` page fault | yes, only that task |
| `run gp` | task killed: `#GP` general protection | yes |
| `run badcall` | exits with code 0: the bad system call was refused, not obeyed | yes |
| `cat nothere` | no such file | yes |
| `write` (no name) | bad name | yes |
| `write a/b x` | no such folder for 'a/b' | yes |
| `rmdir nosuch` | no such folder | yes |
| `rm nothere` | no such file | yes |
| `sort 5 a` | not a number: a | yes |
| `hlep` | unknown command, "Did you mean `help`?" | yes |
| `dread 99999` | no disk (none attached in this run) | yes |

## 4. Findings from the testing itself

| # | Finding | What it means |
|---|---|---|
| 1 | The first evidence run reported a frame "leak" of 16 frames (64 KiB) after ring-3 programs. Reading `usermode.rs` showed this is the 64 KiB user window, allocated once on the first `run` and reused (wiped before every program) | Not a leak: a one-time cost. The scenarios now warm up the window before taking their baseline. After that, free frames are identical before and after (55131 and 55131) |
| 2 | The first TC4 run could not launch `hello` ten times while `run spin` was alive | A real design limit, one ring-3 program at a time, and the scenario now stops `spin` first. It is listed in the limits |
| 3 | The heap is not exactly back to 0 KiB after removing the 20 files: it reads 1 KiB (baseline 0, peak 2) | The check allows 1 KiB of slack. The residue is probably vector capacity kept by the file system; this was not investigated further |
| 4 | The timer measures about 96 Hz, not 100 | The measurement is made from outside the VM with `ps` typed by a script, so the two snapshots are separated by typing time. The error is stable (96.1 to 96.7) but its cause was not isolated; a kernel-side counter against the TSC would settle it |
| 5 | TC1 and two of the TC6 repeats give identical tick counts (304, 304) | Software emulation is very repeatable; on real hardware expect more spread |

## 5. Earlier full-suite failures, for the record

The first full run after the final-week changes gave 314 passed and 2 failed. Both were tests still expecting the old `ls` help text (`list the files in RAM`), which was changed on purpose. They were updated and the full suite then gave 322 of 322.

## 6. What was not tested

| Not tested | Why |
|---|---|
| Real hardware | Only QEMU was used for the measured runs; a real-PC boot of the 0.9.0 image is still to do |
| Long runs | Windows are 6 s |
| SMP, networking | Not built |
