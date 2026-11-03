# Testing and Security Practice for Hobby and Research Rust Kernels

## Testing: which practices to adopt

### Takeaway
For a kernel that already has host unit tests and a 360+ check QEMU boot suite, the cheapest high-value additions are host-side fuzzing of byte parsers (FAT, ELF, event log), Miri on the pure unsafe modules, and a CI job that runs the existing host and QEMU suites. Loom and coverage/mutation testing are conditional on specific code.

### Cited Findings
- cargo-fuzz is the Rust Fuzz Book's recommended tool. It is a driver for libFuzzer (via the `libfuzzer-sys` crate), not a fuzzer itself. Setup is `cargo install cargo-fuzz` and `cargo fuzz init`. — [Rust Fuzz Book](https://rust-fuzz.github.io/book/print.html)
- libFuzzer needs LLVM sanitizer support, so the book lists only x86-64 Linux, x86-64 macOS and aarch64 macOS as supported hosts, and it needs a C++11 compiler. — [Rust Fuzz Book](https://rust-fuzz.github.io/book/print.html)
- A third-party skill page says cargo-fuzz requires nightly and is run as `cargo +nightly fuzz run`. This is a secondary source, not verified against the official docs. — [claudepluginhub cargo-fuzz skill](https://www.claudepluginhub.com/skills/commit-ai-testing-handbook-skills-plugins-testing-handbook-skills/cargo-fuzz)
- Miri is an undefined-behavior detector. It catches out-of-bounds access, use-after-free, uninitialized reads and misaligned references, and it has experimental Stacked Borrows and Tree Borrows checks. Run it with `rustup +nightly component add miri` and `cargo miri test`. — [rust-lang/miri README](https://github.com/rust-lang/miri)
- Miri limits, quoted from its README: "Miri fundamentally cannot ensure that your code is sound." It "tests one of many possible executions of your program." Weak-memory emulation "is not complete." Windows is supported less well than Linux; the README suggests `--target x86_64-unknown-linux-gnu` on Windows. — [rust-lang/miri README](https://github.com/rust-lang/miri)
- Miri gives no access to most platform APIs or FFI. Unsupported operations fail with an explicit error. The `-Zmiri-native-lib` flag is experimental, Unix-only, and "unsound" in that Miri stops tracking initialization and provenance. — [rust-lang/miri README](https://github.com/rust-lang/miri)
- Loom explores concurrent executions under the C11 memory model. It is set up as a `cfg(loom)` dependency, tests are written inside `loom::model`, and it is run with `RUSTFLAGS="--cfg loom"`. — [tokio-rs/loom](https://github.com/tokio-rs/loom), [docs.rs loom 0.7.1 README](https://docs.rs/crate/loom/0.7.1/source/README.md)
- Loom treats `SeqCst` accesses as `AcqRel`, which can cause false alarms. It also does not explore every execution the C11 model allows, so a clean run is not proof. — [docs.rs loom 0.7.1 README](https://docs.rs/crate/loom/0.7.1/source/README.md)
- proptest is a QuickCheck/Hypothesis-style property-testing framework. It generates inputs from `Strategy` types, shrinks failing inputs to a minimal case, and integrates with the standard test harness. — [docs.rs proptest source](https://docs.rs/crate/proptest/latest/source)
- Linux's in-kernel Rust test docs describe KUnit, `#[test]` and Kselftests. This is the only official kernel-Rust testing page I found. It is Linux, not a hobby kernel. — [kernel.org Rust testing](https://kernel.org/doc/Documentation/rust/testing.rst)

### Inferences
- Host-side fuzzing is possible for a kernel only when the parsers are pure code that builds for the host. Verify that FAT, ELF and event-log parsers are separable from kernel-only APIs before planning this. (Inference from the cargo-fuzz host requirements.)
- Miri can run on pure modules whose tests are hosted (`cargo miri test`). It cannot run boot code, MMIO, page-table manipulation or inline asm, so it only covers the host-testable slice of unsafe code. (Inference from the Miri limits above.)
- Loom is worth adding only where the kernel has its own locks, atomics or lock-free queues that run on more than one CPU. Otherwise it adds cost with no coverage. (Inference.)
- proptest is the cheaper complement to fuzzing for round-trip and invariant tests on the same parsers, and it runs in ordinary `cargo test`. (Inference.)

### Gaps
- No source fetched covers QEMU-based GitHub Actions with OVMF in detail, so CI setup is not sourced here. The Theseus search returned no testing guide. The Redox search was not run.
- Coverage (cargo-llvm-cov) and mutation testing (cargo-mutants) were not researched.
- No source gives a 2026 comparison of fuzzing throughput or bug yield on kernel parsers.

## Hardening: CPU and kernel protections

### Takeaway
SMEP, SMAP and UMIP are CPU-enforced protections that the Linux kernel now protects from being turned off. Their sourced details are good, but the research did not cover KASLR, CET, retpolines, IBRS, KPTI, TPM measured boot, Secure Boot/SBAT or reproducible builds, so those are open gaps.

### Cited Findings
- SMEP prevents supervisor-mode code from fetching instructions from user-accessible linear addresses. SMAP stops accidental kernel access to user memory. — [Linux self-protection doc (DragonFly mirror)](https://gitweb.dragonflybsd.org/linux.git/blob/f4a8871f9f347b185c44525c9bb1755951f94841:/Documentation/security/self-protection.rst)
- UMIP makes SGDT, SLDT, SIDT, SMSW and STR raise a general protection fault in user mode. The Linux patch series says it is enabled by default and can be turned off with `clearcpuid=514`. — [LKML UMIP patch v10](https://lkml.iu.edu/hypermail/linux/kernel/1710.3/03405.html)
- **Conflict:** A later Linux commit describes UMIP as disabled by default at build time, with a build-time option to enable it. The two sources disagree on the default. Check the current Linux Kconfig before relying on either. — [LKML UMIP runtime-enable commit](https://lkml.iu.edu/hypermail/linux/kernel/1711.1/00325.html)
- Linux pins the SMAP, SMEP and UMIP bits in `native_write_cr4`, so exploits cannot trivially clear them. Phoronix reports the motivation was exploits calling `native_write_cr4` to disable SMEP/SMAP. — [Phoronix](https://www.phoronix.com/news/Linux-Protect-Special-CR4-Bits)
- The Linux self-protection design goals say a mechanism should be effective, on by default, need no developer opt-in, have no performance cost, not block kernel debugging, and have tests. — [Linux self-protection doc (DragonFly mirror)](https://gitweb.dragonflybsd.org/linux.git/blob/f4a8871f9f347b185c44525c9bb1755951f94841:/Documentation/security/self-protection.rst)
- Tock's threat model separates the kernel, capsules and userland, and relies on MPU hardware to isolate apps. The current model lives in the official repo's `docs/` folder, which I did not read directly. — [Tock getting started](https://tockos.org/documentation/getting-started) (search only; threat model not fetched)

### Inferences
- The project already has SMEP/SMAP/UMIP and W^X. Adding the CR4 pinning pattern and checking that the bits are set at every AP boot is a small, sourced step. (Inference from the Phoronix item.)
- The design goal "have tests" suggests each hardening feature should have a QEMU boot check that fails if the bit is clear. The project's 360+ check suite may already do this. Verify. (Inference.)

### Gaps
- Not researched: KASLR, guard-page best practice beyond the project's own stack guard, stack canaries, CET/shadow stacks, retpolines, IBRS, KPTI, TPM measured boot, Secure Boot chain (shim, SBAT), signing and key handling, reproducible builds.
- The Redox security documentation and the kernsec.org self-protection pages were not fetched.
- The Intel SDM and AMD APM were not fetched, so no primary bit-level citations are included.

## Unsafe-code discipline

### Takeaway
Clippy's `undocumented_unsafe_blocks` lint enforces a `// SAFETY:` comment on each unsafe block, and its companion `unnecessary_safety_comment` catches stale ones. Both lints have open bugs. No sourced figure for how many unsafe blocks typical Rust kernels contain was found.

### Cited Findings
- `undocumented_unsafe_blocks` was added in Clippy 1.58.0 in the `restriction` group. It flags unsafe blocks without a `// SAFETY:` comment. — [Clippy lint source](https://rust.googlesource.com/rust-clippy/+/refs/heads/beta/clippy_lints/src/undocumented_unsafe_blocks.rs) (version and group from search snippet; the official lint page fetch hit a long page and did not reach this lint)
- `unnecessary_safety_comment` was added in 1.67.0 and flags safety comments on code that does not need one. — [Clippy lint source](https://rust.googlesource.com/rust-clippy/+/refs/heads/beta/clippy_lints/src/undocumented_unsafe_blocks.rs)
- Open issues: a `// SAFETY:` comment above a multi-line `#[allow]` triggers a false positive ([clippy #15915](https://github.com/rust-lang/rust-clippy/issues/15915)); a comment placed between attributes is not found ([clippy #15754](https://github.com/rust-lang/rust-clippy/issues/15754)); the two lints can both flag the same comment ([clippy #15755](https://github.com/rust-lang/rust-clippy/issues/15755)).
- PR #15678 changed how `unnecessary_safety_comment` treats `unsafe fn`, suggesting a `# Safety` doc section instead. The author said a larger refactor is needed. — [clippy #15678](https://github.com/rust-lang/rust-clippy/pull/15678)

### Inferences
- Enable `clippy::undocumented_unsafe_blocks` in CI as a warning, and treat its open false positives as known noise. (Inference.)
- Miri and fuzzing find bugs in the unsafe code; the SAFETY comment is the review artifact. Pair the lint with a manual review of each unsafe block that touches page tables or MMIO. (Inference.)

### Gaps
- cargo-geiger (unsafe counting) was not researched. No sourced count of unsafe blocks in Redox, Theseus, Tock or blog_os was found, so the "typical kernel" number is unknown.

## Priority ranking for the current kernel

### Takeaway
Ranked by value for effort, for a kernel that already has host unit tests, a QEMU suite, W^X, SMEP/SMAP/UMIP, guard pages, Secure Boot signing and TPM measured boot, but no fuzzing, Miri or CI runs.

### Cited Findings
- Ranking inputs are from the sources above: cargo-fuzz host requirements ([Rust Fuzz Book](https://rust-fuzz.github.io/book/print.html)), Miri scope and limits ([rust-lang/miri](https://github.com/rust-lang/miri)), loom scope ([docs.rs loom](https://docs.rs/crate/loom/0.7.1/source/README.md)), and Clippy lint status ([Clippy source](https://rust.googlesource.com/rust-clippy/+/refs/heads/beta/clippy_lints/src/undocumented_unsafe_blocks.rs)).

### Inferences
Ranked, highest value for effort first (this ranking is my judgment, not a sourced result):
1. **CI on the existing suites** (host tests plus QEMU boot suite, on GitHub Actions). Low effort, and it makes every other item stick. Needs OVMF and QEMU install steps, which are not sourced here.
2. **Miri on the pure unsafe modules.** Low effort if the modules are host-testable. Catches UB that tests miss.
3. **cargo-fuzz on FAT, ELF and event-log parsers** (host-side, nightly, x86-64 Linux). Medium effort. Highest bug-finding yield for parsers that read untrusted bytes.
4. **Clippy `undocumented_unsafe_blocks` in CI.** Very low effort. Accept the known false positives.
5. **proptest round-trip and invariant tests** for the same parsers. Low effort, complements fuzzing in ordinary `cargo test`.
6. **Supply-chain checks (cargo-audit, cargo-deny).** Low effort; not researched here, so see the gaps.
7. **Loom**, only if the kernel has multi-CPU lock-free or atomic code. Conditional.
8. **Hardening extras** (CET, KPTI, retpolines, IBRS, KASLR) depend on target CPUs and threat model. Not researched, so no ranking is given.

### Gaps
- Coverage and mutation testing (cargo-llvm-cov, cargo-mutants) were not researched, so their rank is unsupported.
- cargo-deny, cargo-vet and cargo-audit were not researched.
- No Redox, Theseus or Tock testing-practice source was fetched in full; Theseus returned no testing guide.
- Speculative-execution mitigations, TPM and Secure Boot items were not researched.
- "Current as of 2026" is not confirmed for every source. Search snippets carried no dates, and the Clippy and Linux pages may have changed since they were indexed.
