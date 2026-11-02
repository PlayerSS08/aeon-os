# AI usage disclosure (CA-6)

> **For the student to check and sign.** This was written from what the repository and the working notes show. Read it, correct anything that does not match what happened, and put your own name on it. The assignment requires you to be able to explain and defend every part of the code.

## Tools used

| Tool | Used for |
|---|---|
| Claude Code (Anthropic's command-line coding assistant; models Sonnet, Opus and Haiku) | Writing and changing Rust code and tests, writing documents, running builds and tests, reading the course notes and the research report |
| QEMU, OVMF, swtpm, `cargo`, `clippy`, Python | Running and checking the work (not AI) |

## What the AI did

| Area | AI contribution |
|---|---|
| Kernel code | Wrote and edited most of the Rust source under the student's direction, including the scheduler, allocators, paging, user mode, shell and the changes in the final week (folders, `edit`/`append`, the checked interrupts-off rule, the disk busy flag) |
| Tests | Wrote the unit tests, the QEMU boot tests and the CA-6 evidence script |
| Documents | Drafted this submission package from the code, the test results and the course notes |
| Review | Audited the code for `unsafe` problems and wrote a research report ranking improvements (`G:\Aeon\reports`) |

## What the student did

> Fill this in yourself. Suggested points, true for this project according to the notes: chose the project goal and the order of work; decided what to build next and what to leave out; ran the system in QEMU and on a real PC and reported what failed (no disk, one-line files); reviewed and approved or rejected changes; kept the rule that nothing is pushed or released without approval. Add anything else you did, and remove anything that is not true.

## How the AI output was checked

| Check | How |
|---|---|
| Does it build and pass lint | `cargo clippy -- -D warnings` (two gates) before every commit |
| Does the logic work | 225 host unit tests |
| Does the real kernel work | 322 QEMU boot checks, run after every substantial change |
| Are the numbers right | The scheduler and Banker's tests use worked examples from the course notes; one of those examples turned out to be wrong and was documented (debugging log, entry 2) |
| Is every `unsafe` justified | Each site has a `SAFETY:` comment; the invariants are listed in the design document |
| Are claims honest | Documents state what was not tested (real hardware) and what is not built (per-program memory, disk writes) |

## Statement

I understand that AI-generated low-level code must not be copied blindly. The code in this repository was read, tested and approved by me before it was submitted, and I can explain it.

Name: ______________________  Date: ______________  (the student signs here)
