# Reference OS projects for a small Rust teaching OS

Research date: 2026-11-03. Aeon context from `G:\Aeon\OS\docs\HANDOFF.md` (read 2026-11-02): Aeon has W^X paging, a preemptive round-robin scheduler at 100 Hz, ring 3 via syscall/sysret, LAPIC timer, virtio-blk, read-only FAT, a RAM filesystem and TPM measured boot. It does not yet have an ELF loader, per-process address spaces, a VFS, IOAPIC, USB, networking or SMP.

Method: repository metadata (licence, pushed_at, archived flag, stars, default branch) comes from the GitHub or GitLab API, read on 2026-11-03. Feature claims come from fetched READMEs, book pages and repo listings. Search results are marked "(search lead)" and were not checked against the page itself. "Pushed" is the date of the last push to the default branch. It is not a release date or proof of maintenance.

Size figures are GitHub language byte counts, not lines of code. Four repos returned usable breakdowns (see Q1). No LOC count was made.

---

## Q1. Architecture, licence, size, maturity and 2026 activity

### Takeaway
Most listed projects were pushed in 2026. Theseus is the exception: its last push is 2024-09-22, which contradicts its README's "under active development" line. Redox has the most 2026 activity, with a monthly report and an EEVDF scheduler rewrite. For a UEFI x86_64 teaching OS, the closest comparators are MikanOS (UEFI, x86_64, but last pushed 2023 and documented in Japanese), blog_os with the rust-osdev crates, and xv6 (C, RISC-V, MIT). None of them covers Aeon's full set (Secure Boot, TPM measured boot, W^X, signed image) in one place.

### Cited Findings

