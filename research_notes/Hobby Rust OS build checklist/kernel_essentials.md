# Kernel Essentials for a Hobby x86_64 Rust OS (UEFI, no_std)

Research date: 2026-11-03. Status: partial. Coverage is strong for interrupts (APIC/MADT/SMP) and tickless; weak for Redox, Theseus and xv6, and sizes are thin. The research budget (about 15 calls) was exceeded to fetch the osdev.wiki mirror and to make these corrections.

Labels: [Fetched] = read in a WebFetch page (quoted text is the fetch tool's output and was not checked against raw HTML). [Summary] = a fetch-tool summary of a file (line numbers and counts are approximate, not checked with wc). [Snippet] = search-result text only, not the page itself. [Opinion] = judgment. [Inference] = my reasoning. [Unverified] = from memory, not checked.

Note on hosts: wiki.osdev.org returns HTTP 403 to WebFetch. The same OSDev pages were fetched from https://osdev.wiki/wiki/... instead.

---

## Q1. Recommended build order and pitfalls

### Takeaway
The one sourced ordering is blog_os's tutorial sequence: exceptions, double faults, hardware interrupts, paging, heap, allocators, then async multitasking. The full UEFI spine below is my hypothesis, not a sourced consensus. Whether to follow it is an opinion.

### Build order (hypothesis, verify against your design)
- Inference: UEFI handoff (memory map, RSDP, framebuffer) -> serial logging -> GDT/TSS (IST stacks) and IDT with exception handlers -> physical frame allocator -> own page tables and kernel heap -> ACPI tables (MADT, FADT, HPET, MCFG) -> mask legacy PIC, enable LAPIC and IOAPIC, calibrate a timer -> kernel-thread scheduler -> syscall entry -> per-process PML4 with CR3 switching -> ELF loader and first ring-3 entry -> SMP last.
- Dependency notes (Inference):
  - A heap needs mapped pages, so the frame allocator comes before it.
  - IST stacks are needed for double-fault handling.
  - MSI depends on PCI enumeration (MCFG/ECAM config space) plus a working LAPIC.
  - The xHCI keyboard path depends on PCI, interrupts, and MSI.
  - SMP needs the LAPIC and MADT per-CPU entries (see Q4).

### Cited Findings
- blog_os second edition orders its 12 posts as: Bare Bones (freestanding binary, minimal kernel, VGA text mode, testing), Interrupts (CPU exceptions, double faults, hardware interrupts), Memory Management (intro to paging, paging implementation, heap allocation, allocator designs), Multitasking (async/await). [Fetched] — https://os.phil-opp.com/
- The second edition is current. The first edition used GRUB and is no longer updated. [Fetched] — https://os.phil-opp.com/
- The blog_os series has no user mode, ELF loading or system calls. [Fetched] — https://os.phil-opp.com/
- Nothing on that page states a Rust nightly/toolchain policy. [Fetched] — https://os.phil-opp.com/
- Linux NO_HZ docs: NO_HZ_IDLE is "the common default," and NO_HZ_FULL "is normally -not- wanted" unless running real-time or HPC workloads. [Fetched] — https://www.kernel.org/doc/html/latest/timers/no_hz.html
- Linux NO_HZ docs also list costs: dyntick-idle adds instructions on idle entry and exit, user/kernel transitions get slightly more expensive in adaptive-tick mode, and POSIX CPU timers prevent adaptive-tick mode. [Fetched] — https://www.kernel.org/doc/html/latest/timers/no_hz.html
- Legacy PIC is fully masked once APIC is active. [Snippet — NyxOS wiki https://github.com/nyxos-dev/nyx-os/wiki/Drivers]

### Inferences
- blog_os's paging-before-heap order matches the dependency above.
- [Opinion] A reasonable default is this spine, with a single-core scheduler before SMP. This is a judgment, not a sourced recommendation.

### Pitfalls
- Enabling the APIC without masking the legacy PIC leaves the PIC able to deliver spurious interrupts. The osdev.wiki APIC page says the PIC should be masked and its IRQs remapped to vectors starting at 32. Per the fetch summary, it also recommends keeping those vectors reserved, since a masked PIC can still emit spurious interrupts (paraphrase, not verbatim). [Fetched/Summary] — https://osdev.wiki/wiki/APIC
- Sysret with a non-canonical RCX is a known privilege-escalation class (CVE-2012-0217). [Unverified — from memory; confirm before citing.]
- Rust-specific items (stability of `extern "x86-interrupt"`, naked-function status for syscall stubs) were not checked against 2026 sources.

### Gaps
- The osdev.wiki SYSENTER and Paging pages were not fetched, so SYSCALL/SYSRET setup details and paging specifics are unverified.
- No source found for a canonical order for a UEFI kernel. The spine is a hypothesis.
- Recency: the blog_os status page (fetched) says its status list runs through September 2026. A search summary said the newest "This Month in Rust OSDev" issue is July 2026, and later issues were not found. These two sources conflict. The newsletter index is https://os.phil-opp.com/status-update/. Not resolved.

---

## Q2. How hobby kernels implement per-process address spaces and the ELF loader

### Takeaway
Only one implementation was read, and it is a summary of one file. blog_os has no per-process address spaces or ELF loader. Hermit is a unikernel with no user-space processes in its README. Redox, Theseus and xv6 could not be read usefully.

### Cited Findings
- SerenityOS `Kernel/Memory/AddressSpace.cpp`: about 425 lines (approximate, from a fetch summary). [Summary] — https://github.com/SerenityOS/serenity/blob/master/Kernel/Memory/AddressSpace.cpp
- The file holds an `m_page_directory` of type `PageDirectory`, creates it via `PageDirectory::try_create_for_userspace(process)`, and delegates PTE writes to `Region::map()`, `Region::unmap()` and `set_page_directory()`. [Summary] — same URL
- `try_create(Process&, AddressSpace const* parent)` reuses the parent's `total_range()` but does not copy regions or page tables, so fork-style duplication must be implemented elsewhere. [Summary] — same URL
- The address space covers only the user range (`USER_RANGE_BASE` to `USER_RANGE_CEILING`). `unmap_mmap_range()` rejects other addresses with `is_user_range()` and returns `EFAULT`. [Summary] — same URL
- Hermit describes itself as "A Rust-based, lightweight unikernel" and says "Usually the kernel will be linked as static library to your applications." The README names x86_64, aarch64 and riscv64 targets. It does not mention per-process page tables or ELF loading. [Fetched] — https://github.com/hermit-os/kernel
- blog_os has no user mode, ELF loader or syscalls. [Fetched] — https://os.phil-opp.com/

### Inferences
- The SerenityOS split (address-space object owns regions, region code writes PTEs) is a pattern a hobby kernel can copy. The ~425 lines covers only this file, not the full memory subsystem.
- A minimal per-process address space needs: a PML4 per process, a kernel half shared across all PML4s, and a user half owned by the process. [Inference]

### Gaps
- Redox: the GitLab repo page returned only a tagline and repo stats, not the README or source tree. Where paging, context and exec live is unverified. [Fetched page had no structure — https://gitlab.redox-os.org/redox-os/kernel]
- Theseus: the book redirected to https://www.theseus-os.com/Theseus/book/, which returned only a table of contents, not the address-space sections. Single-address-space design is unverified. [Unverified — from memory]
- xv6: the PDF fetch returned binary data. The RISC-V xv6 book's exec and address-space chapters are unverified. The maintained xv6 is RISC-V, and the x86 xv6-public is legacy 32-bit. [Unverified — from memory]
- Hermit is confirmed as a unikernel. Whether it has per-process page tables is not stated in its README.
- ELF-loader size: no source found for any of these kernels.

---

## Q3. IOAPIC, IRQ1 keyboard, ACPI MADT/FADT, PS/2 emulation, USB-only machines

### Takeaway
On a standard PC, IRQ1 goes to IOAPIC redirection entry 1, but the MADT can override legacy routing, so check the override table first. The MADT layout and LAPIC enable steps are now sourced from osdev.wiki. PS/2 emulation on a USB-only PC is firmware behaviour. An OS that wants the keyboard without BIOS help needs an xHCI driver and the xHCI BIOS-to-OS ownership handoff.

### Cited Findings
- MADT header: signature 'APIC', a Local APIC Address field at offset 0x24, and a Flags field at offset 0x28 where bit 0 means dual 8259 PICs are installed and should be masked. Records start at offset 0x2C. [Fetched] — https://osdev.wiki/wiki/MADT
- MADT record types: Type 0 Processor Local APIC (APIC ID, flags, bit 0 enabled, bit 1 online-capable); Type 1 I/O APIC (ID, address, GSI base); Type 2 Interrupt Source Override (bus source, IRQ source, GSI, flags). [Fetched] — https://osdev.wiki/wiki/MADT
- Example from the MADT page: the PIT is IRQ 0 but is usually wired to GSI 2. Route it by finding the I/O APIC whose base is at or below that GSI, then programming entry (GSI minus base). Without an override, ISA defaults to edge-triggered, active high. [Fetched] — https://osdev.wiki/wiki/MADT
- LAPIC enable: bit 11 of IA32_APIC_BASE enables the APIC; Spurious Interrupt Vector Register at offset 0xF0 needs its bit 8 (0x100) set to enable the LAPIC to receive interrupts; EOI is written at offset 0xB0 with 0, and a non-zero value may cause a general protection fault. [Fetched] — https://osdev.wiki/wiki/APIC
- IRQ1 maps to IOAPIC redirection entry 1. [Snippet — https://osdev.wiki/wiki/Special:MobileDiff/18063, a diff view, may be out of date]
- Redirection entry offsets for IRQ n are 0x10 + n*2 (low 32 bits at the first offset, +1 for high). [Snippet — https://forums.osdev.org/IOAPIC]
- The PIT arrives on IOAPIC pin 2 under QEMU because of an ACPI interrupt-source override, so IRQ1 may not be identity-mapped. [Snippet — NyxOS wiki https://github.com/nyxos-dev/nyx-os/wiki/Drivers]
- Search summary (attribution to this URL not confirmed): an OS write-up routes timer and keyboard to IDT vectors 0x20 and 0x21, targets the BSP's LAPIC ID, and masks all redirection entries before programming them. [Snippet — https://cmsd2.github.io/ostoo/apic-ioapic.html]
- Motherboard manual (search summary): "XHCI Hand-off" is a workaround for OSes lacking xHCI ownership support, and the xHCI driver should claim ownership when present. [Snippet — https://www.manualowl.com/m/ASRock/H310CM-HDVP/Manual/524156?page=64]
- Motherboard manuals: Legacy USB support and Port 60/64 emulation are the BIOS settings that make a USB keyboard work before the OS takes over. [Snippet — manual search results; exact URL for this sentence not confirmed]
- Linux list participant (opinion, not a spec): the BIOS historically translated USB keyboard and mouse into PS/2 events. [Snippet — https://lkml.iu.edu/hypermail/linux/kernel/2105.0/05088.html]
- iPXE: releasing xHCI ownership back to the BIOS is a rarely used path, since most OSes claim ownership and never return it. [Snippet — https://ognproject.evlt.uma.es/gitea/narenas/opengnsys_ipxe/commit/ec0e2a7bd77c3021bb492ccb78855482312bca4b; attribution to this URL not confirmed]

### Inferences
- "PS/2 emulation" on a USB-only PC is firmware behaviour (legacy USB support / SMM). The OS-side work is: detect the xHCI, perform the BIOS-to-OS ownership handoff through the xHCI legacy-support capability, run a HID boot-protocol keyboard driver, and route its interrupt through MSI or IOAPIC.
- A kernel that skips the handoff may still work when firmware has already released ownership, but this is not reliable.
- Keyboard interrupts on a real PC depend on the MADT override check (for PS/2 IRQ1 routing) and on MSI (for xHCI). Both depend on the APIC step.

### Pitfalls
- Trusting the BIOS PS/2 path after ExitBootServices is not guaranteed on every board; check the MADT and xHCI state instead. [Inference]
- Hand-off timing and vendor option names vary. [Snippet]

### Gaps
- FADT fields were not read. The question about whether the FADT's IA-PC boot-architecture flags report an 8042 controller remains open. The ACPI spec's FADT section is the place to check. Not fetched.
- The xHCI legacy-support capability register layout and handoff sequence were not read from the xHCI spec or the Linux `quirk_usb_handoff_xhci` code. Not fetched.
- HID boot protocol details (SET_PROTOCOL, report layout) were not sourced.
- Whether a given modern board still emulates PS/2 for USB keyboards was not verified.

---

## Q4. SMP bring-up and whether it is worth doing

### Takeaway
The osdev.wiki SMP page gives the INIT-SIPI-SIPI sequence, the trampoline constraint, and MADT-based AP enumeration. It says nothing about GS base or per-CPU data, so those remain unsourced. Whether SMP is worth doing is opinion.

### Cited Findings
- Sequence: BSP sends INIT, waits, then sends SIPI. The example code waits 10 ms after INIT and 200 µs after each STARTUP IPI. The page prefers "just send the two SIPI and make the APs to wait for the BSP." For newer CPUs, "one SIPI is enough," and Intel's generic algorithm uses 200 µs between SIPIs. [Fetched] — https://osdev.wiki/wiki/Symmetric_Multiprocessing
- Broadcast INIT and SIPI are not allowed: "You must not do Broadcast INIT IPIs nor Broadcast SIPIs." [Fetched] — same URL
- Trampoline: "Copy the AP trampoline into low RAM (since real mode can't address more than 20 bits at startup)." Startup vectors are "limited to a 4KB boundary within the first 1MB of space." The example uses 0x8000. [Fetched] — same URL
- AP enumeration: the MADT lists local APICs, and the count should match the number of cores. The BSP's own APIC ID comes from CPUID leaf 1 and is skipped. [Fetched] — same URL
- Per-AP stack: "Set up 32k stack, one for each core. It is important that all core must have its own stack." [Fetched] — same URL

### Inferences
- SMP requires the LAPIC and MADT (Q3), so it comes after the interrupt step.
- SMP forces per-CPU data and synchronized page-table updates (TLB shootdown IPIs), which interact with the scheduler and address spaces. [Inference]
- [Opinion] SMP is worth doing only after a single-core scheduler and address spaces work. Many hobby authors defer it; this is a judgment, not sourced.

### Gaps
- GS-base per-CPU data and TLB-shootdown details: not in the fetched page, and not sourced.
- Time cost of SMP bring-up: no credible source found.

---

## Q5. Rough sizes (lines of code, days of work)

### Takeaway
One size figure: SerenityOS's AddressSpace.cpp is about 425 lines. No credible days-of-work source was found. Other LOC figures were not measured.

### Cited Findings
- SerenityOS `Kernel/Memory/AddressSpace.cpp`: about 425 lines, approximate, from a fetch summary. [Summary] — https://github.com/SerenityOS/serenity/blob/master/Kernel/Memory/AddressSpace.cpp
- blog_os second edition: 12 posts across four sections. A proxy for scope only. [Fetched] — https://os.phil-opp.com/

### Inferences
- The 425-line figure covers one file in a larger subsystem, so it undercounts the whole memory subsystem.

### Gaps
- No LOC counts for the frame allocator, paging, IDT/GDT, APIC, scheduler, syscall, ELF loader or SMP. Measuring requires a shallow clone of each repo and counting specific files with wc. Not done.
- No credible days-of-work source. None is given, to avoid inventing one.
