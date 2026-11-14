# Prompt for Antigravity: continue Aeon (paste everything below the line)

---

You are continuing work on **Aeon**, a pure-Rust `no_std` UEFI x86_64 micro-OS in `S:\Works\Projects\Aeon\OS` (branch `feat/create--user`, remote `https://github.com/Scrooge-777/aeon.git`). Kernel "Nexus", shell "Pulse", boot menu "Dawn". Version 0.9.2, signed image in `usb/`. Read these first, in order: `docs/SESSION-2026-11-03.md`, `docs/HANDOFF.md` (top section "2026-11-04"), `docs/superpowers/plans/2026-11-03-pendrive-storage.md`, `docs/COMPARISON.md`.

## Where it stands

- Last commit: `ebc4733` (local, **not pushed**): FAT32 writer done and tested. `origin/aeon-boot-upgrade` is at `7c1a0ad`; `main` has everything up to that point (PR #2 merged).
- Done: `src/fatw.rs` (write/replace file, mkdir, remove file, remove dir, free count), `src/fatname.rs`, `src/block.rs` (`BlockWrite`, `MemDiskMut`), 428 host tests, `tools/fatw-check.sh` (mkfs.fat images + `fsck.vfat`).
- **Not linked into the kernel.** The kernel has no USB mass-storage driver, so nothing can write to a pendrive yet.
- Existing drivers: virtio-blk, AHCI (read only), xHCI USB keyboard, read-only FAT16/32 reader (`src/fat.rs`), RAM filesystem with folders (`src/ramfs.rs`).

## Your job: the rest of the pendrive plan, in this order

1. **`src/usbmsd.rs` (pure logic, host-tested).** USB mass-storage bulk-only transport: command block wrapper (CBW) and status wrapper (CSW) encode/decode with signature and tag checks; SCSI commands INQUIRY, TEST UNIT READY, REQUEST SENSE, READ CAPACITY(10), READ(10), WRITE(10); sense-data decoding; capacity math. Tests first, including hostile device answers (wrong signature, wrong tag, short data, absurd capacity, zero block size). No `unsafe`.
2. **xHCI bulk endpoints** in the existing xHCI code (`src/xhci*.rs`, `src/xhcitrb.rs`, `src/usbdesc.rs`): find a mass-storage interface (class 0x08, subclass 0x06, protocol 0x50), configure bulk IN and OUT endpoints, transfer rings, polled completion with a timeout, stall/reset recovery. Keep the TRB and descriptor logic in the pure files with unit tests; the register code is hardware-only. QEMU test: `-device qemu-xhci -device usb-storage,drive=...`; Aeon must list the stick and read sector 0 (use a scratch image, not a real disk).
3. **Block device + cache + `sync`.** Implement `BlockDev` and `BlockWrite` for the USB stick. Write-back cache with an explicit `sync`; flush on `sync`, `reboot` and `shutdown`. QEMU test: write a file, `sync`, restart the VM on the same image, the file is still there, and `fsck.vfat -n` on the image is clean.
4. **Shell integration.** A mount (folder name `/usb`, owner can change it) where the existing commands (`ls cat cp mv rm mkdir rmdir write append edit touch tree find grep ...`) work on the stick through `fatw.rs`. `mv` between volumes is copy + remove. Add `sync`. Add boot tests for the refusals listed below.
5. **Docs and release:** update `docs/HANDOFF.md`, `docs/VERSIONS.md`, the plan file, `docs/ca6/CHECKLIST.md`; then (only when the owner says so) `bash tools/release.sh` for a signed 0.9.3.

Later, as separate plans (do not start without the owner): per-process page tables + loading programs from files (`src/elf.rs` is ready), `>` and `>>` redirection, tab completion, multi-core, the remaining security items S3 to S6 and S8 to S11 in `docs/COMPARISON.md`.

## Safety rules (never relax; each needs a test)

- Only a volume labelled exactly `AEON` is writable. Anything else stays read-only.
- Never write inside `EFI/` and never to the boot sector. `fatw.rs` already enforces this; do not bypass it.
- Validate every device answer: sector in range, lengths, tags, signatures. A hostile or broken stick must give an error, never a panic, a hang or an unbounded loop. Every wait has a timeout.
- Writes reach the disk only on `sync`, `reboot`, `shutdown`, or when the cache is full.
- **Do not write to any real physical disk or pendrive** (`D:` or any other) from tools or tests. Use image files and QEMU only. A test on the owner's real pendrive happens only when the owner asks, with a spare stick.

## Rules for working in this repo

- **Tests first, watch them fail, then write the minimum code.** Run the project's tests before saying anything is done and report failures plainly.
- Gates before every commit, all must pass:
  - `cargo test --lib --target x86_64-pc-windows-gnu` (MSVC `link.exe` is not installed, so use the GNU target)
  - `cargo clippy --release -- -D warnings`
  - `cargo clippy --target x86_64-pc-windows-gnu --lib -- -D warnings` (the host build hides `no_std` mistakes such as a missing `alloc::vec` import; the first clippy command catches them)
  - `bash tools/fatw-check.sh` when `fatw*.rs` or `block.rs` change
- QEMU tests run in WSL Ubuntu: `python3 tests/boot_test.py -k <group>`; first `cargo build --release`. See "How to run things" in `docs/SESSION-2026-11-03.md`. **Check free memory first** (`powershell Get-CimInstance Win32_OperatingSystem`): heavy QEMU runs were killed once when Windows had about 1.3 GB free; aim for 3 GB or more, and never restart a killed run on your own.
- Pure logic goes in its own host-testable module (like `fatw.rs`, `fatname.rs`, `usbmsd.rs`); hardware code stays thin. Keep `unsafe` minimal, each block with a `// SAFETY:` comment.
- No third-party crates, no new tools or plugins installed without the owner's yes.
- **Never read, print, or commit** `.env`, `*.pem`, SSH keys, cloud credentials, or anything in `~/.aeon-keys` (the signing key). Signing is done only by `bash tools/release.sh`, and only when asked.
- Commit identity (the shell has none; do not change git config): `git -c user.name="Scrooge-777" -c user.email="318593710+Scrooge-777@users.noreply.github.com" commit ...`. Short messages in the style `feat(usb): ...`, `test: ...`, `docs: ...`.
- **Push only when the owner asks. Never force-push.**
- Git Bash rewrites `/mnt/...` paths: use `MSYS_NO_PATHCONV=1` when passing WSL paths. Python is not installed on Windows; run it through WSL (`python3`).
- Owner preferences: plain words, short answers, tables, tests before claims, ask before big decisions. Networking and apps stay at zero; do not add them.

## What to report at the end of each step

What changed (files), test counts (host, QEMU groups), anything that failed or was skipped, and the exact next command.