**Redox OS** (Rust, microkernel, general-purpose)
- Licence MIT, copyright "Redox OS Developers (2016)" — [Redox LICENSE](https://gitlab.redox-os.org/redox-os/redox/-/raw/master/LICENSE)
- Primary repo: GitLab, last activity 2026-11-02, 136 stars, default branch `master` — [Redox GitLab API](https://gitlab.redox-os.org/api/v4/projects/redox-os%2Fredox). No licence field in that API response.
- GitHub mirror `redox-os/redox`: MIT, pushed 2026-09-27, 16,679 stars, description "Mirror of https://gitlab.redox-os.org/redox-os/redox" — [GitHub API](https://api.github.com/repos/redox-os/redox). The mirror's language breakdown is Rust about 347 KB, Shell about 95 KB, Makefile about 68 KB. That is the build/meta repo; the Repository Structure page names cookbook and the redox project, not a kernel repo — [Redox book, repository structure](https://doc.redox-os.org/book/repository-structure.html). The kernel size is therefore not measured here.
- 2026 monthly reports, from the Redox news index — [Redox news](https://www.redox-os.org/news/):
  - Apr 30: Bochs video driver replaced by VESA; xHCI driver loads config at runtime; RISC-V switched to Sv39; faster multi-core boot; tmux ported — [This Month in Redox, April 2026](https://www.redox-os.org/news/this-month-260430/)
  - Sep 30: direct context switching that bypasses the scheduler for service calls (IPC latency down 24%); page-fault handling surfaces as POSIX signals on x86-64 and i586; Clang cross-compile; Google Cloud boot — [This Month in Redox, September 2026](https://www.redox-os.org/news/this-month-260930/)
  - Aug 22, RSoC 2026: EEVDF scheduler replaces DWRR; per-CPU run queues; work stealing between cores; the change is in `select_next_context` — [RSoC 2026: EEVDF for Redox](https://www.redox-os.org/news/rsoc-eevdf/)
  - Index titles only (post not opened): Capability-based security (Mar 17), DWRR scheduler (Apr 2), monthly reports Jan, Feb, Mar, May, Jun, Jul, Aug — [Redox news](https://www.redox-os.org/news/)
- Secondary coverage (Phoronix) for January 2026 Cargo and rustc running on Redox — [Phoronix, Jan 2026](https://phoronix.com/news/redox-os-january-2026). Not checked against the primary monthly report.
- No formal versioned release for 2026 was found.

**Theseus OS** (Rust research OS, "intralingual design")
- Licence MIT. Default branch `theseus_main`. 3,222 stars. Not archived — [Theseus GitHub API](https://api.github.com/repos/theseus-os/Theseus)
- Last push 2024-09-22. The commits page's most recent commit is 2024-09-22 — [Theseus commits](https://github.com/theseus-os/Theseus/commits/theseus_main)
- README: "Theseus is under active development, and although it is not yet mature" and 1,263 commits — [Theseus README](https://github.com/theseus-os/Theseus). This conflicts with the 2024 date. Treat as stale unless a newer commit is found.
- Language bytes: Rust about 4.29 MB, Makefile 64 KB, assembly 43 KB, C 8 KB — [Theseus languages API](https://api.github.com/repos/theseus-os/Theseus/languages)
- x86 is the primary target; aarch64 is "an ongoing effort" — [Theseus README](https://github.com/theseus-os/Theseus)
- Docs: rustdoc for crates and a Markdown design book — [Theseus book](https://theseus-os.github.io/Theseus/book/index.html)

**blog_os / "Writing an OS in Rust"** (Philipp Oppermann)
- Licence Apache-2.0. Pushed 2026-10-28. 17,789 stars. Not archived — [blog_os GitHub API](https://api.github.com/repos/phil-opp/blog_os)
- Book is in its second edition — [os.phil-opp.com](https://os.phil-opp.com/)
- Status page lists updates through July 2026 — [blog_os status updates](https://os.phil-opp.com/status-update/)

**bootloader crate** (rust-osdev)
- Licence Apache-2.0. Pushed 2026-08-18. 1,701 stars. Description "An experimental pure-Rust x86 bootloader" — [bootloader GitHub API](https://api.github.com/repos/rust-osdev/bootloader)
- README: "An experimental x86_64 bootloader that works on both BIOS and UEFI systems." Loads an ELF kernel from a FAT partition. Kernel entry is `fn kernel_main(boot_info: &'static mut BootInfo) -> !` — [bootloader README](https://github.com/rust-osdev/bootloader)
- Version: a January 2026 This Month draft mentions bootloader 0.11.14. Source is a PR diff, not the published post — [homepage PR 268 patch](https://github.com/rust-osdev/homepage/pull/268.patch)
- Language bytes: Rust about 326 KB, linker script 2 KB, assembly 1 KB — [bootloader languages API](https://api.github.com/repos/rust-osdev/bootloader/languages)

**x86_64 crate** (rust-osdev)
- Licence Apache-2.0. Pushed 2026-10-29. 942 stars. Description "Library to program x86_64 hardware." — [x86_64 GitHub API](https://api.github.com/repos/rust-osdev/x86_64)
- Feature list not fetched.

**uefi-rs** (rust-osdev)
- Licence Apache-2.0. Pushed 2026-11-02. 1,674 stars. "Rusty wrapper for the Unified Extensible Firmware Interface (UEFI)" — [uefi-rs GitHub API](https://api.github.com/repos/rust-osdev/uefi-rs)
- March 2026 draft: uefi 0.37 and uefi-raw 0.14.0 — [homepage PR 263](https://github.com/rust-osdev/homepage/pull/263/files) (draft, not the published post)

**Hermit** (Rust unikernel)
- Licence dual Apache-2.0 / MIT (top-level LICENSE-APACHE and LICENSE-MIT). Pushed 2026-11-02. 1,515 stars. Default branch `main`. "A Rust-based, lightweight unikernel." — [Hermit GitHub API](https://api.github.com/repos/hermit-os/kernel); [Hermit repo](https://github.com/hermit-os/kernel)
- Language bytes: Rust about 1.21 MB, assembly 15 KB, Python 13 KB — [Hermit languages API](https://api.github.com/repos/hermit-os/kernel/languages)
- Targets: x86_64 as the example, with aarch64 and riscv64 as alternatives — [Hermit README](https://github.com/hermit-os/kernel)

**Tock OS** (Rust, embedded, process isolation)
- Pushed 2026-11-02. 6,486 stars. Default branch `master`. GitHub API reports licence NOASSERTION — [Tock GitHub API](https://api.github.com/repos/tock/tock). The LICENSE-APACHE file was fetched and is Apache-2.0 — [Tock LICENSE-APACHE](https://github.com/tock/tock/blob/master/LICENSE-APACHE). The MIT counterpart was not fetched.
- README: "designed for running multiple concurrent, mutually distrustful applications on Cortex-M and RISC-V based embedded platforms." No x86 mention. "Tock uses memory protection units to isolate applications from each other and the kernel" — [Tock README](https://github.com/tock/tock)
- Language bytes: Rust about 14.8 MB, Makefile 110 KB, linker script 72 KB — [Tock languages API](https://api.github.com/repos/tock/tock/languages)

**SerenityOS** (C++ desktop OS)
- Licence BSD-2-Clause. Pushed 2026-11-02. 33,937 stars — [SerenityOS GitHub API](https://api.github.com/repos/SerenityOS/serenity)
- Language bytes: C++ about 47.8 MB, plus JavaScript 2.6 MB and HTML 1.8 MB. This is the whole repo, including ports and tools — [SerenityOS languages API](https://api.github.com/repos/SerenityOS/serenity/languages)
- Top level: Kernel, AK, Userland, Ports, Meta, Documentation, Base, Ladybird, Tests, Toolchain, .github, .devcontainer. Libraries sits under Userland — [SerenityOS repo](https://github.com/SerenityOS/serenity)
- README points to BuildInstructions.md; the default build launches QEMU — [SerenityOS repo](https://github.com/SerenityOS/serenity)

**xv6** (MIT PDOS teaching OS)
- `mit-pdos/xv6-riscv`, default branch `riscv`: pushed 2026-10-26, 10,736 stars. API licence NOASSERTION. The LICENSE file on the `riscv` branch is MIT: "Copyright (c) 2006-2024 Frans Kaashoek, Robert Morris, Russ Cox" — [xv6-riscv GitHub API](https://api.github.com/repos/mit-pdos/xv6-riscv); [xv6-riscv LICENSE](https://github.com/mit-pdos/xv6-riscv/blob/riscv/LICENSE)
- `mit-pdos/xv6-public` (x86): pushed 2024-08-13, 9,717 stars. README: "we have stopped maintaining the x86 version of xv6, and switched our efforts to the RISC-V version" — [xv6-public README](https://github.com/mit-pdos/xv6-public)
- Book: "xv6: a simple, Unix-like teaching operating system", Russ Cox, Frans Kaashoek, Robert Morris, dated September 2, 2025 — [xv6 book rev5 PDF](https://pdos.csail.mit.edu/6.1810/2025/xv6/book-riscv-rev5.pdf)

**MikanOS** (UEFI x86_64 educational OS, the closest teaching comparator)
- Licence Apache-2.0. Pushed 2023-08-22. 761 stars. "Educational Operating System" — [MikanOS GitHub API](https://api.github.com/repos/uchan-nos/mikanos)
- Top-level layout: MikanLoaderPkg (UEFI loader), kernel, apps, docs, tools, resource, release_pack, plus LICENSE, NOTICE, build.sh, run.sh — [MikanOS repo](https://github.com/uchan-nos/mikanos/tree/master)
- Companion book is in Japanese (Mynavi, 2021; ISBN 9784839975869) — [Maruzen listing](https://www.maruzenjunkudo.co.jp/products/9784839975869). Author credit differs between listings.
- A Zenn review gives about 710 pages — [Zenn review](https://zenn.dev/yukisatoblog/articles/orange-book-intro) (lead only)
- Japanese chapter topics from a retailer listing: bootloader, pixel drawing, console, interrupts, memory management, later app files, app memory, Japanese display, inter-app communication — [Rakuten listing](https://books.rakuten.co.jp/rk/c562445399183353952e57a5e1daca05/) (lead only)
- No English edition found.

**xv6 Rust ports** (search leads only; dates come from search listings, not the API)
- `Jaic1/xv6-riscv-rust`: MIT, pushed 2022-05-09 per the API — [Jaic1 GitHub API](https://api.github.com/repos/Jaic1/xv6-riscv-rust). Stale.
- `sjet47/xv6.rs`: last push June 2024 per search listing — [ecosyste.ms listing](https://awesome.ecosyste.ms/projects/github.com%2Fsjet47%2Fxv6.rs)
- `Ko-oK-OS/xv6-rust`: last push January 2023 per search listing — [ecosyste.ms listing](https://awesome.ecosyste.ms/projects/github.com%2FKo-oK-OS%2Fxv6-rust)
- `garentyler/xv6-riscv`: a Rust kernel for RISC-V, with networking and real hardware still unchecked; undated — [garentyler/xv6-riscv](https://github.com/garentyler/xv6-riscv)
- `WCU-EDGE/xv6-rust`: x86 port in progress, forked from connerkuehl's work; undated — [WCU-EDGE/xv6-rust](https://github.com/WCU-EDGE/xv6-rust)
- No 2026-active xv6 Rust port was found. No project called "rv6" was found.

### Inferences
- Redox is the only large, actively maintained Rust OS with a dated monthly report in 2026. Its scheduler and SMP work (EEVDF with per-CPU queues) is the most current example of the scheduling step Aeon would take next.
- MikanOS is the closest architecture match to Aeon (UEFI loader package, kernel, apps split), but it is stale and in Japanese.
- xv6 (RISC-V) is the best small reference for process and filesystem design. Its x86 port is no longer maintained, so cite the RISC-V repo.

### Gaps
- Redox kernel LOC and language breakdown: the kernel is not in the mirror's breakdown. The kernel repo was not identified from the fetched pages.
- The Tock language breakdown is from one response. It includes Tcl and GDB entries that were not checked against the repo, so use it only as an order of magnitude.
- x86_64 crate feature list: not fetched.
- Published 2026 This Month in Rust OSDev text: not retrieved. Only drafts were found.
- MikanOS English chapter list: none found.
- Real LOC for every project: needs a shallow clone and line count, not done.
- Theseus activity after 2024-09-22: not found.

---

## Q2. Features a small OS needs next, and which projects show them

### Takeaway
The features Aeon lacks (per-process address spaces, ELF loading, VFS, device model, networking, POSIX, SMP, tickless scheduling) each appear in at least one project, but only in a few cases can I point to a source. Redox has the most 2026 evidence (per-CPU run queues, work stealing, direct context switching, page-fault signals). xv6 covers the address-space and exec path in its book. Most of the other cells cannot be filled from the fetched sources.

### Cited Findings

Feature matrix. A cell is filled only when a fetched source says it. "Not verified" means not checked, not absent.

| Project | Per-process address space | ELF loading | VFS / schemes | Driver model | Networking | SMP | Other |
|---|---|---|---|---|---|---|---|
| Redox | Memory chapter is a TODO in the book; not verified in code — [Redox book, memory](https://doc.redox-os.org/book/memory.html) | Not verified | Schemes chapter exists — [Redox book, schemes](https://doc.redox-os.org/book/schemes.html) | Drivers chapter exists — [Redox book, drivers](https://doc.redox-os.org/book/drivers.html) | Not verified | Per-CPU queues and work stealing in the EEVDF change — [RSoC 2026 EEVDF](https://www.redox-os.org/news/rsoc-eevdf/) | Round-robin in the book; EEVDF is a TODO there — [Redox book, scheduling](https://doc.redox-os.org/book/scheduling.html). Syscall via `syscall`/`sysretq` from ring 3 — [Redox book, kernel](https://doc.redox-os.org/book/kernel.html) |
| Tock | MPU isolation of apps — [Tock README](https://github.com/tock/tock) | Not applicable (no x86) | Not verified | Not verified | Not verified | Not verified | Cortex-M and RISC-V only |
| Hermit | Kernel is linked into the app as a static library — [Hermit README](https://github.com/hermit-os/kernel) | Not verified | Not verified | Not verified | Not verified | Not verified | `src/` has `fs`, `drivers`, `mm`, `scheduler`, `syscalls`, `executor` directories — [Hermit src](https://github.com/hermit-os/kernel/tree/main/src) (names only) |
| Theseus | Not verified | Not verified | Not verified | Not verified | Not verified | Not verified | Live-evolution goal stated, mechanism not explained — [Theseus README](https://github.com/theseus-os/Theseus) |
| SerenityOS | Not verified | Not verified | Not verified | Not verified | Not verified | Not verified | Kernel directories include Memory, Syscalls, Tasks, FileSystem, Net, Interrupts, Locking, Security — [SerenityOS Kernel dir](https://github.com/SerenityOS/serenity/tree/master/Kernel) (names only) |
| xv6 (RISC-V) | Book chapter 3 "Page tables" — [xv6 book](https://pdos.csail.mit.edu/6.1810/2025/xv6/book-riscv-rev5.pdf) | Book chapter 1.1 "Processes and memory"; `kernel/exec.c` is named in the book text (exec's role in chapter 1 not confirmed) — [xv6 book](https://pdos.csail.mit.edu/6.1810/2025/xv6/book-riscv-rev5.pdf) | Book chapter 10 "File system" — same source | Book chapter 6 "Interrupts and device drivers" — same source | Lecture topic "Networking" — [xv6 lectures](https://pdos.csail.mit.edu/6.1810/2025/schedule.html) | Book chapter 7 "Locking" and chapter 11 "Concurrency revisited" — same source | Book chapter 8 "Scheduling", chapter 9 "Sleep and Wakeup" — same source |
| blog_os | Chapter "Paging Implementation" — [blog_os](https://os.phil-opp.com/paging-implementation/) | Not in book TOC | Not in book TOC | Not in book TOC | Not in book TOC | Not in book TOC | Ends at async/await, before user mode — [blog_os](https://os.phil-opp.com/) |
| MikanOS | Not verified | Japanese TOC lists app-file chapters (lead only) | Not verified | Not verified | Not verified | Not verified | Repo has a UEFI loader package and separate `apps` directory — [MikanOS repo](https://github.com/uchan-nos/mikanos/tree/master) |

Redox 2026 scheduling and kernel items (from the primary monthly reports and RSoC post listed in Q1):
- EEVDF replaces DWRR; per-CPU run queues; work stealing; blocked tasks and timers in a BTreeSet — [RSoC 2026 EEVDF](https://www.redox-os.org/news/rsoc-eevdf/)
- Direct context switching to skip the scheduler on service calls; page-fault signals on x86-64 — [This Month in Redox, Sep 2026](https://www.redox-os.org/news/this-month-260930/)

### Inferences
- For Aeon's next step (ELF loader and per-process address spaces, the HANDOFF recommendation), the xv6 book is the most usable text. Its chapters 1 to 3 cover the same ground, and the book's exercises ask for a COW fork and mmap.
- For per-CPU scheduling and SMP, Redox's EEVDF change is the current example, and the scheduling chapter of the Redox book is still a round-robin description.
- Tock's MPU isolation is a contrast only. Aeon has paging, so Tock offers no pattern to copy.
- Hermit's README describes the kernel as a static library linked into the app. It shows no ring 3 or syscall layer, so it is a weak source for Aeon's syscall ABI decision.

### Gaps
- Per-process address spaces, ELF loading, networking, POSIX, SMP and tickless scheduling were not verified in code for any project except the xv6 book and the Redox news posts.
- Tickless (NO_HZ) scheduling and priority classes: no source fetched.
- TPM measured boot, Secure Boot, signed images and W^X: searched in this pass. No Redox-specific or hobby-OS source was found. The search for Redox returned only generic Linux and UEFI material (for example [Lanzaboote](https://github.com/tfc/lanzaboote), a NixOS Secure Boot and measured-boot tool, which is a tool and not an OS). Result: three generic searches (Redox Secure Boot/TPM, hobby-OS W^X with TPM and UEFI, xv6 Rust ports) found no project-specific source. Theseus, Tock, Hermit, SerenityOS and blog_os were not searched individually for these features. Treat as a Gap, not as proof of absence.

---

## Q3. Source files and book chapters to read for each feature

### Takeaway
The best-verified reading list is the xv6 book, whose chapters name real kernel source files, and the Redox book and news posts. The xv6 file names below come from the book text itself. Other repos were checked at directory level only.

### Cited Findings

xv6 book, chapter list (from the rev5 PDF, text extracted in this pass) — [xv6 book](https://pdos.csail.mit.edu/6.1810/2025/xv6/book-riscv-rev5.pdf):
1. Operating system interfaces (p. 9): 1.1 Processes and memory, 1.2 I/O and file descriptors, 1.3 Pipes, 1.4 File system
2. Operating system organization (p. 21)
3. Page tables (p. 31)
4. Traps and system calls (p. 43)
5. Page faults (p. 51)
6. Interrupts and device drivers (p. 57)
7. Locking (p. 63)
8. Scheduling (p. 75): 8.2 context switch, 8.3 code: context switching, 8.4 code: scheduling, 8.5 code: mycpu and myproc
9. Sleep and wakeup (p. 81): 9.2 code: sleep and wakeup, 9.3 code: pipes, 9.4 code: wait, exit, and kill, 9.5 process locking
10. File system (p. 89)
11. Concurrency revisited (p. 105)
12. Summary (p. 109)

Kernel source files cited in the xv6 book text: `kernel/vm.c` (5 mentions), `kernel/trap.c`, `kernel/proc.c`, `kernel/proc.h`, `kernel/syscall.c`, `kernel/sysproc.c`, `kernel/sysfile.c`, `kernel/exec.c`, `kernel/start.c`, `kernel/main.c`, `kernel/kalloc.c`, `kernel/pipe.c`, `kernel/spinlock.c`, `kernel/spinlock.h`, and user programs `user/init.c`, `user/sh.c` — [xv6 book](https://pdos.csail.mit.edu/6.1810/2025/xv6/book-riscv-rev5.pdf). These are the file names the book cites; they are the primary-source paths for the xv6 column in Q2.

Redox book (fetched) — [Redox book](https://doc.redox-os.org/book/):
- 2.1 Microkernels, 2.2 Boot process, 2.3 Redox kernel (kernel.html), 2.4 System services in user space, 2.6 Memory management (TODO in text), 2.7 Scheduling (round-robin, a 10-tick timer switch, EEVDF marked TODO), 2.8 Drivers, 2.9 RedoxFS, 3.3 Schemes, 3.4 "Everything is a File"
- The Redox kernel page names only the `redox_syscall` docs — [docs.rs redox_syscall](https://docs.rs/redox_syscall/latest/syscall/)
- No source file paths named in the book pages.

Repository directory listings (names only, not read):
- Hermit `src/`: acpi, arch, console, drivers, env, executor, fd, fs, mm, scheduler, synch, syscalls, plus common_os.rs, config.rs, rt.rs, shell.rs, uhyve.rs and others — [Hermit src](https://github.com/hermit-os/kernel/tree/main/src)
- SerenityOS `Kernel/`: API, Arch, Boot, Bus, Devices, EFIPrekernel, FileSystem, Firmware, Heap, Interrupts, Library, Locking, Memory, Net, Prekernel, Security, Syscalls, Tasks, Time — [SerenityOS Kernel](https://github.com/SerenityOS/serenity/tree/master/Kernel)
- MikanOS top level: MikanLoaderPkg, kernel, apps — [MikanOS](https://github.com/uchan-nos/mikanos/tree/master). Subfolders of the kernel and loader were not listed.
- blog_os chapter list (book, 12 chapters) — [blog_os](https://os.phil-opp.com/)

### Inferences
- Paging and address spaces: blog_os "Paging Implementation" for the mechanism, then xv6 chapter 3 and `kernel/vm.c` for the user-space side.
- Exec and loading: `kernel/exec.c` in xv6 (named in the book) is the file to read for a loader. MikanOS's `MikanLoaderPkg` is the closest UEFI analogue to Aeon's boot stage.
- Scheduling: xv6 chapter 8 and `kernel/proc.c` for the basic loop; Redox's EEVDF change (`select_next_context`) for per-CPU queues.
- Drivers and interrupts: xv6 chapter 6 and `kernel/spinlock.c` for the locking behind device code.
- Filesystem: xv6 chapter 10 for the layout Aeon's FAT reader would be compared against.

### Gaps
- Hermit, SerenityOS and MikanOS: only directory names were read. The file paths for the features above were not opened and should be checked before citing.
- Redox kernel source paths: not named in the book pages. The kernel repo was not identified (see Q1 Gaps).
- The xv6 book's chapter 1 title and the xv6 column in Q2 are from extracted text and not checked against the rendered PDF.

---

## Q4. Lesson list: practices worth copying

### Takeaway
The clearest practices in the fetched sources are a repo-root config set (Hermit), a book kept with the project and a best-practices chapter (Redox), a named build-instructions file and a Documentation directory (SerenityOS), a dated status page (blog_os), and an explicit "stopped maintaining" note (xv6).

### Cited Findings
- Hermit keeps deny.toml, clippy.toml, rustfmt.toml, typos.toml, rust-toolchain.toml, CITATION.cff, CONTRIBUTING.md at the repo root — [Hermit repo](https://github.com/hermit-os/kernel)
- Redox Book chapter 7 covers literate programming, style, "Rusting properly", "Avoiding Panics" and testing; chapter 8 covers the GitLab workflow and bug reports — [Redox book](https://doc.redox-os.org/book/)
- Redox publishes monthly reports and RSoC write-ups with named authors and component names — [Redox news](https://www.redox-os.org/news/)
- Theseus publishes rustdoc for crates and a Markdown design book — [Theseus README](https://github.com/theseus-os/Theseus)
- SerenityOS keeps BuildInstructions.md and a top-level Documentation directory — [SerenityOS repo](https://github.com/SerenityOS/serenity)
- blog_os has a dated status-update page — [blog_os status updates](https://os.phil-opp.com/status-update/)
- xv6 README states the x86 version is no longer maintained — [xv6-public README](https://github.com/mit-pdos/xv6-public)

### Inferences
- Aeon already has `tests/run.sh` and a release script. The missing pieces are a supported-targets statement and a dated status page, which blog_os and Redox both show.
- A "Not built yet" list (already in HANDOFF) follows the xv6 practice of saying what is unmaintained.
- Hermit's deny.toml and typos.toml are cheap checks that run without QEMU.

### Gaps
- Release process (tagging, changelog, signing): checked only in Redox's contributing chapters.
- Test practice in Tock, Theseus, SerenityOS: not fetched.

---

## Q5. Textbook coverage that maps to the missing features

### Takeaway
OSTEP (free online, version 1.10, November 2023) has a chapter for each missing feature, and the xv6 book covers the same material in code. Tanenbaum is only partly verified and is marked as a Gap.

### Cited Findings

OSTEP (Operating Systems: Three Easy Pieces, v1.10, November 2023) — [OSTEP](https://pages.cs.wisc.edu/~remzi/OSTEP/):
- Virtualization: 4 Processes, 5 Process API, 6 Direct Execution, 7 CPU Scheduling, 8 Multi-level Feedback, 9 Lottery Scheduling, 10 Multi-CPU Scheduling
- Memory: 13 Address Spaces, 14 Memory API, 15 Address Translation, 16 Segmentation, 17 Free Space Management, 18 Introduction to Paging, 19 TLBs, 20 Advanced Page Tables, 21 to 23 Swapping and complete VM systems
- Concurrency: 26 Concurrency and Threads, 27 Thread API, 28 Locks, 29 Locked Data Structures, 30 Condition Variables, 31 Semaphores, 32 Concurrency Bugs, 33 Event-based Concurrency
- Persistence: 36 I/O Devices, 37 Hard Disk Drives, 39 Files and Directories, 40 File System Implementation, 42 FSCK and Journaling, 43 LFS
- Security: 53 Intro Security, 54 Authentication, 55 Access Control, 56 Cryptography. These are general security topics; OSTEP does not cover UEFI, Secure Boot or TPM.

Tanenbaum, Modern Operating Systems (4th edition, Global Edition), partial:
- Chapter 1 Introduction, 2 Processes and Threads, 3 Memory Management, 4 File Systems — [Pearson listing](https://www.pearson.com/uk/educators/higher-education-educators/program/Tanenbaum-Modern-Operating-Systems-Global-Edition-4th-Edition/PGM1081000.html)
- Chapter 6 Deadlocks, and chapter 5 covering I/O — [Lehmanns excerpt](https://www.lehmanns.ch/media/113164027)
- Chapter 7 begins with Virtualization — [Pearson listing](https://www.pearson.com/uk/educators/higher-education-educators/program/Tanenbaum-Modern-Operating-Systems-Global-Edition-4th-Edition/PGM1081000.html)

xv6 book chapters that map to the Aeon gaps (from Q3):
- Per-process address space and exec: chapter 3 Page tables, `kernel/exec.c` (book names it)
- Syscall ABI: chapter 4 Traps and system calls
- Devices and IRQs: chapter 6 Interrupts and device drivers
- SMP and locking: chapter 7 Locking, chapter 11 Concurrency revisited
- Scheduling: chapter 8 Scheduling, chapter 9 Sleep and wakeup
- Filesystem: chapter 10 File system

Mapping table for a course project:

| Missing Aeon feature | OSTEP chapter(s) | xv6 book chapter |
|---|---|---|
| Per-process address space, ELF loading | 13, 14, 15 | 3 Page tables; 1.1 Processes and memory (exec not confirmed in TOC) |
| Ring 3 and syscall ABI | 6 Direct Execution | 4 Traps and system calls |
| Paging and TLB | 18, 19, 20 | 3, 5 Page faults |
| VFS and filesystem | 39, 40 | 10 File system |
| IOAPIC and device interrupts | 36 | 6 Interrupts and device drivers |
| SMP and locks | 10, 28 | 7 Locking, 11 Concurrency revisited |
| Scheduler policy (built; comparison) | 7, 8, 9 | 8 Scheduling |
| Security: Secure Boot, TPM, W^X | 53 to 56 (general background only) | none |

### Inferences
- The OSTEP and xv6 chapter numbers are verified against fetched TOCs.
- Aeon's scheduler-policy module (`src/schedpolicy.rs` per HANDOFF) can cite OSTEP chapters 7 to 9.
- Aeon's Banker's module can cite Tanenbaum chapter 6 (verified via the Lehmanns excerpt). The OSTEP deadlock chapter was not in the fetched TOC, so do not cite an OSTEP number for it.
- For Secure Boot and TPM, no textbook in this pass covers them. Cite the UEFI and TCG specs directly once they are fetched.

### Gaps
- Tanenbaum chapters 8 to 12: no verified TOC URL. Do not cite these numbers without a publisher TOC.
- OSTEP deadlock chapter: not in the fetched TOC.
- Secure Boot, TPM and W^X textbook coverage: none found.

---

## Open items for a follow-up pass
1. Fetch the Hermit, SerenityOS and MikanOS subdirectories that hold the features above, and record file paths.
2. Identify the Redox kernel repo and its language breakdown.
3. Fetch the x86_64 crate's module list.
4. Check the published July 2026 This Month in Rust OSDev post.
5. Confirm Tock's MIT licence file.
6. Run a shallow clone of the repos for real LOC counts.
