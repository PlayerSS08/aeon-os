# Aeon kernel layers Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans (native, single session). Steps use checkbox syntax.

**Goal:** Add the missing kernel layers after boot: a RAM filesystem, ring-3 user mode with system calls, and an APIC timer in place of the legacy PIT/PIC.

**Architecture:** Pure logic (filesystem, user-pointer checks, MADT parsing) lives in host-testable modules with unit tests; hardware code (GDT user segments, SYSCALL entry, LAPIC) lives in `arch`-side modules behind `cfg(target_os = "uefi")`. Every layer is proven by a QEMU boot test in `tests/boot_test.py`.

**Tech Stack:** Rust `no_std`, `x86_64-unknown-uefi`, inline asm only, zero dependencies, QEMU+OVMF in WSL.

**Spec:** the user's request of 2026-11-01 ("build the rest of the layers; design and looks later") plus the open kernel list in memory `aeon.md`.

## Global Constraints

- Zero dependencies, no `std`; image stays signed and under 200 KB.
- Never weaken existing protections: NX, W^X, CR0.WP, SMEP, SMAP, UMIP stay on.
- `clippy::undocumented_unsafe_blocks` and `missing_safety_doc` are denied: every `unsafe` gets a `// SAFETY:` comment.
- Stage files by path; never `git add -A`. Do not push. No Claude attribution in commits.
- `src/fb.rs` and splash code are out of scope (design is later).
- Non-goals: SMP, xHCI USB, networking, aarch64 (not testable here or too large).

## Review Focus

- Filesystem name edge cases: empty, 64+ chars, `/`, duplicate names, NUL bytes must be rejected, not stored.
- Filesystem full (file count and byte budget): write fails with an error, existing files stay intact.
- User pointer validation: pointer+len overflow, kernel-range pointers, zero length, pointer straddling the user/kernel boundary.
- A user program that faults (touches kernel memory, executes `hlt`, bad syscall number) must be killed while the kernel and shell keep running.
- A user program that never yields must still be preempted by the timer.

---

### Task 1: RAM filesystem (host-testable core)

**Files:**
- Create: `src/ramfs.rs`; Modify: `src/lib.rs` (add `pub mod ramfs;`)
- Test: unit tests inside `src/ramfs.rs`

**Interfaces:**
- Produces: `pub struct RamFs`, `RamFs::new(max_files: usize, max_bytes: usize) -> RamFs`, `write(&mut self, name: &str, data: &[u8]) -> Result<(), FsError>`, `read(&self, name: &str) -> Option<&[u8]>`, `remove(&mut self, name: &str) -> bool`, `names(&self) -> impl Iterator<Item=(&str, usize)>`, `used_bytes(&self) -> usize`; `pub enum FsError { BadName, Full, TooBig }`; `pub const MAX_NAME: usize = 32`.

- [ ] **Step 1:** Write failing tests: `rejects_bad_names` (empty, 33 chars, contains `/`, contains `\0`, `.`/`..`), `write_read_roundtrip`, `overwrite_replaces_and_frees_old_bytes`, `full_by_count_keeps_existing`, `full_by_bytes_keeps_existing` (write of too-big overwrite leaves old content), `remove_frees_space`, `names_sorted`.
- [ ] **Step 2:** Run `cargo test --lib ramfs` (host target). Expected: FAIL, module missing.
- [ ] **Step 3:** Implement with `alloc::vec::Vec<(String, Vec<u8>)>` kept sorted by name; byte budget counts file data only; overwrite checks budget after subtracting the old size.
- [ ] **Step 4:** Run tests, expect PASS. Run `cargo clippy --lib`.
- [ ] **Step 5:** Commit `feat(fs): RAM filesystem core with name and budget limits`.

### Task 2: Filesystem shell commands

**Files:**
- Modify: `src/shell.rs` (commands `ls`, `cat`, `write`, `rm`, `df`), `src/main.rs` (create the global `RamFs`, 64 files / 1 MiB; seed `readme.txt` and `hello.bin` placeholder), `tests/boot_test.py`
- Test: new `t_fs(ctx)` in `tests/boot_test.py`, registered in `main()`

**Interfaces:**
- Consumes: Task 1 `RamFs`.
- Produces: `static FS: Global<RamFs>` accessor `fs() -> &'static mut RamFs` in `src/main.rs`; shell syntax `write <name> <text...>`.

- [ ] **Step 1:** Write `t_fs`: boot, run `write a.txt hello world`, `cat a.txt` expects `hello world`, `ls` lists `a.txt`, `rm a.txt` then `cat a.txt` prints `no such file`, `write` with a 40-char name prints `bad name`, `df` shows `used`.
- [ ] **Step 2:** Run it, expect FAIL (`unknown command`).
- [ ] **Step 3:** Add the commands and update the `help` text and README command list.
- [ ] **Step 4:** Run `bash tests/run.sh`, expect all previous tests plus `t_fs` pass.
- [ ] **Step 5:** Commit `feat(shell): ls cat write rm df on the RAM filesystem`.

### Task 3: User-pointer validation and syscall table (host-testable)

**Files:**
- Create: `src/usercopy.rs`, `src/syscall.rs`; Modify: `src/lib.rs`
- Test: unit tests in both files

