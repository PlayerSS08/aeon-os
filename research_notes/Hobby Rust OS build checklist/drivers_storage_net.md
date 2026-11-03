# Drivers, storage and networking for Aeon (hobby no_std Rust x86_64 OS)

Research date: 2026-11-03. Sources are cited inline. Sizes marked "(estimate)" are my own judgement, not sourced. Where a source could not be fetched, that is said in the Gaps section.

Project context used for the recommendation comes from `G:\Aeon\OS\docs\HANDOFF.md` and the code (`src/vblk.rs`, `src/fat.rs`, `src/shell.rs`, `src/main.rs`, `src/uefi.rs`, `Cargo.toml`). Current state: UEFI boot stage with signed image; PCI scan; virtio-blk (read); read-only FAT16/32; RAM filesystem; shell. Keyboard in the kernel shell is read from PS/2 port 0x60 (`src/shell.rs` line 63-70). The boot menu reads UEFI `ConIn` (`src/main.rs` line 631-637) before ExitBootServices. The project has a zero-dependency rule; the owner has not yet decided whether crates are allowed (HANDOFF decision 2).

## Key Question 1: xHCI USB host controller and a USB HID keyboard

### Takeaway
The minimum is a full xHCI bring-up plus a USB enumeration stack plus an HID boot-protocol keyboard, so this is the largest of the driver tasks, and the "minimal" version still needs several thousand lines (estimate). A Rust crate for the register layer exists, but nothing I found provides enumeration or HID for a no_std kernel. The UEFI route only helps before ExitBootServices.

