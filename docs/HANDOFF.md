# Aeon handoff: read this first in a new session

Last updated 2026-11-02. Branch `aeon-boot-upgrade` in `G:\Aeon\OS`. Nothing is pushed (no upstream, no PR).

## What Aeon is

A pure-Rust, `no_std`, zero-dependency UEFI x86_64 micro-OS. Kernel is called Nexus, shell Pulse, boot menu Dawn (names not yet approved by the owner). `G:\Aeon\extras` holds the roadmap and design pages (not in git).

## How to build, test, run

| Task | Command (from `G:\Aeon\OS`, Git Bash) |
|---|---|
| Host unit tests (225) | `cargo test --lib --target x86_64-pc-windows-gnu` (MSVC link.exe is not installed, so use the GNU target) |
| Clippy | `cargo clippy --release -- -D warnings` and `cargo clippy --target x86_64-pc-windows-gnu --lib -- -D warnings` |
| Release build | `cargo build --release` (output `target\x86_64-unknown-uefi\release\aeon.efi`) |
| Full test run (about 10 min, lint + host tests + QEMU boot tests in WSL Ubuntu) | `bash tests/run.sh` (run it detached and wait; it exceeds a 10 min tool limit). One test: `bash tests/run.sh -k console` |
| Latest result | 322 passed, 0 failed (full suite, 2026-11-02, after the second session) |
| See Aeon in a QEMU window | `powershell -ExecutionPolicy Bypass -File tools\run-qemu.ps1 -Build -Disk` |
| Signed release for the pendrive | `bash tools/release.sh` (builds with reproducible flags, signs in WSL with the key in `~/.aeon-keys`, rewrites `usb/`) |
| Tester's checklist | `G:\AEON-TEST-GUIDE.md` |