**Interfaces:**
- Produces: `usercopy::USER_BASE: u64 = 0x4000_0000`, `USER_END: u64 = 0x4010_0000` (1 MiB window); `usercopy::check(ptr: u64, len: u64) -> bool` (true only when `len` may be 0, and `[ptr, ptr+len)` lies inside the window, with checked overflow); `syscall::Number` enum `Write=1, Exit=2, Yield=3, Uptime=4, GetPid=5`; `syscall::decode(n: u64) -> Option<Number>`.

- [ ] **Step 1:** Tests: `check_rejects_overflow` (`ptr=u64::MAX-1,len=8`), `check_rejects_kernel_range`, `check_rejects_straddle`, `check_accepts_zero_len_inside`, `check_accepts_exact_window`, `decode_unknown_is_none`.
- [ ] **Step 2:** Run, expect FAIL.
- [ ] **Step 3:** Implement both with checked arithmetic.
- [ ] **Step 4:** Run, expect PASS.
- [ ] **Step 5:** Commit `feat(user): user pointer checks and syscall numbers`.

### Task 4: Ring-3 execution

**Files:**
- Modify: `src/arch.rs` (GDT gets user code/data: layout `[null, kcode, kdata, user32-null, udata=0x1B... ]` per SYSRET rules, STAR/LSTAR/SFMASK/EFER.SCE setup, TSS.rsp0 per task), `src/paging.rs` (map the user window with U/S bit, RW+NX for data, RX for code; kernel pages stay supervisor-only), `src/sched.rs` (user task kind: entry via `iretq`), `src/main.rs` (syscall dispatch `extern "C" fn syscall_dispatch(n, a0, a1, a2) -> u64`)
- Create: `src/userprog.rs` (hand-assembled hello program as a `const [u8; N]`; a faulting program; a looping program)
- Test: `t_user(ctx)` in `tests/boot_test.py`

**Interfaces:**
- Consumes: Task 3 `usercopy::check`, `syscall::decode`.
- Produces: `sched::spawn_user(name: &str, code: &[u8]) -> Option<usize>`; shell command `run <hello|fault|spin>`.

- [ ] **Step 1:** Write `t_user`: `run hello` prints `hello from ring 3` and a clean exit line; `run fault` prints `user task killed` and then the shell still answers `uptime`; `run spin` then `counters`/`ps` still progress (preemption), `kill` ends it; `security` output now says `ring 3: yes`.
- [ ] **Step 2:** Run, expect FAIL.
- [ ] **Step 3:** Implement. SMAP stays enabled: the `write` syscall copies through `stac`/`clac` after `usercopy::check`. User page faults and general-protection faults from CPL 3 kill the task instead of panicking (check saved CS RPL in the exception handler).
- [ ] **Step 4:** Run `bash tests/run.sh`; also confirm `fault-*` feature tests still pass.
- [ ] **Step 5:** Run `/security-review` on the diff (runs user-supplied code paths). Fix findings.
- [ ] **Step 6:** Commit `feat(kernel): ring 3 user tasks and syscalls`.

### Task 5: MADT parsing (host-testable)

**Files:**
- Modify: `src/acpi.rs` (add `parse_madt`), tests in same file

**Interfaces:**
- Produces: `pub struct Madt { pub lapic_base: u32, pub cpus: u8, pub ioapic_base: Option<u32> }`, `pub fn parse_madt(t: &[u8]) -> Option<Madt>`; ignores disabled CPUs, tolerates entries with length 0 (returns `None`, no infinite loop).

- [ ] **Step 1:** Tests: `madt_two_cpus_one_ioapic` (bytes built in the test), `madt_zero_length_entry_is_none`, `madt_truncated_is_none`, `madt_disabled_cpu_not_counted`.
- [ ] **Step 2:** Run, expect FAIL. Step 3: implement. Step 4: PASS.
- [ ] **Step 5:** Commit `feat(acpi): parse the MADT`.

### Task 6: LAPIC timer

**Files:**
- Create: `src/apic.rs` (UEFI-only); Modify: `src/main.rs` (use LAPIC if MADT and CPUID report one, else keep PIT), `src/arch.rs` (spurious vector, EOI path), `tests/boot_test.py`

**Interfaces:**
- Consumes: Task 5 `Madt`.
- Produces: `apic::init(madt: &Madt) -> bool`, `apic::start_timer(hz: u64)` (calibrated against the TSC already calibrated at boot), `apic::eoi()`. Shell `info` reports `timer: lapic` or `timer: pit`.

- [ ] **Step 1:** Test `t_apic`: boot with `-cpu max` default, run `info`, expect `timer: lapic`; `uptime` advances after 2 s; with `nolapic` boot option, expect `timer: pit` and the system still works.
- [ ] **Step 2:** Run, expect FAIL.
- [ ] **Step 3:** Implement: mask the 8259 completely, map the LAPIC page uncached in the identity map (MMIO, NX), software-enable via spurious register, periodic LAPIC timer on the existing timer vector. Add the `nolapic` option to `bootopts.rs` with a unit test (it is part of the measured command line).
- [ ] **Step 4:** Run the whole suite.
- [ ] **Step 5:** Commit `feat(kernel): LAPIC timer with PIT fallback`.

### Task 7: Release

**Files:** `README.md`, `Cargo.toml` (version 0.7.0), `usb/`

- [ ] **Step 1:** Update README tables (commands, boot stages, security list, non-goals).
- [ ] **Step 2:** `bash tools/release.sh`, then `bash tests/run.sh`; expect 0 failed.
- [ ] **Step 3:** Commit `chore(release): 0.7.0`. Leave unpushed.
