# Aeon: debugging log (CA-6)

Each entry has the symptom, how the cause was found, the fix, and the evidence. Entries 1 to 13 are from the final days. Entry 1 was found on a real PC; the others by tests, by reading the code, and by re-reading the assignment (entries 7 and 8 were gaps in my own first submission package). The table at the end lists earlier fixes straight from the git history, with the commit subject only, because the commit bodies are empty.

## Entries

### 1. Files could hold only one line (found on a real PC)

| Item | Detail |
|---|---|
| Symptom | `write notes text` stored text, but there was no way to make a file with more than one line |
| Investigation | Read `cmd_write` (`shell.rs`): it saves everything after the name. Read the input path: the line editor is limited to the screen width (`ed.set_max(cols - PROMPT.len() - 2)`, at most 120 characters) and Enter runs the command at once, so no newline can be typed |
| Root cause | A one-row input line plus "Enter runs the command" leaves no way to enter a newline |
| Fix | `edit <file>` (many lines, a lone `.` saves, Ctrl+C cancels), `append`, and `\n` `\t` `\\` escapes in `write`. Tests first: `unescape_makes_newlines_and_tabs`, `append_adds_to_the_end_and_creates` |
| Evidence | Boot test group `fs`: 37 checks, including `the file keeps its lines apart` |

### 2. The priority-scheduling example in the course notes is wrong

| Item | Detail |
|---|---|
| Symptom | The scheduler simulator failed its own test for preemptive priority: expected waits (5, 0, 16, 10) did not match |
| Investigation | Checked the notes' own cross-check, turnaround = waiting + burst. The notes show P1 running 0 to 1 and 5 to 13, which is 9 ticks for an 8-tick job, and P3 finishing at 27 when all four jobs add up to 26 ticks with no idle time |
| Root cause | An arithmetic slip in the notes, not a bug in the simulator. The simulator's answer (P1 done at 12, waits 4, 0, 15, 9, average 7.0) satisfies the cross-check |
| Fix | The test was changed to the correct numbers, with a comment explaining the difference |
| Evidence | `schedpolicy::tests::preemptive_priority_correct_run`; the other four policies match the notes exactly (FCFS 8.75, SJF 7.75, SRTF 6.5, RR 11.75) |

### 3. Help lines wrapped in the middle of a word at 640x480

| Item | Detail |
|---|---|
| Symptom | New test `t_text_clip` showed the line `run a built-in ... spin ba` followed by `dcall` on the next row |
| Investigation | A screenshot of the 640 px screen. The console is 71 columns there (9 px cells), but the help test assumed 80 |
| Root cause | The width test was too optimistic, and six help lines were 72 to 77 columns |
| Fix | The test now enforces 71 columns; the six texts were shortened. The status bar, which was cut off at the right, got a short form below 84 columns |
| Evidence | `cmdhelp::tests::lines_fit_a_71_column_screen`; the screenshot `target/test-shots/9-text-clip.png` |

### 4. A comment promised an interrupts-off rule that nothing checked

| Item | Detail |
|---|---|
| Symptom | None yet; found by reading. `global.rs` said no interrupt handler touches shared data, but the timer interrupt runs the scheduler |
| Investigation | Listed every `Global<...>` static and asked which ones an interrupt handler can reach |
| Root cause | The rule existed only as a comment, so a future change could break it silently |
| Fix | `Global::get_irqs_off()` stops the kernel if interrupts are on. The scheduler, wait lists, logger and memory manager use it. `log::reset_after_fault` now switches interrupts off itself |
| Evidence | Full boot suite passes with the checks active (322 of 322 at the time; 359 of 359 now); `docs/design/globals.md` |

### 5. A slow disk read stopped the timer

| Item | Detail |
|---|---|
| Symptom | None in normal use; found by reading `vblk.rs`. A disk request polls for up to 2 seconds while holding a `SpinLock`, which switches interrupts off |
| Root cause | The wrong kind of lock for slow work: the timer, the scheduler and the keyboard polling all stop for as long as a request lasts |
| Fix | A busy flag instead of an interrupts-off lock; a second task sleeps 10 ms and retries |
| Evidence | Boot-test groups `disk` and `disk32` still pass |

### 6. My own test script crashed on its first real run

| Item | Detail |
|---|---|
| Symptom | `ValueError: invalid literal for int() with base 10: 'counters'` in the evidence script |
| Root cause | The text captured after a command included the echo of the command itself |
| Fix | `Session.run` now drops the echoed first line and the trailing prompt; each scenario is wrapped so one failure is recorded and the run goes on |

### 7. There was no ready queue (found by re-reading the assignment)

| Item | Detail |
|---|---|
| Symptom | None in running code. My first CA-6 package claimed layer 4 ("ready queue") was met because the scheduler scans the task array for the next ready task |
| Investigation | Re-read the assignment's layer 4: "implement the structure so students understand insertion, selection, removal, and ordering". A scan has none of those four as visible operations |
| Root cause | The mechanism existed implicitly, not as a structure the assignment can see, and I had marked it done |
| Fix | A pure `ReadyQueue<16>` (FIFO ring of ids), written test-first (7 tests, one against a reference queue with 5000 random operations), then wired into the scheduler: tasks are inserted when created, woken or preempted, selected from the front, removed on `kill`. A `queue` command prints the order and checks the invariant "queue = tasks in state Ready" |
| Evidence | Boot-suite group `sched`; the queue is checked as consistent after every scenario; the full suite still passes with the scheduler rewritten |

