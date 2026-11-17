# Aeon 0.11.0: real processes

Written 2026-11-17. This release was built and checked on the host only: **the QEMU boot tests were not run
and nothing was tried on real hardware** (the owner asked to skip the boot tests). What was and was not
checked is listed at the end; read it before trusting the image.

## What is new

| Area | Before (0.10.0) | Now (0.11.0) |
|---|---|---|
| Address spaces | one identity-mapped space; one 64 KiB ring-3 "window" inside it, one program at a time | every program has its own 4-level page tables (`vmspace`); the kernel never dereferences a user pointer, it walks the tables (bad pointer = `-EFAULT`, never a kernel fault) |
| Running programs | five built-in assembly demos | static ELF64 programs from files (`elf` parser is now used by `loader`), argc/argv on a System V start stack, up to 28 processes at once |
| System calls | 17, with "legacy" alias numbers and fake sockets (hard-coded peer, raw frames) | 27 Linux-numbered calls: `fork`, `execve`, `wait4`, `pipe`, `dup2`, `kill`, `getdents64`, `getcwd`/`chdir`, `mkdir`, `unlink`, `nanosleep`, `uname`, `munmap`, ... Every register is preserved except `rax/rcx/r11`, as in Linux. The fake socket calls are gone |
| Shell | built-in commands only | unknown commands run `/bin/<name>` (or `/usb/bin/<name>`, or a path); `a \| b`, `> file`, `>> file`, `prog &`; Ctrl+C ends the foreground job; keyboard input reaches the running program line by line; a builtin may be the first stage of a pipe |
| Clock | `date` said there is no clock | `date` reads the CMOS clock (shown as clock time: the chip does not say which zone it holds); `clock_gettime` works |
| Shipped programs | none | `hello echo cat wc ls forktest` (Rust, `user/`), packed into the image (`initfs`) and copied to `/bin` at start |
| Races fixed | the shell task and system calls both used the file systems with no lock | the shell holds `FsGuard` while it works; file system calls wait for it |
| Hygiene | CI size gate said 512 KiB (image was already larger) | gate is 768 KiB in CI and in the boot test; image is 757 KB |

`kill <n>` takes a program's pid first, then a kernel task id (pids start at 100 so the two never collide).

## Design notes

* Memory layout of a process: image at 1 TiB + 4 KiB, `brk` heap from 1 TiB + 64 GiB, `mmap` area from
  1 TiB + 256 GiB, a 256 KiB stack just under the end of the user range (guard pages above and below).
  The kernel's two top-level page-table entries are copied into every process without the user bit.
* `fork` copies every page (no copy-on-write yet). `execve` builds the new image first and swaps only when
  it is complete, so a failed `execve` leaves the caller untouched.
* A process is one scheduler task. The scheduler loads the task's page-table root on every switch.
  A process killed from outside (Ctrl+C, `kill`) is cleaned up by the scheduler when it frees the task.
* System calls run with interrupts off; a call that must wait yields in a loop and re-checks.

## Wi-Fi driver, stage 1 (added the same day)

The OMEN's RTL8852BE-VT has no driver yet, and the Linux driver it must be ported from (`rtw89`, GPL-2.0 OR
BSD-3-Clause) is tens of thousands of lines, so it is being ported in stages that the laptop can check:

| Stage | Content | State |
|---|---|---|
| 1 | find the chip, enable it, map its registers (**BAR 2**; the old `wifi diag` read BAR 0, an I/O window), power the MAC on and off exactly like Linux's `rtw8852bt_pwr_on_func`/`pwr_off_func`, read the cut version and analog-die id | written (`src/rtw89.rs`, register names generated from Linux's headers into `src/rtw89_regs.rs`); sequences replayed on a fake register file in 9 host tests; **never run on the chip** |
| 2 | firmware download (`firmware/rtw8852bt_fw*.bin`): needs the PCI DMA rings and the DMAC/DLE/HFC set-up | not written |
| 3 | BB/RF tables, calibration, a raw receive | not written |
| 4 | scan, join, data path, hook into the existing WPA2 and IP code | not written |

`wifi probe` is read-only; `wifi probe on` also powers the chip on and off and prints every step. `report` now
includes the read-only probe. Stage 2 is written from what the first real run prints, so the first run on the
laptop is the next step. This is not "Wi-Fi works": for internet today use USB tethering or the wired port.

## Checked

* 660 host unit tests pass (47 new): page-table maths with a fake memory (`vmspace`), the ELF/stack loader
  end to end (`loader`), pipes, start-up stack, shell line parsing, CMOS clock decoding and calendar maths,
  system-call numbering and dirent records, the packed program archive. A test parses every shipped
  program with Aeon's own ELF checks.
* `cargo clippy --release -- -D warnings` clean on the kernel and on the host library; release build links.

## Not checked, and known limits

* **No boot of this image has been run**: not in QEMU, not on the OMEN. The context-switch code now loads
  CR3 on every task switch and the `syscall` entry was rewritten; both are exercised only by reading and by
  host tests of the pure parts. Treat the image as untested. The old signed 0.10.0 image is the fallback.
* The shipped Rust programs were built, and their ELF headers pass the loader's checks, but **none has been
  run on Aeon**, including `forktest` (which would exercise `fork`/`wait4`).
* `tests/boot_test.py` was updated for the new behaviour (the old "one program at a time" check now expects
  a second program to run) but was not executed. `tests/ca6_evidence.py` still describes the old single
  ring-3 slot and may need adjusting.
* No copy-on-write, signals, `fork` bomb limits beyond the 28-process table, TLS, SMP, Wi-Fi chip driver,
  I2C touchpad (still untested on hardware), NVMe.
* Pipes are polled (a waiting program yields instead of sleeping), so a blocked pipeline uses CPU time.
* File I/O through system calls reads and rewrites whole files (as the 0.10.0 `write` did): fine for small
  files, slow for large ones.

## Rebuilding

* Programs: `bash tools/build-user.sh` (needs `rustup target add x86_64-unknown-none`); it rewrites
  `user/bin/*` and `user/initfs.bin`, which are committed so the kernel build does not need that target.
* Kernel and tests: as in `docs/HANDOFF.md` (`cargo test --lib --target x86_64-pc-windows-gnu`,
  `cargo clippy --release -- -D warnings`, `cargo build --release`, `bash tools/release.sh`).