Boot-test screenshots land in `target\test-shots\` (the console test saves `console-a.png`, `console-up.png`).

## Git

- Commit with `git -c user.name="Scrooge-777" -c user.email="318593710+Scrooge-777@users.noreply.github.com" commit ...` (the shell has no git identity; do not change git config).
- Do not push unless the owner asks.
- Latest commits: `274fab4` signed `usb/` refresh, `80919c6` Up/Down scroll, `6247a6a` line editing, `499c228` scrollback and grouped help.

## State of the pendrive

`usb/` in the repo holds the signed 0.9.0 image (commit `5984d8b`, 250,408 bytes, SHA-256 `d56d5f68...6d97`; reproducible build checked). The signing certificate is unchanged, so nothing needs re-enrolling. The pendrive `D:` (label AEON, FAT32) still holds the OLD 0.8.1 files from commit `274fab4` until someone copies `usb\EFI\BOOT\BOOTX64.EFI`, `aeon-secureboot.cer` and `SHA256.txt` onto it. The copy at `S:\Works\Projects\Aeon` is old (from `ad5d657`).

## What is built (all tested in QEMU, none on real hardware yet)

- Boot: signed image, Secure Boot, TPM measured boot (PCR 11 and 12), event-log replay, SBAT, boot counting and recovery mode, Dawn menu, splash.
- Kernel: W^X paging to 1 TiB, SMEP/SMAP/UMIP, GDT/TSS/IDT with IST stacks, preemptive round-robin scheduler at 100 Hz, wait queues, SpinLock (used on 2 statics), ring 3 via syscall/sysret (5 built-in programs), LAPIC timer (PIT fallback), ACPI MADT, PCI scan, virtio-blk, read-only FAT16/FAT32.
- Console and shell: smooth fonts (Inter, JetBrains Mono), 512-line scrollback, Up/Down/PgUp/PgDn scroll, Ctrl+Home/End, block cursor, line editing (Left/Right/Home/End/Delete, Ctrl+A/E/B/F/U/K/W/D), Tab completion, Ctrl+Up/Down or Ctrl+P/N command history (last 8), did-you-mean, aliases (cls, dir, ver, man, free, uname, history), grouped `help`, `help <cmd>`, `help all`. The command table is `src/cmdhelp.rs`; a unit test checks it matches what `src/shell.rs` dispatches.
- RAM file system with folders (`mkdir cd pwd tree rmdir`, paths up to 128 characters, `.` and `..`), many-line files (`edit <file>` ends on a lone `.`, `append`, `
` escapes in `write`). Nothing is kept after a reboot.

## Not built yet

ELF loader and per-process address spaces (apps from files), USB (xHCI) keyboard and mouse, NVMe/AHCI, networking, disk writes, IOAPIC and device interrupts, SMP, desktop, AI service, tickless idle and priority classes.

## The plan: read the research report

`G:\Aeon\reports\Aeon and clg improvements.md` ranks 21 improvements and gives a 10-step build order. Notes behind it: `G:\Aeon\research_notes\Aeon and clg improvements\`. Summary of the order:

1. Fix the record: `Cargo.toml` says 0.8.1 but the release commit said 0.9.0; the size claims in docs (115 to 200 KB) differ from the real image (about 245 KB signed, test cap 256 KiB); the roadmap overclaims `fs-fat`, `fs-blk`, `r-unsafe`, `t-apic`; stale README and design-doc lines.
2. Move disk I/O out of the interrupts-off lock (`src/vblk.rs` around lines 42 and 124-127; 2 s timeout stops the timer).
3. Make the interrupts-off rule checkable; fix `reset_after_fault` in `src/log.rs` (clears a busy flag an interrupted writer may hold).
4. Boot the signed stick on 2 or 3 real PCs and report what is seen.
5. CI (no `.github` yet). 6 and 7: pure scheduler-policy module and Banker's algorithm module (test numbers from `S:\clg\Operating Systems\bk`: FCFS 8.75, SJF 7.75). Then global-state cleanup, fuzzing and Miri, IOAPIC. After that: per-process address spaces, VFS, ELF loader, IPC, USB, NVMe.

## Decisions waiting for the owner (do not decide silently)

1. Goal: a real laptop, or learning in QEMU?
2. Keep the zero-dependency rule, or allow TLS libraries later?
3. Native syscall ABI or Linux-compatible?
4. Approve the names Dawn, Nexus, Pulse?
5. Rotate the signing key? (Current key is RSA 2048, unencrypted, certificate marked CA; rotating means everyone re-enrolls the certificate.)
6. Should menu Restart and Firmware setup stop counting as failed boots? (Recommended: yes.)
7. Install Kani, cargo-fuzz, nightly Miri component, loom? Each needs an explicit yes.
8. Next big piece: ELF loader with address spaces (recommended) or the USB keyboard?
9. Size budget for the image.
10. Push to GitHub? Not without being asked.
11. Bring back the wordmark above the shell log? (It was removed to give the log the whole screen; easy to add as a `logo` toggle.)
12. Up arrow scrolls the screen; command history is on Ctrl+Up/Down. Owner asked for this. Say if Up should recall commands when the line is empty.

## Owner's preferences and rules

- Plain, child-level explanations; tables when summarising. Terse replies (caveman mode on). Security warnings in plain sentences.
- Test before reporting. Do not send results that were not run. Show real outputs.
- Plan before multi-file code; check Context7 for library APIs; run tests or build before saying done; no secrets in code; never read or print `.env`, `*.pem`, SSH keys or cloud credentials; do not install plugins, skills or MCP servers without a yes; do not push.
- Mechanical work goes to the `quick-edit` subagent; main session on Sonnet.

## Tool gotchas

- Long Python patch scripts with quotes inside a Bash heredoc often break. Write them with the Write tool into the scratchpad and run them. Scripts use `edit(path, [(old, new), ...])` with exact unique matches.
- Writing straight to `G:\` with the Write tool fails (EPERM mkdir). Write to the scratchpad and `cp` it.
- `tests/run.sh` takes about 10 minutes: start it detached (`(bash tests/run.sh > target/full-suite.log 2>&1; echo FINISHED >> target/full-suite.log) &`) and wait for `FINISHED`.
- Typing in boot tests: `Vm.type` sends no Shift, so capital letters are dropped. `Vm.chord("ctrl", "up")` sends key combinations.
- PDFs: the Read tool cannot render them here; `pdftotext` is at `/mingw64/bin`.

## Other folders

- `S:\clg`: study library (OS, Rust, Elixir books, papers, labs, an old `rust-micro-os` prototype with nothing worth porting). Index: `S:\clg\Books\README.md` (it omits ElixirBooks). Library problems found: mislabelled PDFs (`Asterinas_..._2024.pdf` is an ML paper; the "Brinch Hansen" file is the Solaris Zones paper); no xHCI, NVMe, AHCI, ACPI, UEFI or virtio spec PDFs.
- `S:\Works\Projects\Aeon`: older copy of the project.

## Second session (2026-11-02): what changed, uncommitted

The owner tested on a real PC and found: files hold only one line, and no disk shows up. This session read the three notes (this file, the research report, the test guide) and did the following.

| Item | Result |
|---|---|
| One-line files (root cause: the input line is one screen row and Enter runs the command, so no newline could be typed) | Fixed: `edit <file>`, `append`, and `\n` / `\t` / `\` escapes in `write`. Tested in QEMU. |
| Tree (folders) | `src/ramfs.rs` rewritten: every entry is a path, folders exist, `resolve()` handles `.` `..` and the current folder. Shell: `mkdir rmdir cd pwd tree`, `ls [folder]`. Random-operation test keeps the byte and count books straight. |
| Report step 1, the record | Version set to 0.9.0 (matches the release commit message), size budget stated as "about 230 KB, test limit 256 KiB", README, design doc backlog and test guide refreshed. NOT done: roadmap HTML/JSON overclaims and the mock pages (`G:\Aeon\extras`, not in git). |
| Step 2, disk I/O inside the interrupts-off lock | Fixed in `src/vblk.rs`: the disk is guarded by a flag (`take_disk`, `release_disk`), not a `SpinLock`, so a slow request no longer stops the timer. A second task waits 10 ms and retries. |
| Step 3, interrupts-off rule | `Global::get_irqs_off()` stops the kernel if interrupts are on; the scheduler uses it for every access to its state. `log::reset_after_fault` now switches interrupts off itself. `global.rs` comment rewritten to state the real rule. |
| Step 5, CI | `.github/workflows/ci.yml` written (clippy x2, host tests, UEFI build, 256 KiB gate, reproducible-build check). NOT run: there is no remote. The QEMU suite is not in CI. |
| Steps 6 and 7 | `src/schedpolicy.rs` (FCFS, SJF, SRTF, priority, round robin) and `src/banker.rs` (Banker's, request check, deadlock detection, wait-for graph). Tested with the course's own numbers: FCFS 8.75, SJF 7.75, SRTF 6.5, RR(4) 11.75; Banker's Examples 1 to 3. Not wired into the kernel. |
| Boot stage vs kernel separation | Design only: `docs/design/boot-kernel-split.md`. No code changed (needs your decisions and a real-PC test). |

Finding in the course notes: Chapter 5's preemptive-priority example is wrong (P1 shown running 9 ticks of an 8-tick job; P3 finishing at 27 with 26 ticks of work). The correct average wait is 28/4 = 7.0, not 7.75. The test in `schedpolicy.rs` explains it.

Report item 16 (`sleep_ms` reads `active` without masking interrupts): checked, left alone. The flag is written only during init, so the read is harmless; the function needs interrupts ON for its no-scheduler wait.

Not done, and why:

| Item | Why not |
|---|---|
| Report step 4: boot on real PCs | Needs a person at a PC. The test guide section 8 says what to write down. |
| Step 8, safer global API | Large; this session only added the checked accessor for the scheduler. About 20 other `Global` sites still use plain `get`. |
| Step 9, fuzz and Miri | Needs your yes for each tool (decision 7). |
| Step 10, IOAPIC and IRQ1 keyboard | Hardware risk; needs the Step 8 cleanup first. |
| Real-PC disks (AHCI, NVMe), disk writes, address spaces, ELF loader, USB | Each is a large task (L). The PC test showed "no disk" because only virtio-blk exists. |
| Report item 17: shell `panic` command in the release image | The boot tests use it (`lastpanic`); putting it behind a feature needs a test-build change. Owner's call. |
| Roadmap and mock pages in `G:\Aeon\extras` | Outside git; `build.js` generates the roadmap. Fix with the roadmap's own tooling. |
| Re-signed `usb/` image and pendrive | DONE for `usb/` (commit `5984d8b`). Copying the three files to `D:` is still to do. |
| Commit | Done, see the first section. |

Choices made without you (change them if you disagree): version 0.9.0; size budget 256 KiB (the existing test gate); the disk flag lock instead of a sleeping mutex.

## Third pass (2026-11-02): docs audit and step 8

Asked to check `docs/` and finish what was open.

| Item | Result |
|---|---|
| Plan files | All four plans checked against code and git. `kernel-layers`, `console-upgrade` complete; `look-and-feel` complete with listed differences; `storage` Task 4 (release) still half open (needs `bash tools/release.sh` and the roadmap page). Checkboxes ticked, status block at the top of each plan. |
| Look-and-feel gaps closed | `t_text_clip` boot test (shell at 640x480); README screenshots section (`docs/img/`). |
| Bug found by that test | At 640 px the console has 71 columns, and six `help all` lines were longer, so they wrapped mid-word; the status bar was cut off. Fixed: help texts shortened, `cmdhelp` test now enforces 71 columns, status bar has a short form below 84 columns. |
| Step 8 (safer global state) | `get_irqs_off()` now guards the memory manager (`FRAMES`, `TOTAL_FRAMES`, `HEAP`), the logger (`LOGGER`, `RING`), the wait lists and the scheduler. Inventory of every `Global` and its rule: `docs/design/globals.md`. Known gap: `paging.rs` tables are not guarded against two tasks mapping pages at once (only the shell task does, today). |
| Tests | 225 host tests, 322 QEMU checks, 0 failed. Both clippy gates clean. |

Next in the report's order: step 4 (real PC; needs a person), 9 (fuzz/Miri; needs tool approval), 10 (IOAPIC, keyboard on IRQ1), then the real-disk driver (AHCI or NVMe, depends on what `pci` shows on the owner's PC), address spaces and the ELF loader.