### 8. No scheduler trace, no cooperative demonstration, no race demonstration

Same cause as entry 7: the assignment asks for evidence (task order, time-slice behaviour, a trace; cooperative against timer switching; shared-resource problem, synchronization, corrected execution) that the first package did not produce. Fix: `TraceRing` (every switch with its reason), `spawn coop`, `race` and `race lock`, `job`, `alloc`. Evidence: the L4 to L8 section of the evidence run.

### 9. My first trace analysis counted the shell as a worker

| Item | Detail |
|---|---|
| Symptom | The first run of the layer checks failed: "four workers" found five, 82% (not 95%) of switches were timer switches, round-robin order matched only 67% |
| Investigation | Printed the raw trace. The workers 4, 3, 5, 6 cycled in a perfect order; task 2 (`shell`) appeared every second tick as a sub-tick visit, and its sleeps are not timer switches |
| Root cause | The measurement, not the kernel: I picked "workers" as tasks with many turns, which included the shell, and measured the timer share over all switches |
| Fix | Identify workers from `ps`, and measure why the WORKERS left the CPU (100% timer), their stretch length (100% exactly one tick) and their cycle (100% repeating) |
| Lesson | The trace was right and the analysis wrong; checking the raw data first saved a wrong conclusion about the scheduler |

### 10. The "jobs" never overlapped, and a help text claim was false

| Item | Detail |
|---|---|
| Symptom | Jobs of 160, 80 and 40 units finished in the order 160, 80, 40, the opposite of round robin's prediction |
| Investigation | The serial log showed each job starting and finishing before the next command was echoed: a unit takes about 2.9 ms, so 160 units take 0.46 s, while typing a command takes about 1.5 s |
| Root cause | My test started the jobs with three typed commands, slower than the jobs. Separately, the help text said "100 units take about one second", which I had written without measuring (it is about 0.29 s: one unit takes about 2.9 ms) |
| Fix | `job` now takes several sizes in one command and starts them in the same instant; the false sentence was removed. The check became quantitative: turnaround ratios 300:150 and 600:150 against the equal-share round-robin prediction of 1.667 and 2.333 |
| Evidence | Measured 1.701 and 2.362 in the final run, within 2.5% of the prediction |

### 11. An intermittent failure in a correctness test

| Item | Detail |
|---|---|
| Symptom | One run of the `sched` group failed "with the lock, no update is lost"; the next run passed |
| Investigation | The failing line was not captured. The kernel prints the result in pieces (`out!` writes fragments); the test waited for the first piece (`race with lock: expected 40000`) and then read the line, which can still be incomplete |
| Probable cause | A timing race in the test, not in the kernel. This is a hypothesis: the first failure's output was lost |
| Fix | The test now waits for the complete line with a regular expression (`wait_regex`) |
| Evidence | Three further runs of the group: 35 of 35 each time; later runs of the extended group: 37 of 37. The locked race gave 40000 of 40000 in every run (also in the evidence run, twice) |

### 12. One VM took more than 180 s to start

An evidence run failed two checks because one VM did not reach its prompt in time. Its log showed a normal kernel boot (shell at 3.2 s of kernel time), and the whole run took 923 s against 463 s on the next attempt, so the delay was in the firmware or host before the kernel started. The evidence script now restarts a slow VM once and prints a note. It was not needed on the final run.

### 13. The kernel grew by 9 KB

The new commands and the trace raised the unsigned image from 248,832 to 259,072 bytes, to within 1.5 KB of the 256 KiB test limit (once signed). The 16 KiB trace ring was put in its own all-zero static so it lives in `.bss` and not in the image; without that the limit would already have been exceeded. The size budget is now the next constraint to deal with (research report, decision 9).

## Earlier fixes, from the git history (subjects only)

| Commit | Subject |
|---|---|
| `5fce208` | fix(security): run NMI and machine check on a dedicated IST stack |
| `a0c3f5a` | fix(kernel): unsafe audit fixes (asm aliasing, sysret register leak, fatal #DF, run gating, WC page split, read-only boot info) |
| `a0e59ae` | fix(kernel): scan 2-5 fixes (SMAP gate, NT mask, output sanitising, stack guard and scrub, panic IF, sleep, option parsing, README claims) |
| `493313a` | fix(security): keep the stored seed boot-services-only, do not count it as proof the canary is random |

## Lessons

1. A test that fails is information. Entries 2 and 3 were both caught because a new test disagreed with what was assumed, and in entry 2 the assumption was in the course notes, not in the code.
2. A rule that only lives in a comment will be broken. Make the machine check it (entry 4).
3. Check the cheap invariant first: turnaround = waiting + burst, free frames before = free frames after.
