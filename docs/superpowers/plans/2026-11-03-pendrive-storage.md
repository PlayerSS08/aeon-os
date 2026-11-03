# Files that stay on the pendrive: implementation plan

**Goal:** boot Aeon from the pendrive, create, edit, copy, move and delete files and folders on the pendrive's FAT32 volume, and find them again after a restart (and on a PC).

**Why not simpler:** after Aeon leaves the firmware (ExitBootServices) the firmware's USB and file services are gone, so Aeon needs its own USB mass-storage driver and its own FAT32 writer. There is no shortcut that survives a restart.

**Architecture:** four layers, each testable alone.
1. `fatw.rs` (pure): FAT32 writing on top of the existing read-only `fat.rs`: cluster allocation, FAT copies, directory entries (short and long names), create, overwrite, delete, mkdir, rmdir, rename. Host-tested against images made by `mkfs.fat` and checked by `fsck.vfat`.
2. `usbmsd.rs` (pure) + `xhci.rs` additions: SCSI commands (INQUIRY, READ CAPACITY(10), READ(10), WRITE(10), TEST UNIT READY, REQUEST SENSE), command and status wrappers of the bulk-only transport, bulk transfer rings on the xHCI controller. QEMU test with `-device usb-storage`.
3. `BlockDev` for the pendrive plus a write path in `block.rs`, a block cache with an explicit `sync`.
4. Shell: a `/usb` mount (folder name to be decided), the existing commands (`ls cp mv rm mkdir rmdir cat edit write append`) work inside it; `sync`; flush on `reboot` and `shutdown`.

## Safety rules (checked by tests, never relaxed)

- Only a volume whose label is `AEON` is writable; anything else stays read-only.
- Never write inside `EFI/` (the boot files) or to the boot sector, FAT or FSInfo of anything but through the FAT writer.
- Every write is validated first: sector in range, cluster number in range, name legal; hostile images must give errors, not panics.
- Writes go to disk only on `sync`, `reboot`, `shutdown` or when the cache is full; the volume is marked "dirty" while writes are pending and "clean" after.
- The first real-hardware test uses a pendrive with nothing else on it.

## Tasks

1. [ ] `fatw.rs`: tests first (allocate and free clusters, grow and shrink chains, write and overwrite files across cluster boundaries, directory growth, 8.3 names and long names, delete, rename, mkdir and rmdir, volume full, random operation storms compared with a model, `fsck.vfat` clean after each scenario).
2. [ ] `usbmsd.rs`: tests first (command and status wrappers, sense data decoding, capacity, hostile device answers).
3. [ ] xHCI bulk endpoints: configure endpoint for bulk IN and OUT, transfer ring, polled completion with timeout; QEMU test: Aeon lists the stick, reads sector 0.
4. [ ] Block cache and `sync`; QEMU test: write a file, `sync`, restart the VM on the same image, the file is there; `fsck.vfat` on the image is clean.
5. [ ] Shell integration and safety checks; tests: refusal on a non-AEON label, refusal inside `EFI/`.
6. [ ] Real pendrive test by the owner, with a spare stick.

## Then: per-process memory (next plan)

Per-process page tables, the ELF loader on top of `elf.rs`, `run <file>`, `spawn` and `wait`, then pipes and `|` and `>` in the shell. Written as its own plan when this one is done.
