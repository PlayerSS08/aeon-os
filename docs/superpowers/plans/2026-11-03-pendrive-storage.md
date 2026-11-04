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

1. [x] `fatw.rs` + `fatname.rs` (done 2026-11-04): 36 writer tests + 15 name tests, all written first and watched failing. Covers allocate/free, files across cluster boundaries, folder growth, 8.3 and long names with `~N` aliases, replace, delete, mkdir/rmdir, volume full (byte-identical after a refusal), the `EFI` and label rules, damaged chains, random garbage, a 700-step random storm against a model, and a disk failure injected at every single write (volume must stay sound). `bash tools/fatw-check.sh` runs the same operations on `mkfs.fat` images and `fsck.vfat -n` calls them clean (checked: the script does fail when one FAT copy is not written). Not done: `rename` (the shell can use copy + remove for now). Nothing touches a real disk yet; the writer is not linked into the kernel.
2. [x] `usbmsd.rs` (done 2026-11-04, commit `687e232`): 10 unit tests for CBW/CSW wrappers, SCSI commands (INQUIRY, READ CAPACITY(10), READ(10), WRITE(10), TEST UNIT READY, REQUEST SENSE), sense decoding, capacity math, hostile CSW validation, and fuzzing with random garbage.
3. [x] xHCI bulk endpoints (done 2026-11-04): endpoint context and TRB ring support (`xhcictx.rs`, `xhcitrb.rs`), descriptor detection (`usbdesc.rs`), bulk IN/OUT configuration, stall clearing, BOT reset, `XhciDisk` (`BlockDev` + `BlockWrite`) in `xhci.rs`, disk discovery in `disk.rs`.
4. [x] Block cache and `sync` (done 2026-11-04): 64-slot LRU write-back block cache with dirty slot tracking (`bcache.rs`), flush on `sync`, `reboot`, and `shutdown` (`power.rs`). 5 unit tests for read/write/eviction/dirty flush.
5. [x] Shell integration and safety checks (done 2026-11-04): `/usb` mount adapter (`usbfs.rs`) with volume label `AEON` check, `EFI/` path write protection, FAT32 integration; shell routing in `shell.rs` for `cd`, `ls`, `mkdir`, `rmdir`, `tree`, `df`, `cat`, `write`, `append`, `rm`, `cp`, `mv`, `touch`, `find`, `head`, `tail`, `wc`, `grep`, `hexdump`, `edit`, and `sync` registered in `cmdhelp.rs`. Full QEMU `t_pendrive` test suite (37 passed, 0 failed), sweep (73 passed), and usb (13 passed).
6. [ ] Real pendrive test by the owner, with a spare stick.

## Then: per-process memory (next plan)

Per-process page tables, the ELF loader on top of `elf.rs`, `run <file>`, `spawn` and `wait`, then pipes and `|` and `>` in the shell. Written as its own plan when this one is done.
