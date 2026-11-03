# CA-6 checklist: what is done, where the proof is

`[x]` = done and proved by a test or a measured number. `[ ]` = only you can do it. Numbers are from the final evidence run (111 checks, 0 failed) and the boot suite.

## A. The ten layers

- [x] **1 Boot**: kernel starts, prints, stays up. Proof: boot test group `boot`; M1, kernel at the shell in 3.27 s
- [x] **2 Console and diagnostics**: information, warning/error, structured messages. Proof: `log`, `info`; M2
- [x] **3 Task representation**: id, state, context, stack, ticks. Proof: `ps`; TC1, TC2
- [x] **4 Ready queue**: insert, select, remove, order. Proof: `src/readyq.rs` (7 tests, 5000 random operations), `queue` command
- [x] **5 Round robin**: order, time slice, repeats, completion, trace. Proof: `trace` (100% one-tick slices, 100% same order), `job` (turnaround 127/216/300 ticks)
- [x] **6 Timer-driven scheduling**: timer connected to the scheduler; cooperative against timer. Proof: `spawn coop` gives 4401 switches/s against 183/s (24 times)
- [x] **7 Synchronization**: problem, fix, corrected run. Proof: `race` loses 20,000 of 40,000; `race lock` loses 0
- [x] **8 Memory management**: allocator, statistics, failure path. Proof: `mem`, `alloc` (6 to 10 microseconds; refusal changes nothing)
- [x] **9 System interface (optional)**: ring 3 through `syscall`. Proof: `run hello`, `run fault`, `run gp`, TC5
- [x] **10 Integrated micro-OS**: everything together. Proof: TC4 (9 busy tasks, 10 of 10 ring-3 launches under load)

## B. The ten milestones

- [x] M1 Boot  - [x] M2 Diagnostics  - [x] M3 Tasks  - [x] M4 Scheduler  - [x] M5 Timer
- [x] M6 Synchronization  - [x] M7 Memory  - [x] M8 Specialization (Track F, Security and Isolation)
- [x] M9 Integration  - [x] M10 Validation (this package)

## C. The six test cases

- [x] TC1 minimum configuration (two workers, fairness 1.000)
- [x] TC2 multiple tasks (5 runnable, fairness 0.992, blocked task uses 0 CPU)
- [x] TC3 boundary (lone worker 98.2%, bad `kill` arguments refused)
- [x] TC4 stress (9 busy tasks, no frame leak: 55124 before and after)
- [x] TC5 failure (12 hostile requests contained)
- [x] TC6 repeatability (3 fresh boots, timer-rate spread 0.0%, boot-time spread 1.08%)

## D. Metrics

- [x] Context-switch count and rate  - [x] Scheduling latency (mean 3.07, max 4 ticks)  - [x] Response time (2, 1, 0 ticks)
- [x] Turnaround  - [x] Throughput (350 units/s)  - [x] Fairness  - [x] Allocation time  - [x] Memory utilisation
- [ ] Synchronization wait time: not measured (a spinlock never waits on one CPU; say so in the viva)
- [ ] I/O latency: not measured (the disk driver is read-only)

## E. The submission package (assignment section 42)

- [x] 1 Source repository (`G:\Aeon\OS`)  - [x] 2 Architecture  - [x] 3 Design  - [x] 4 Test plan  - [x] 5 Test results
- [x] 6 Performance data and charts  - [x] 7 Python analysis scripts  - [x] 8 Debugging log
- [x] 9 Final technical report (20 sections)  - [x] 10 Demonstration script
- [-] 11 Individual contribution statement: not needed for now (owner decision 2026-11-03)
- [-] 12 AI usage disclosure signature: not needed for now (owner decision 2026-11-03)

## F. Viva

- [x] 18 of 20 questions answered in `viva-prep.md`
- [-] Q17 and Q18 of the viva: not needed for now (owner decision 2026-11-03)
- [ ] Read the answers aloud once; change any sentence you would not say yourself
- [ ] Check the reference list in the final report

## G. Screen and boot upgrades (2026-11-02)

- [x] Aeon picks the largest screen mode by itself (`src/gopmode.rs`, 8 tests); `mode=WxH` still overrides
- [x] A typed line can use several rows and Backspace follows it back (2 editor tests + boot test `longline`)
- [x] Dawn, the boot menu, opens first with "Boot Aeon" highlighted and counts down; any key stops the count
- [ ] Try all three on your real PC (not tested on real hardware yet) and tell me what looks wrong
- [ ] Run `pci` on the real PC (decides the AHCI or NVMe disk driver)

## H. Things I did not do

- [ ] Pendrive `D:` was not plugged in: the 0.9.x files are not copied yet
- [ ] Real hardware boot
- [ ] Push to GitHub (never done without your word)

## I. After 0.9.1: what changed and what is re-proved (2026-11-03)

The numbers in sections A to D come from the 0.9.1 evidence run. Build 0.9.2 adds work that is **not** part of that run and is not needed for the assignment's core requirements:

- [x] Dawn boot menu: 15 s countdown, arrow keys and Enter only, the countdown never freezes, unsafe entries fall back to booting Aeon (12 unit tests, QEMU group `dawn` 19 of 19)
- [x] Apple-like menu and stop screen, colours checked for contrast (`src/palette.rs`)
- [x] Full-screen `edit`, folder prompt, 1024-character lines, `write` refuses to replace without `-f`, 14 new commands (373 unit tests in all)
- [x] xHCI USB keyboard, AHCI SATA disk (read only), ELF parser (not used by the kernel yet): QEMU only
- [ ] Re-run the evidence run (`tests/ca6_evidence.py`) on 0.9.2 so the numbers in `test-results.md` and `performance.md` match the shipped build (needs free memory; not started)
- [ ] Re-run the whole QEMU suite after the editor work; the groups `editor`, `workflow`, `sweep` and `power` are written but not yet run
- [ ] Change "0.9.1" to "0.9.2" in the package once the evidence is refreshed
