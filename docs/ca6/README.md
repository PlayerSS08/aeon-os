# CA-6 submission package: Aeon, a Rust micro-OS

Assignment: *CA6 Rust Micro-OS Engineering Project* (Operating System Concepts, 15 marks). Specialization: Security and Isolation (Track F). Build: Aeon 0.9.2.

## The submission package (assignment section 42)

| # | Required item | File |
|---|---|---|
| 1 | Rust source repository | `S:\Works\Projects\Aeon\OS` (git, branch `feat/create--user`) |
| 2 | Architecture document | [architecture.md](architecture.md) |
| 3 | Design document | [design.md](design.md) |
| 4 | Test plan | [test-plan.md](test-plan.md) |
| 5 | Test results | [test-results.md](test-results.md), [evidence/results.csv](evidence/results.csv), [evidence/results.json](evidence/results.json), raw logs in [evidence/logs](evidence/logs) |
| 6 | Performance data | [performance.md](performance.md) and the charts in [evidence](evidence) |
| 7 | Python analysis scripts | `tests/ca6_evidence.py` (runs the cases), `tests/ca6_layers.py` (trace parser, analysis, charts), `tests/boot_test.py` (boot suite) |
| 8 | Debugging log | [debugging-log.md](debugging-log.md) |
| 9 | Final technical report (20 sections) | [final-report.md](final-report.md) |
| 10 | Demonstration | [demo-script.md](demo-script.md) |
| 11 | Individual contribution statement | [contribution-statement.md](contribution-statement.md) (completed by SREEHARAN, 25BTRCL257) |
| 12 | AI usage disclosure | [ai-disclosure.md](ai-disclosure.md) (signed by SREEHARAN, 25BTRCL257) |
| extra | Viva preparation: the assignment's 20 questions answered | [viva-prep.md](viva-prep.md) |

## The ten layers (assignment section 9)

| Layer | Name | Status | Command to see it | Evidence |
|---|---|---|---|---|
| 1 | Boot | done | (boot) | boot log; M1 |
| 2 | Console and diagnostics | done | `log`, `info` | M2 |
| 3 | Task representation | done | `ps` | TC1, TC2 |
| 4 | Ready queue | done (added 2026-11-02; was an implicit array scan before) | `queue` | L4/L5, 7 unit tests, `sched` group |
| 5 | Round robin | done, with trace | `trace`, `job` | L4/L5: order 100%, slices 100%, jobs |
| 6 | Timer-driven scheduling | done, with the cooperative comparison (added 2026-11-02) | `spawn`, `spawn coop`, `trace` | L6: 24 times the switch rate |
| 7 | Synchronization | done, with the problem and the fix (added 2026-11-02) | `race`, `race lock`, `block`, `wake` | L7, M6 |
| 8 | Memory management | done, with failure path (added 2026-11-02) | `mem`, `alloc` | L8, TC4 |
| 9 | System interface (optional) | done | `run hello`, `run fault` | TC5 |
| 10 | Integrated micro-OS | done | all | TC4 |

## The ten milestones (section 13)

| # | Milestone | Where |
|---|---|---|
| 1 Boot | `M1` |
| 2 Diagnostics | `M2` |
| 3 Tasks | TC1, TC2 |
| 4 Scheduler | L4/L5 |
| 5 Timer | `M5`, L6 |
| 6 Synchronization | `M6`, L7 |
| 7 Memory | L8, TC4 |
| 8 Specialization | TC5 and the fault-injection boot tests |
| 9 Integration | TC4 |
| 10 Validation | this package |

## Reproduce everything

```
cd S:\Works\Projects\Aeon\OS
cargo test --lib --target x86_64-pc-windows-gnu
bash tests/run.sh
wsl -d Ubuntu -- python3 tests/ca6_evidence.py
```