### Cited Findings
- The `xhci` crate (rust-osdev) is `#![no_std]` compatible, version 0.9.2, last release 2023-07-19, dual Apache-2.0/MIT, and it provides types for xHCI registers and contexts. Its README says it is aimed at OS developers writing a USB device driver. Usage needs `unsafe` construction of `Registers::new(MMIO_BASE, mapper)`. — [docs.rs xhci](https://docs.rs/crate/xhci/latest); source repo listed at [github rust-osdev/xhci](https://github.com/rust-osdev/xhci) (seen in search results, not fetched).
- The `xhci` crate's last release is more than three years old as of 2026-11-03, so check whether it still builds on the pinned toolchain (rust-version 1.85 in `Cargo.toml`). Its docs list no MSRV. — [docs.rs xhci](https://docs.rs/crate/xhci/latest)
- Intel's "Extensible Host Controller Interface for USB Requirements Specification R2.0" is listed by Intel as the xHCI requirements document. I could not fetch its body, so I cannot confirm its contents. — [Intel](https://www.intel.com/content/www/us/en/content-details/868296/extensible-host-controller-interface-for-universal-serial-bus-xhci-requirements-specification-r2-0.html)
- Real-hardware keyboards are USB-first. The project's current keyboard path is PS/2 port 0x60 in `src/shell.rs`. The project's own handoff lists "USB (xHCI) keyboard and mouse" as not built. (Project file, not a web source.)
- Interrupt transfers are needed for keyboard input on xHCI; this is discussed on the SLOF and U-Boot mailing-list threads. These are secondary sources: [SLOF thread](https://lists.ozlabs.org/pipermail/slof/2015-September/000184.html), [U-Boot thread](https://lists.denx.de/pipermail/u-boot/2020-May/411374.html). Not verified against the spec.
- Sample hobby-OS implementations found in search: a handmade.network post describes a 64-bit hobby OS with xHCI keyboard input ([handmade.network](https://handmade.network/snippet/3209)). Not reviewed in depth.

### Inferences
- The xHCI driver path for a keyboard is: find controller (PCI class 0x0C, subclass 0x03, prog-if 0x30 — from the PCI class table, not yet fetched, mark as a memory-based value to verify); map BAR0; reset; set up DCBAA, command ring, event ring, scratchpad buffers; enable ports; enumerate device (Enable Slot, Address Device, GET_DESCRIPTOR, SET_CONFIGURATION); set HID boot protocol; poll or service interrupt endpoint. (Estimate: about 2000-3000 lines including USB descriptor parsing and HID parsing, excluding debugging time.)
- Because `xhci` covers only the register and context layer, the enumeration and HID parts would still be written by hand under the zero-dependency rule.
- An alternative is UEFI SimpleTextInput (`ConIn`, already used by the boot menu in this project, `src/uefi.rs` line 137). It works only before ExitBootServices. After exit the kernel gets no keyboard from UEFI. It does not replace an in-kernel driver, but it is the lowest-effort way to get keys in the boot menu on machines where it works. Its contract is in the UEFI spec, which I did not fetch.

### Gaps
- MikanOS: I did not find its xHCI or keyboard chapter. The search returned no MikanOS page. Not confirmed.
- Redox and Theseus driver sources: not checked this session.
- Intel xHCI spec body: not fetched.
- Whether `xhci` 0.9.2 compiles on Rust 1.85 or current stable: not tested.
- Real-world xHCI quirks across chipsets: not researched.

## Key Question 2: AHCI (SATA) and NVMe drivers

### Takeaway
AHCI is the simpler first target. The read path is a fixed sequence of register writes and command table setup, and the OSDev wiki gives it step by step. NVMe needs admin and I/O queues before any read, and its wiki page is incomplete. Both are reachable under QEMU (estimate), which makes them testable in this project's existing harness.

### Cited Findings
- PCI class codes (from the OSDev wiki PCI table): class 0x01 (Mass Storage), subclass 0x06 (Serial ATA), prog-if 0x01 (AHCI 1.0); subclass 0x08 (Non-Volatile Memory), prog-if 0x02 (NVM Express). Caveat: the wiki table is flattened in extraction, so the grouping was inferred from order. — [osdev.wiki PCI](https://osdev.wiki/wiki/PCI)
- So the user's `lspci` rule is confirmed: class 01:06:01 means AHCI, 01:08:02 means NVMe. Note that a PC can have an AHCI controller in a different prog-if (for example, 01:06:00 is vendor-specific); check prog-if too, not only the subclass.
- AHCI read steps, from the OSDev wiki: map ABAR (BAR5) uncacheable; set GHC.AE (bit 31); scan ports with PI and check PxSSTS.det = 3 and PxSIG = 0x00000101 for a SATA disk; stop the port (clear PxCMD.ST and FRE; wait for FR and CR to clear); set PxCLB/PxFB (command list 1 KB aligned, received-FIS 256 B aligned); set command header and command table (128 B aligned); restart the port (set FRE then ST); issue READ DMA EXT with FIS type 0x27, command bit set, LBA bytes, sector count; poll CI or use interrupts (PxIS, PxIE). — [osdev.wiki AHCI](https://osdev.wiki/wiki/AHCI)
- The same AHCI page says its example has bugs: no timeout in the completion loop, `cmdslots` undefined, and 64-bit pointers cast to 32-bit (works only below 4 GB). These are caveats for copying its code, not for the design. — [osdev.wiki AHCI](https://osdev.wiki/wiki/AHCI)
- The wiki says its authors found little documentation on AHCI tricks and point to the Intel AHCI 1.3.1 spec and the Linux source as primary references. The Intel spec was not fetched, so no URL is given for it here. — [osdev.wiki AHCI](https://osdev.wiki/wiki/AHCI)
- NVMe minimal path, from the OSDev wiki: map BAR0 (64-bit, BAR1:BAR0); enable memory space and bus mastering; read CAP and VS; reset and configure CC; set AQA, ASQ, ACQ; Identify controller (opcode 0x06, CNS 1) and namespace (CNS 0); Create I/O CQ (opcode 0x05) and SQ (opcode 0x01); NVM Read (0x02) and Write (0x01) with PRP1/PRP2 and zero-based block count in DW12. The wiki says its queue code is non-standard and its PRP2 = 0 example only works for transfers up to two pages. — [osdev.wiki NVMe](https://osdev.wiki/wiki/NVMe)
- The NVMe spec: Base Specification 2.0e, ratified 2024-07-29, is a free PDF. Newer 2.x base revisions are announced; the 2.3 Base Spec is named in a press release, but I did not confirm its download link. — [NVMe 2.0e PDF](https://nvmexpress.org/wp-content/uploads/NVM-Express-Base-Specification-2.0e-2024.07.29-Ratified.pdf); [NVMe press release (2025)](https://www.businesswire.com/news/home/20250805262591/en/)

### Inferences
- Minimum AHCI read (estimate): about 500-900 lines including PCI bind, port init, FIS builder, one command slot, and polling. Interrupts can come later.
- Minimum NVMe read (estimate): about 600-1000 lines including admin queue, one I/O queue pair, Identify, and polling. PRP lists are needed for transfers over two pages (the wiki's example skips this).
- QEMU has `-device ahci` and `-device nvme` (QEMU device names, not checked this session). This means both can be tested in the project's existing QEMU harness without real hardware.
- For Aeon, the PCI scan already exists (`src/pci.rs`), so discovery by class code is a small addition.

### Gaps
- Crates for AHCI or NVMe in no_std Rust: not searched this session. Redox and Theseus driver sources not checked.
- Intel AHCI 1.3.1 and NVMe 2.x PDFs: not fetched, so register field names above come from the OSDev wiki only. Check against the spec before coding.
- DMA and cache coherence details for real hardware: not researched.

## Key Question 3: Filesystems with write support (FAT32, ext2, journalling, crash safety)

### Takeaway
FAT32 write support is the easiest step from the current read-only FAT code, and the `fatfs` crate already provides it for no_std use if the owner allows crates. ext2 is simpler than ext4 but needs bitmap and inode updates for every create. Journalling is a separate project; without it a crash mid-write can corrupt the volume, and the notes below cannot give a sourced size for that.

### Cited Findings
- `fatfs` (rafalh/rust-fatfs): read/write, directories, FAT12/16/32. For no_std, disable default features. LFN support is an opt-in feature that uses `alloc`. The docs say these features were designed mainly for no_std use. — [rust-fatfs](https://github.com/rafalh/rust-fatfs) (search snippet; main README not fetched)
- An older fork's README says no_std builds needed a nightly compiler for `core_io`. This looks outdated; check current docs before relying on it. — [fork README](https://git.m-labs.hk/srenblad/rust-fatfs/raw/commit/2d689a668d0953e24047128691c26d6468b3605d/README.md) (search result)
- `simple-fatfs` 0.1.0-alpha.2: embedded-io based, storage needs Read and Seek, Write is optional. Alpha quality. — [simple-fatfs source](https://docs.rs/crate/simple-fatfs/latest/source/src/lib.rs)
- `ext4-view`: read-only by design; write is out of scope. Needs `alloc`. Takes any `Ext4Read` source. Recent versions also read ext2. Latest release seen: 0.9.3. Uses no `unsafe` (its README says so). — [ext4-view 0.4.2 README](https://docs.rs/crate/ext4-view/0.4.2/source/README.md); [ext4-view latest](https://docs.rs/crate/ext4-view/latest)
- ext2 write-path essentials (superblock, group descriptors, inode and block bitmaps, inode table) are described on the OSDev Ext2 page, but the fetch was blocked with HTTP 403. Not verified.
- The FAT spec (Microsoft) and journalling references were not fetched this session.

### Inferences
- FAT32 write (estimate): 800-1500 lines for cluster allocation, FAT table update (both copies), directory entry creation, 8.3 names first, then LFN. Crash safety: every write touches the FAT and the directory entry, so a power loss can leave a lost chain. Mitigate by writing data before updating the FAT and by a `chkdsk`-style check on boot (estimate of the effort).
- ext2 write (estimate): 1500-2500 lines for bitmaps, inode allocation, indirect blocks, and directory growth. ext2 has no journal, so the same crash risk applies. ext2 is a good read target because the existing `ext4-view` crate reads it (if crates are allowed).
- Journalling (ext3/ext4-style) is a large separate project (estimate: several thousand lines); the notes do not give a sourced figure.
- Practical order: FAT32 write before ext2, because the OS already reads FAT and the AEON USB stick is FAT32.

### Gaps
- No sourced figures for FAT write or ext2 write effort.
- The ext2 OSDev page and the Microsoft FAT spec were not read.
- Crash-safety test strategy (power-cut simulation under QEMU): not researched.

## Key Question 4: Networking, smallest useful stack

### Takeaway
The smallest useful stack is a virtio-net driver (QEMU-friendly) plus `smoltcp` for TCP/IP. `smoltcp` is designed for no_std, heap-free use, so the driver is the main work. A physical NIC (e1000, rtl8139, or RTL8168/8169 on real laptops) is a separate driver with its own effort. No real-hardware NIC driver was researched this session.

### Cited Findings
- `smoltcp` 0.13.1 (latest seen on docs.rs): crate root is `no_std` unless `test` or `std` is on; described as a TCP/IP stack for bare-metal, real-time systems without a heap. At least one of `proto-ipv4`, `proto-ipv6`, or `proto-sixlowpan` must be enabled. — [smoltcp 0.13.1 source](https://docs.rs/crate/smoltcp/0.13.1/source/src/lib.rs)
- The socket layer is interface-agnostic; the interface and physical layers are the driver's job. — [smoltcp 0.13.1 source](https://docs.rs/crate/smoltcp/0.13.1/source/src/lib.rs)
- The `sntp` crate uses smoltcp in no_std mode without the `alloc` feature. This shows heap-free use is supported in practice. — [docs.rs sntp](https://docs.rs/sntp/latest/src/sntp/lib.rs.html) (search result)
- Older smoltcp MSRVs are listed as 1.65 (0.9.0) and 1.81 (a fork). The MSRV for 0.13.1 was not found. — [smoltcp 0.9.0 source](https://docs.rs/smoltcp/0.9.0/src/smoltcp/lib.rs.html) (search result)
- The virtio-net OSDev page (device IDs, virtqueue, net header) returned HTTP 403; not verified.

### Inferences
- virtio-net plus smoltcp (estimate): 600-900 lines. The driver reuses the existing `src/virtio.rs` queue code (project file), which lowers the effort. smoltcp needs a `Device` impl that hands it RX/TX frames.
- A physical NIC driver (estimate): e1000 is about 500-800 lines; RTL8169 is similar. The project's main work is then the same as virtio-net.
- Networking on real hardware is worth doing only after storage and input, because the shell and boot tests do not need it.

### Gaps
- virtio-net spec and OSDev page: not read (403).
- e1000 and RTL8169 references: not researched.
- smoltcp 0.13.1 MSRV: not found.

## Key Question 5: Best next step for Aeon

### Takeaway
Storage read (AHCI first, then NVMe) is the best next step after the owner's current roadmap item. It is testable in QEMU, the PCI scan already exists, and it unblocks loading programs from disk, which the handoff lists as the next big piece. Keyboard over xHCI matters for real hardware, but it is the largest and riskiest task, so it should follow AHCI.

### Cited Findings
- The owner's test on a real PC found no disk, because only virtio-blk is present (project handoff, "Second session" table). — `G:\Aeon\OS\docs\HANDOFF.md`
- The handoff's recommended next big piece is the ELF loader with address spaces; the USB keyboard is listed as the alternative (decision 8). — `G:\Aeon\OS\docs\HANDOFF.md`
- The project's zero-dependency rule is still open (decision 2). The crates above (`xhci`, `smoltcp`, `fatfs`, `ext4-view`) cannot be used until the owner decides. — `G:\Aeon\OS\docs\HANDOFF.md`
- Keyboard on real hardware: the project reads PS/2 port 0x60 in the kernel shell (`src/shell.rs` 62-70). Many modern laptops have no PS/2 controller, so the shell may get no input on real PCs. (Inference from the code; not checked against hardware.)

### Inferences
- Priority by effort and risk (estimates):

| Step | Why | Effort (estimate) | Testable in QEMU |
|---|---|---|---|
| 1. Owner decides crate policy | Changes every later step | 0 (decision) | n/a |
| 2. AHCI read | Common on SATA PCs; uses existing PCI scan; simplest DMA model | 500-900 lines | Yes (`-device ahci`) |
| 3. NVMe read | Common on newer PCs; needs admin/IO queues | 600-1000 lines | Yes (`-device nvme`) |
| 4. FAT32 write | Extends existing FAT code; needed for saving files | 800-1500 lines | Yes (virtio-blk or AHCI) |
| 5. xHCI keyboard | Needed for real PCs without PS/2 | 2000-3000 lines | Partly (QEMU has xHCI devices) |
| 6. virtio-net + smoltcp | Only needed for networking | 600-900 lines | Yes |

- If the owner wants real-PC keyboard input sooner, the xHCI step can move ahead of NVMe, because keyboard input blocks every real-PC test.
- The ELF loader (owner's recommended step) can use a RAM filesystem first, so it does not strictly need a disk driver. If the owner keeps the ELF step first, AHCI is the first disk driver to add after it.

### Gaps
- The notes do not compare the cost of each step against the owner's time budget, which the owner has not given.
- Real-hardware compatibility (which PCs have which controllers) is not researched.

## Source summary (fetched or seen in search)

- docs.rs xhci: https://docs.rs/crate/xhci/latest (fetched)
- rust-osdev xhci: https://github.com/rust-osdev/xhci (search result)
- osdev.wiki AHCI: https://osdev.wiki/wiki/AHCI (fetched; wiki.osdev.org blocked with 403)
- osdev.wiki NVMe: https://osdev.wiki/wiki/NVMe (fetched; wiki.osdev.org blocked with 403)
- osdev.wiki PCI: https://osdev.wiki/wiki/PCI (fetched)
- NVMe Base Spec 2.0e: https://nvmexpress.org/wp-content/uploads/NVM-Express-Base-Specification-2.0e-2024.07.29-Ratified.pdf (search result)
- smoltcp 0.13.1: https://docs.rs/crate/smoltcp/0.13.1/source/src/lib.rs (search result)
- rust-fatfs: https://github.com/rafalh/rust-fatfs (search result)
- simple-fatfs: https://docs.rs/crate/simple-fatfs/latest/source/src/lib.rs (search result)
- ext4-view: https://docs.rs/crate/ext4-view/latest (search result)

## Gaps across the whole report

- Blocked with HTTP 403: OSDev wiki.osdev.org pages (AHCI, NVMe, Virtio-net, Ext2). The osdev.wiki mirror worked for AHCI, NVMe, and PCI. Virtio-net and ext2 were not read.
- Not read at all: the xHCI spec body, Intel AHCI 1.3.1, the NVMe spec body, the Microsoft FAT spec, the virtio spec, and the UEFI spec's SimpleTextInput section.
- Not searched: Redox, Theseus, and MikanOS driver code; e1000 and RTL8169 references; real-hardware quirks.
- Not sourced: all size estimates in this file.
- No 2026-dated release notes checked for the crates. Versions are as seen in search results on 2026-11-03.
