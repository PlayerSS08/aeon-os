# Plan: real processes, a clock and shell plumbing (v0.11.0)

Written 2026-11-17 after reading the whole kernel. The biggest gap for "a basic operating system" was
that programs could not be run from files: one ring-3 demo at a time, in a window inside the kernel's own
address space, with a tested ELF parser that nothing called.

Rule (unchanged): nothing is called working unless a host test or a QEMU boot proves it.

1. `vmspace` (pure, host-tested): per-process 4-level page tables in PML4 slot 2 (1 TiB..1.5 TiB), map,
   translate, copy in/out by walking the tables (no SMAP window, no faults), deep clone for `fork`, destroy.
2. `pipe`, `procargs`, `shparse`, `rtc` (pure, host-tested): pipe buffers with reader/writer counts, the
   System V start stack (argc/argv), `>` `>>` `|` `&` parsing, CMOS clock decoding and calendar maths.
3. `syscall`: drop the "legacy" number aliases; add fork, execve, wait4, pipe, dup2, kill, nanosleep,
   getcwd/chdir, mkdir, unlink, getdents64, uname, getppid; one allow-bit per call.
4. `usermode`: process table, per-process file descriptors, ELF loader on top of `elf::parse`, a syscall
   frame that preserves every register (Linux ABI) and makes `fork` possible. The socket calls that only
   pretended (hard-coded peer address, raw frames) are removed.
5. `sched`: a page-table root per task, switched on every context switch; a hook when a task is freed.
   `paging`: no user bit anywhere in the kernel half any more.
6. `user/`: a `no_std` Rust user library and small programs (hello, echo, cat, wc, forktest, memtest),
   built to static ELF, shipped in `/bin`.
7. Shell: run `/bin` programs by name with arguments, foreground wait, Ctrl+C, stdin, `&`, `>`/`>>`, `|`
   (stages run one after another through a buffer); `date` reads the real clock.
8. One lock between the shell task and system calls for the filesystem and USB (they raced before).
9. Tests: host tests for every pure module, a QEMU `proc` group, update `user`; docs and version bump.

Not in this plan: SMP, copy-on-write fork, signals, TLS, the Wi-Fi chip driver, NVMe.
