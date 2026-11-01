# Aeon handoff: read this first in a new session

Last updated 2026-11-02. Branch `aeon-boot-upgrade` in `G:\Aeon\OS`. Nothing is pushed (no upstream, no PR).

## What Aeon is

A pure-Rust, `no_std`, zero-dependency UEFI x86_64 micro-OS. Kernel is called Nexus, shell Pulse, boot menu Dawn (names not yet approved by the owner). `G:\Aeon\extras` holds the roadmap and design pages (not in git).

## How to build, test, run

| Task | Command (from `G:\Aeon\OS`, Git Bash) |
|---|---|
| Host unit tests (195) | `cargo test --lib --target x86_64-pc-windows-gnu` (MSVC link.exe is not installed, so use the GNU target) |
| Clippy | `cargo clippy --release -- -D warnings` and `cargo clippy --target x86_64-pc-windows-gnu --lib -- -D warnings` |
| Release build | `cargo build --release` (output `target\x86_64-unknown-uefi\release\aeon.efi`) |
| Full test run (about 10 min, lint + host tests + QEMU boot tests in WSL Ubuntu) | `bash tests/run.sh` (run it detached and wait; it exceeds a 10 min tool limit). One test: `bash tests/run.sh -k console` |
| Latest result | 290 passed, 0 failed |
| See Aeon in a QEMU window | `powershell -ExecutionPolicy Bypass -File tools\run-qemu.ps1 -Build -Disk` |
| Signed release for the pendrive | `bash tools/release.sh` (builds with reproducible flags, signs in WSL with the key in `~/.aeon-keys`, rewrites `usb/`) |
| Tester's checklist | `G:\AEON-TEST-GUIDE.md` |

Boot-test screenshots land in `target\test-shots\` (the console test saves `console-a.png`, `console-up.png`).

## Git

- Commit with `git -c user.name="Scrooge-777" -c user.email="318593710+Scrooge-777@users.noreply.github.com" commit ...` (the shell has no git identity; do not change git config).
- Do not push unless the owner asks.
- Latest commits: `274fab4` signed `usb/` refresh, `80919c6` Up/Down scroll, `6247a6a` line editing, `499c228` scrollback and grouped help.

## State of the pendrive

`D:` (label AEON, FAT32) holds `EFI\BOOT\BOOTX64.EFI`, `aeon-secureboot.cer`, `SHA256.txt` from commit `274fab4`. The signing certificate is unchanged, so nothing needs re-enrolling. The copy at `S:\Works\Projects\Aeon` is old (from `ad5d657`).

## What is built (all tested in QEMU, none on real hardware yet)

- Boot: signed image, Secure Boot, TPM measured boot (PCR 11 and 12), event-log replay, SBAT, boot counting and recovery mode, Dawn menu, splash.
- Kernel: W^X paging to 1 TiB, SMEP/SMAP/UMIP, GDT/TSS/IDT with IST stacks, preemptive round-robin scheduler at 100 Hz, wait queues, SpinLock (used on 2 statics), ring 3 via syscall/sysret (5 built-in programs), LAPIC timer (PIT fallback), ACPI MADT, PCI scan, virtio-blk, read-only FAT16/FAT32.
- Console and shell: smooth fonts (Inter, JetBrains Mono), 512-line scrollback, Up/Down/PgUp/PgDn scroll, Ctrl+Home/End, block cursor, line editing (Left/Right/Home/End/Delete, Ctrl+A/E/B/F/U/K/W/D), Tab completion, Ctrl+Up/Down or Ctrl+P/N command history (last 8), did-you-mean, aliases (cls, dir, ver, man, free, uname, history), grouped `help`, `help <cmd>`, `help all`. The command table is `src/cmdhelp.rs`; a unit test checks it matches what `src/shell.rs` dispatches.
- No folders: the RAM file system is flat (`write` rejects `/`).

## Not built yet

ELF loader and per-process address spaces (apps from files), USB (xHCI) keyboard and mouse, NVMe/AHCI, networking, disk writes, IOAPIC and device interrupts, SMP, desktop, AI service, tickless idle and priority classes.

## The plan: read the research report

`G:\Aeon\reports\Aeon and clg improvements.md` ranks 21 improvements and gives a 10-step build order. Notes behind it: `G:\Aeon\research_notes\Aeon and clg improvements\`. Summary of the order:

1. Fix the record: `Cargo.toml` says 0.8.1 but the release commit said 0.9.0; the size claims in docs (115 to 200 KB) differ from the real image (about 230 KB signed, test cap 256 KiB); the roadmap overclaims `fs-fat`, `fs-blk`, `r-unsafe`, `t-apic`; stale README and design-doc lines.
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
