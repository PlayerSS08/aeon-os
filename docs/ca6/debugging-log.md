# Aeon: debugging log (CA-6)

Each entry has the symptom, how the cause was found, the fix, and the evidence. Entries 1 to 6 are from the final week and were found by testing in QEMU and on a real PC. The table at the end lists earlier fixes straight from the git history, with the commit subject only, because the commit bodies are empty.

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
| Evidence | Full boot suite passes with the checks active (322 of 322); `docs/design/globals.md` |

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
