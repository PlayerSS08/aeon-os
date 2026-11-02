# CA-6 submission package: Aeon, a Rust micro-OS

Assignment: *CA6 Rust Micro-OS Engineering Project* (Operating System Concepts, 15 marks). Specialization: Security.

| # | Required item | File |
|---|---|---|
| 1 | Rust source repository | `G:\Aeon\OS` (git, branch `aeon-boot-upgrade`, latest commit at the time of writing `42fd956`) |
| 2 | Architecture document | [architecture.md](architecture.md) |
| 3 | Design document | [design.md](design.md) |
| 4 | Test plan | [test-plan.md](test-plan.md) |
| 5 | Test results | [test-results.md](test-results.md), [evidence/results.csv](evidence/results.csv), [evidence/results.json](evidence/results.json), raw logs in [evidence/logs](evidence/logs) |
| 6 | Performance data | [performance.md](performance.md) and the four charts in [evidence](evidence) |
| 7 | Python analysis scripts | `tests/ca6_evidence.py` (evidence and charts), `tests/boot_test.py` (boot suite) |
| 8 | Debugging log | [debugging-log.md](debugging-log.md) |
| 9 | Final technical report | [final-report.md](final-report.md) |
| 10 | Demonstration | [demo-script.md](demo-script.md) |
| 11 | Individual contribution statement | [contribution-statement.md](contribution-statement.md) **(student must write)** |
| 12 | AI usage disclosure | [ai-disclosure.md](ai-disclosure.md) **(student must check and sign)** |
| extra | Viva preparation | [viva-prep.md](viva-prep.md) |

## Core requirements, at a glance

| Requirement | Met | Evidence |
|---|---|---|
| 1 Rust kernel boot | yes | M1 |
| 2 Kernel console | yes | M1, M2 |
| 3 Task representation | yes | TC1 `ps` |
| 4 Task/ready queue | yes | design.md section 2 |
| 5 Round robin | yes | TC1 to TC4, fairness 1.000 |
| 6 Timer-based scheduling | yes | M5, TC1 (measured tick rate) |
| 7 Synchronization | yes | M6, spinlock and wait queue |
| 8 Memory allocation | yes | TC4 |
| 9 Logging and debugging | yes | boot logs in `evidence/logs` |
| 10 Integrated demonstration | yes | TC4, `demo-script.md` |

## Reproduce everything

```
cd G:\Aeon\OS
cargo test --lib --target x86_64-pc-windows-gnu
bash tests/run.sh
wsl -d Ubuntu -- python3 tests/ca6_evidence.py
```
