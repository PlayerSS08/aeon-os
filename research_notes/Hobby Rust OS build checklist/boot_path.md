# Boot Path and Bootloader Design for a Small UEFI x86_64 Rust Hobby OS

Research as of 2026-11-03. Sources are cited inline. Claims that rest on a blog, forum, mailing list, bug report, or search-result snippet (not a fetched primary page) are labelled. Several primary pages were rate-limited (GNU GRUB manual, twice) or only partly returned (Microsoft BCDEdit, systemd sd-boot(7)); those are listed under Gaps.

## Q1. One-image UEFI kernel versus separate bootloader

### Takeaway
Mainstream loaders are separate programs that load a kernel file. The only mainstream precedent for a kernel that is itself a PE/COFF EFI application is the Linux EFI stub and the systemd-stub Unified Kernel Image (UKI). Limine and Redox are separate Rust-or-C loaders that hand a documented environment to the kernel; a one-image design must take on the work a loader normally does.

### Cited Findings
- Linux EFI boot stub: the bzImage header is modified and an EFI entry point is added so firmware can execute the kernel directly. Enabled with `CONFIG_EFI_STUB`; the image is copied to the ESP with a `.efi` extension — [kernel.org EFI stub admin guide](https://www.kernel.org/doc/Documentation/admin-guide/efi-stub.rst) (search snippet; not fully fetched).
- systemd-boot's `linux` keyword requires the kernel to have an EFI stub built with `CONFIG_EFI_STUB` — [systemd BOOT docs](https://systemd.io/BOOT/) (fetched).
- systemd-stub UKIs: `ukify` is the upstream-recommended tool to build UKIs and calls systemd-measure to embed PCR 11 values — [systemd-measure(1)](https://man.archlinux.org/man/systemd-measure.1.en) (search snippet); the man pages say not to call systemd-measure directly when constructing a UKI.
- Limine is a separate loader. Its protocol provides request/response features: memory map (sorted typed entries), framebuffer descriptors (with optional EDID and mode list), RSDP address, entry-point override, HHDM offset, and firmware type (x86BIOS, UEFI32, UEFI64). Kernels must load at or above `0xffffffff80000000`; lower-half kernels are not supported — [Limine PROTOCOL.md, v7.x branch](https://github.com/limine-bootloader/limine/blob/v7.x/PROTOCOL.md) (fetched).
- Limine protocol revisions: the current protocol repo says 4 base revisions are specified (0 through 3) and revisions 0 through 2 are deprecated — [limine-protocol PROTOCOL.md](https://github.com/limine-bootloader/limine-protocol/blob/fedf97facd1c473ee8720f8dfd5a71d03490d928/PROTOCOL.md) (fetched). The v7.x copy describes 3 revisions (0 to 2), so the difference is version drift, not a contradiction. The `limine` Rust crate README says base revision 6 — [docs.rs limine 0.6.3](https://docs.rs/crate/limine/0.6.3/source/README.md) (search snippet); check which revision your loader and crate both support.
- Redox OS: the bootloader is written in Rust. UEFI is 64-bit only; firmware starts a PE32+ program, which enters a UEFI entry in `src/os/uefi/mod.rs`. BIOS and UEFI share a common path: set up the memory map and display mode, find the RedoxFS partition, load the kernel, bootstrap and initfs, jump to the kernel — [Redox book, Boot Process](https://doc.redox-os.org/book/boot-process.html) (search snippet). Redox 0.7.0 rewrote the bootloader so BIOS and UEFI share most code and it reuses the OS filesystem driver to unlock encrypted RedoxFS — [Redox 0.7.0 release notes](https://redox-os.org/news/release-0.7.0) (search snippet).
- Theseus: two unrelated projects share the name in search results. The main one (theseus-os/Theseus) defaults to GRUB and can switch to Limine; its tests have used a legacy BIOS path, and it recommends a pinned Limine revision — [theseus-os/Theseus](https://www.github.com/theseus-os/Theseus) (search snippet; docs not read). A separate x86_64 hobby project (emilf/TheseusOS) is a single UEFI executable that collects firmware state, stores a handoff structure, calls ExitBootServices, and jumps to the kernel — [emilf/TheseusOS](https://github.153394839.workers.dev/emilf/TheseusOS) (search result mirror URL; search snippet, not verified in code).
- Boot Loader Specification (BLS, UAPI.1): defines file formats and naming so distribution-independent menus can be read by several loaders. Type #1 entries are one `.conf` per menu item under `loader/entries`; a second type covers UKIs — [UAPI Group BLS](https://uapi-group.org/specifications/specs/boot_loader_specification/) (search snippet).
- Community doubt about BLS on EFI: a 2022 U-Boot list thread asked what the spec is useful for on EFI systems where firmware variables already control boot order — [U-Boot list, 2022-11](https://lists.denx.de/pipermail/u-boot/2022-November/499986.html) (mailing list; opinion).

### Inferences
- A one-image kernel removes a second binary, a loader-to-kernel protocol, and a second signing step. It must instead do its own file access, GOP setup, memory-map capture, and ACPI discovery before its own allocator exists. Limine and Redox exist partly so those steps live in shared code.
- The TheseusOS-style "single UEFI executable that hands off to kernel entry" is the closest structural match to a one-image design; the Linux stub and UKI are the closest mainstream precedent.
- If Secure Boot signs the image (Q5), a one-image design means a kernel change is also a loader change, and a loader bug is a kernel-signing event.

### Gaps
- GRUB's and rEFInd's design as loaders was not checked against primary sources in this session.
- No source compares the one-image trade-offs directly; the list above is inference.
- Theseus internals were not read (only search snippets).

## Q2. Boot menu and countdown design

### Takeaway
systemd-boot shows no menu by default and boots the default entry at once; holding or pressing a key during bootup brings up the menu. GRUB's `GRUB_TIMEOUT_STYLE` decides whether the countdown runs before the menu, and a timeout of 0 boots at once. rEFInd and Windows both use a seconds-based timeout, but neither default could be read from a primary page in this session.

### Cited Findings
- systemd-boot `timeout` (loader.conf): seconds; `menu-disabled`, `menu-hidden` or `0` mean no menu and immediate default boot; `menu-force` keeps the menu up with no timeout — [loader.conf(5)](https://man7.org/linux/man-pages/./man5/loader.conf.5.html) and [sd-boot(7)](https://man7.org/linux//man-pages/man7/sd-boot.7.html) (search snippets; full text not fetched). The default is stated as `0` (no menu) in a search summary; confirm on the fetched sd-boot page.
- systemd-boot example config `timeout 3` and the statement that without a timeout "the default entry is executed right away" — [systemd BOOT docs](https://systemd.io/BOOT/) (fetched).
- Held-key behaviour: "Pressing the Space key (or most other keys actually work too) during bootup will show an on-screen menu" — [systemd BOOT docs](https://systemd.io/BOOT/) (fetched). With timeout 0, the docs say to hold a key (space recommended) before the loader initializes, and warn the window may be short depending on firmware; they suggest a non-zero timeout instead — [systemd BOOT docs](https://systemd.io/BOOT/) (fetched).
- systemd-boot hotkeys: `l`, `w`, `a`, `s` and `1` to `9` boot an entry directly; `f` reboots to firmware (only if the firmware supports it); `F2`, `F10`, `Del`, `Esc` are accepted for firmware compatibility; `+`/`t` and `-`/`T` change the timeout, stored in a non-volatile EFI variable; `e` edits the command line; Enter boots; `d` sets default — [sd-boot(7), systemd 258](https://www.freedesktop.org/software/systemd/man/258/sd-boot.html) (fetched). Keyboard layout defaults to US unless configured in firmware — [sd-boot(7), systemd 255](https://www.freedesktop.org/software/systemd/man/255/systemd-boot.html) (search snippet).
- GRUB timeout (GNU manual, fetched): "A timeout of '0' means to boot the default entry immediately without displaying the menu." A timeout of `-1` (or unset) "means to wait indefinitely." When `GRUB_TIMEOUT_STYLE` is `countdown` or `hidden`, the timeout is counted before the menu is displayed — [GNU GRUB manual, timeout](https://www.gnu.org/software/grub/manual/grub/html_node/timeout.html) (fetched). The `menu` value text and default were not returned (the timeout_style subpage returned HTTP 429 twice).
- GRUB hidden menu in practice (blog, unverified): a RHEL guide pairs `GRUB_TIMEOUT=0` with `GRUB_TIMEOUT_STYLE=hidden` and says the menu is still reachable with Esc, F4 or Shift — [OneUptime RHEL 9 guide](https://oneuptime.com/blog/post/2026-03-04-configure-grub2-boot-menu-timeout-rhel-9/markdown) (blog, not official).
- GRUB hidden menu bug (bug report): on Ubuntu 24.10, `GRUB_TIMEOUT=0` with `GRUB_TIMEOUT_STYLE=hidden` still showed a menu with a 30-second timeout; adding `GRUB_RECORDFAIL_TIMEOUT` fixed it — [Ubuntu bug 2100718](https://lists.snapcraft.io/archives/foundations-bugs/2025-March/534182.html) (bug report, distro-specific).
- GRUB deprecated settings: `GRUB_HIDDEN_TIMEOUT` and `GRUB_HIDDEN_TIMEOUT_QUIET` are deprecated in favour of `GRUB_TIMEOUT_STYLE` — [Launchpad bug 1258597](https://launchpad.net/ubuntu/+source/grub2/+bug/1258597/comments/4) (bug comment).
- rEFInd timeout (rodsbooks configfile docs, fetched): "Sets the timeout period in seconds." `0`: "rEFInd waits indefinitely for user input." `-1`: "rEFInd will normally boot immediately to the default selection," and a shortcut key such as W still boots that system. Any other key shows the menu with no timeout. The example says "rEFInd will wait 20 seconds before launching the default boot loader." No default value is stated — [rEFInd configfile docs](https://rodsbooks.com/refind/configfile.html) (fetched; URL found via the rodsbooks site, which also hosts the using page).
- Windows Boot Manager: the timeout is set with `bcdedit /timeout <seconds>`, and the Microsoft example is `bcdedit /timeout 15` — [Microsoft Learn, Changing the Boot Menu Time-out](https://learn.microsoft.com/en-us/windows-hardware/drivers/devtest/changing-the-boot-menu-time-out) (search snippet). `bootmenupolicy` is `Legacy` or `Standard`; the default is `Standard` on Windows 10 and 8.1. In `Standard`, the menu appears only under conditions such as a startup failure or multiple entries, and the F8 key is ignored during boot — [Microsoft Learn, BCDEdit /set](https://learn.microsoft.com/en-us/windows-hardware/drivers/devtest/bcdedit--set) (fetched). A third-party guide gives a 0 to 999 second range (unverified against Microsoft). A NeoSmart forum post says `displaybootmenu no` did not hide the menu for one user (anecdote).

### Inferences
- A hobby loader can reasonably default to "no menu, boot now" and expose a menu via a held key or Esc, matching systemd-boot. A held key is firmware-timing dependent, so a short non-zero countdown is more dependable for reaching a menu. This is inference from the systemd docs' own advice.
- GRUB's two overlapping settings (`GRUB_TIMEOUT_STYLE` plus the deprecated hidden-timeout variables), and the recordfail issue, argue for one explicit timeout value and one menu-mode setting.

### Gaps
- GRUB's default timeout and `menu` style text: not returned (429 on the timeout_style page).
- Windows default timeout: not stated on the Microsoft pages fetched; the BCDEdit /set page does not document `timeout` (it is under `bcdedit /timeout`).
- rEFInd default timeout: not stated in the fetched page.
- systemd-boot default `timeout` of `0`: from a search summary only.
- The GRUB Esc/Shift behaviour is only from a blog.

## Q3. Fast boot: where time goes

### Takeaway
No primary source gives realistic per-stage numbers for firmware POST, loader, and kernel init. Anecdotal reports show the firmware and loader stages can be very slow on some boards, and that boot order and firmware settings can dominate. A bootloader can shorten its own countdown and its file loading; it cannot shorten firmware POST or kernel init.

### Cited Findings
- `systemd-analyze` with no arguments reports firmware, loader, kernel and userspace times. Firmware and loader times come from EFI boot timestamps and may be missing or wrong when those cannot be read — [opensource.com, systemd startup](https://opensource.com/article/20/9/systemd-startup-configuration) (tutorial); [freedesktop bug 64371](https://bugs.freedesktop.org/show_bug.cgi?id=64371) (bug report).
- Anecdote: a loader time of about 10 minutes was traced to boot order on one Manjaro system, and changing the boot order fixed it — [Manjaro forum](https://forum.manjaro.org/t/systemd-analyze-shows-something-strange-10min-57-502s-loader/92505) (forum, single user).
- Anecdote: an Arch forum user saw large firmware gains after disabling a memory overclocking profile — [Arch forum](https://bbs.archlinux.org/viewtopic.php?id=262636) (forum, single user).

### Inferences
- A bootloader can cut its own countdown (systemd-boot `timeout 0`, GRUB `timeout 0`, rEFInd `-1`), its file-loading time (fewer and smaller files, a direct protocol), and any firmware-visible waits it adds (for example, long GOP mode sets). It cannot cut firmware POST, memory training, or kernel init after handoff.
- The firmware-to-loader stage should be measured first on the target hardware with `systemd-analyze` or timestamped serial output, rather than guessed.

### Gaps
- No primary source for typical firmware, loader, or kernel times on current hardware. No numbers are given here rather than invented.
- No source on which stage a one-image kernel adds time to.

## Q4. Firmware setup key placement and "reboot to firmware"

### Takeaway
The firmware reads its setup key (F2, Del, or similar) during its own POST, before any bootloader runs. A bootloader can only ask the firmware to stop at its setup UI on the next boot, using the `OsIndications` variable, and only if `OsIndicationsSupported` advertises the bit.

### Cited Findings
- UEFI spec 2.10, Section 8.5.4 (search snippet of the uefi.org page): `EFI_OS_INDICATIONS_BOOT_TO_FW_UI` is bit value 0x1; the firmware advertises support by setting the bit in `OsIndicationsSupported`; the OS sets the bit in `OsIndications` to request a stop at firmware UI on the next boot; after the firmware consumes it and shows the UI, it should clear the bit. `OsIndicationsSupported` is rebuilt each boot and the OS cannot modify it — [UEFI 2.10, Runtime Services](https://uefi.org/specs/UEFI/2.10/08_Services_Runtime_Services.html) and [UEFI 2.10, Boot Manager](https://uefi.org/specs/UEFI/2.10/03_Boot_Manager.html) (search snippets; section text not fetched).
- systemd-boot offers "Reboot Into Firmware Interface" only if the firmware supports it — [systemd BOOT docs](https://systemd.io/BOOT/) and [sd-boot(7), systemd 258](https://www.freedesktop.org/software/systemd/man/258/sd-boot.html) (fetched).
- systemd's shared EFI code defines the bit, rewrites the variable only when its value changes, and notes that some firmware drops `OsIndications` when unset — [systemd efi-api.c](https://third-party-mirror.googlesource.com/systemd/+/refs/heads/master/src/shared/efi-api.c) (search snippet; third-party mirror, not confirmed upstream).
- A Rust boot manager, bootmgr-rs-core, reads `OsIndications` (0 if missing), ORs in bit 1, writes it back, then warm-resets — [bootmgr-rs-core firmware.rs](https://docs.rs/crate/bootmgr-rs-core/latest/source/src/boot/action/firmware.rs) (crate source snippet).
- FreeBSD's efi-setup patch checks `OsIndicationsSupported` before setting the bit — [FreeBSD review D32634](https://reviews.freebsd.org/D32634) (code review).
- A FreeBSD review describes the request as one-shot, consistent with the spec's "clear the bit" rule — [FreeBSD review D25839](https://reviews-dev.freebsd.org/D25839?id=75030) (code review).
- Practice (forum): `systemctl reboot --firmware-setup` can report that the firmware does not support booting to setup on some boards; a BIOS update fixed one ASUS board; GRUB has a `fwsetup` command — [Debian forum](https://debianforum.de/forum/viewtopic.php?p=1336233) (forum, single user).
- U-Boot: a 2026 patch series adds `EFI_OS_INDICATIONS_BOOT_TO_FW_UI` support; not confirmed merged — [U-Boot list, 2026-03](https://lists.denx.de/pipermail/u-boot/2026-March/613668.html) (mailing list).

### Inferences
- Setup-key placement is a firmware policy, not a loader choice. A hobby loader that offers "firmware setup" should check `OsIndicationsSupported` first and hide the entry when the bit is absent.
- Because the bit is cleared after use, the loader should set it, reset, and not rely on it persisting.

### Gaps
- Exact F2/Del timing window relative to the firmware boot manager: no source.
- Spec section text was not fetched; the description rests on search snippets from uefi.org.

## Q5. Secure Boot, measured boot, and GOP mode selection

### Takeaway
Shim plus MOK is the distro path for Secure Boot with a third-party-signed loader, and SBAT is its revocation mechanism. systemd-stub UKIs and systemd-boot measure into specific TPM2 PCRs, and a signed PCR 11 policy can gate secrets. For GOP, the cited guidance is native-mode-from-EDID first, then same aspect ratio, and not below 1024x768; a loader can set the mode explicitly, as Limine does.

### Cited Findings
- SBAT: a mechanism for revoking older core boot components (shim, grub2) using generation numbers. Oracle's guide describes `latest` and `previous` policies, with `latest` the default under Secure Boot — [Oracle Linux 9, Secure Boot tools](https://docs.oracle.com/en/operating-systems/oracle-linux/9/secure-boot/sboot-ToolsandApplicationsforAdministeringSecureBoot.html) (vendor doc, search snippet).
- Shim and MOK: the distribution vendor key is built into shim; a separate Machine Owner Key database is managed through MOK Manager — [Oracle Linux Secure Boot docs](https://docs.oracle.com/en/operating-systems/oracle-linux/secure-boot/sboot-ToolsandApplicationsforAdministeringSecureBoot.html) (vendor doc, search snippet).
- SBAT compatibility problem: shim 15.8 or newer (released 2024-01-23) requires SBAT level 5 on distributions such as Debian sid; systems with older firmware or SBAT data can fail with a security policy violation — [Tails issue 20471](https://gitlab.tails.boum.org/tails/tails/-/issues/20471) (issue tracker; single report).
- mokutil SBAT policy values differ by version: one man page lists `latest | automatic | delete`, Oracle lists `latest` and `previous` — [mokutil man page](https://mankier.com/1/mokutil) (search snippet).
- systemd-boot measurements (fetched from systemd TPM2 PCR page): firmware-side events: PCR 1 gets SMBIOS types 1, 2 and 11 (EV_EVENT_TAG); PCR 5 gets `loader.conf` (EV_EVENT_TAG); PCR 12 gets the kernel command line (EV_IPL). Firmware-side measurements before ExitBootServices appear in the firmware TPM2 event log; userspace events go to `/run/log/systemd/tpm2-measure.log` — [systemd TPM2 PCR measurements](https://systemd.io/TPM2_PCR_MEASUREMENTS/) (fetched).
- systemd-stub measurements (same page, fetched): PCR 1 gets SMBIOS data if systemd-boot did not; PCR 11 gets UKI PE section names and data (EV_IPL), once per section; PCR 12 gets command lines, DeviceTree, initrd, ucode addons and credential initrds; PCR 13 gets the sysext initrd. Userspace: PCR 9 (NvPCR init), PCR 11 (boot phase strings), PCR 15 (machine ID, filesystem identity, LUKS volume key) — same page (fetched). The page states EV_IPL will not be used for new firmware measurements.
- systemd-measure pre-computes expected PCR 11 values, starting from zeroed PCR 11, for a systemd-stub UKI and can sign them; the `.pcrpkey` section holds the public key used by systemd-cryptenroll and systemd-creds defaults; added in systemd 252 — [systemd-measure(1)](https://man.archlinux.org/man/systemd-measure.1.en) and [systemd 253 man page](https://www.freedesktop.org/software/systemd/man/253/systemd-measure.html) (search snippets).
- systemd-boot Secure Boot enrollment: keys go in `/loader/keys/<name>/{db,dbx,KEK,PK}.auth` on the ESP; enrollment applies when firmware is in setup mode and the files are present on the ESP — [systemd BOOT docs](https://systemd.io/BOOT/) (fetched).
- Limine per-entry `resolution:` directive calls `gop->SetMode()` before handing off, changing the framebuffer the kernel gets; refresh rate cannot be set via GOP — [Limine commit on an nixpkgs fork](https://code.ornl.gov/nix/nixpkgs/-/commit/ffffc8aaa1191c2a910b60c459c16ac9e053d283) (downstream commit snippet; verify against Limine's own config docs).
- Firmware GOP guidance (as summarized in a search result, not verified on the page): firmware should try the EDID native resolution and timing, else a mode with the same aspect ratio, and at minimum 1024x768. The linked page is an MSDN "UEFI GOP mode test" document — [MSDN UEFI GOP mode test](https://msdn.microsoft.com/en-us/library/windows/hardware/hh997769(v=vs.85)) (not fetched; attribution unverified).
- GOP practice (forum): FreeBSD's `gop set N` in loader.conf was ignored for some users, leaving 1024x768; `efi_max_resolution` fixed one case — [FreeBSD forums](https://forums.freebsd.org/threads/gop-set-mode-being-ignored-in-boot-loader-conf.77779/) (forum, anecdotal). Manual `gop mode` / `gop set` in a UEFI shell lists and sets modes; firmware updates can change GOP behaviour — [FreeBSD forums](https://forums.freebsd.org/threads/gop-set-mode-being-ignored-in-boot-loader-conf.77779/latest) (forum).

### Inferences
- For a one-image kernel, Secure Boot means signing the single PE image with a key the firmware trusts (or a shim chain for a distro key). Every kernel change is then a re-sign. SBAT matters only if you use shim or a grub-style chain; a hobby OS with its own key still has to load under the firmware's db.
- Measured boot is a documented pattern through UKI plus PCR 11 and PCR 12 measurements. The cited mechanisms are systemd-specific; a custom kernel would need to emit TCG event log entries itself.
- A loader should enumerate GOP modes, prefer EDID-native, fall back by aspect ratio, log the chosen mode, and allow an explicit override (Limine's model).

### Gaps
- TCG PC Client firmware profile (PCR allocation for PCR 0 to 7 and the event log format): not fetched.
- ConSplitter and multi-GPU console routing: not researched.
- UEFI spec GOP and EDID sections: not fetched.
- The Microsoft GOP rule is unverified (see above).
- No source found for PCR 4, 7 and 8 behaviour from systemd (the systemd page does not cover them).
