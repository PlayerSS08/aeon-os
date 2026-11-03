# Keyboard model of boot menus and Apple's Startup Manager

Research date context: 2026-11-03. Sources marked "fetched" were read in full or in the relevant part. Sources marked "search snippet" were seen only in search results and not opened, so treat them as unverified. Several key questions are only partly answered; see Gaps in each section.

## 1. Arrow keys plus Enter versus per-entry letter shortcuts

### Takeaway
Every boot menu surveyed uses arrow keys plus Enter as the primary model. Letter or digit shortcuts exist as accelerators in GRUB (menuentry --hotkey) and rEFInd (number and letter launch keys), but no source found says single letters are the safe default for destructive actions. Apple's HIG expresses the general rule that shortcuts should use modifier chords, and it binds restart and shut down to chords (Control-Command-Eject, Control-Option-Command-Eject), not to single letters. The Windows Boot Manager and systemd-boot key model could not be fully verified.

### Cited Findings
- GRUB's interactive timeout behavior: "If a hotkey associated with a menu entry is pressed, it will boot the associated menu entry immediately." and "If ESC or F4 are pressed, or SHIFT is held down during that time, it will display the menu and wait for input." (fetched) — [GNU GRUB manual, grub.html](https://www.gnu.org/software/grub/manual/grub/grub.html)
- GRUB's menuentry --hotkey option accepts "a single letter, or one of the aliases 'backspace', 'tab', or 'delete'" (search snippet, not opened) — [GNU GRUB manual, menuentry](https://gnu.org/software/grub/manual/html_node/menuentry.html)
- GRUB hotkey behavior was debated on the mailing list: a 2005 proposal said the hotkey should move the selection bar, not boot the entry (search snippet, proposal only, current behavior not verified) — [bug-grub 2005 thread](https://mail.gnu.org/archive/html/grub-devel/2005-02/msg00037.html)
- GRUB hotkeys did not work when the menu was hidden, per a 2013 Ubuntu bug report (search snippet) — [Ubuntu bug 1178618 mail](https://lists.ubuntu.com/archives/foundations-bugs/2013-November/173232.html)
- rEFInd current manual (rEFInd 0.14.2, updated 2024-04-06): arrow keys move the selection; Enter or spacebar launches the selected entry; 1 through 9 launch a loader by number; E, G, L, M, P, S, W launch specific loaders (ELILO, GRUB, Linux, Mac OS, gptsync, EFI shell, Windows); other letters launch OSes whose names begin with those letters; Insert, F2, Tab, or + open the submenu for the selected entry; Delete (or -) hides a menu tag; Esc or Backspace returns from a submenu. (fetched) — [rEFInd: Using rEFInd](https://rodsbooks.com/refind/using.html)
- rEFInd documents the Mac Delete key as Backspace for the Esc/Backspace return action. (fetched) — same URL
- rEFInd states the firmware reboot entry appears only "if your firmware supports this feature", and that on some machines you "may need to select it from your firmware's boot options". (fetched) — same URL
- systemd-boot: the menu can be shown by pressing and holding a key before systemd-boot launches; timeout values "menu-disabled", "menu-hidden", or 0 suppress the menu; "menu-force" keeps it visible with no countdown. (fetched, Arch man page for loader.conf) — [loader.conf(5), Arch man pages](https://man.archlinux.org/man/loader.conf.5.en.txt)
- systemd-boot in-menu keys (search snippet, not opened): digits 1 to 9 select entries; "+"/"t" and "-"/"T" change the timeout; "v" shows versions; "e" edits the kernel command line; keys are read with a US layout unless firmware says otherwise — [sd-boot(7), Ubuntu manpages](https://manpages.ubuntu.com/manpages/kinetic/man7/sd-boot.7.html)
- Apple HIG, Keyboards (fetched): a keyboard shortcut is "a combination of a primary key and one or more modifier keys (Control, Option, Shift, and Command)". "A keyboard shortcut in a game — called a key binding — often consists of a single key." — [Apple HIG: Keyboards](https://developer.apple.com/design/human-interface-guidelines/keyboards.md)
- Apple HIG (fetched): "Avoid using the Control key as a modifier. The system uses Control in many systemwide features and shortcuts." "Prefer the Command key as the main modifier key." "Define custom keyboard shortcuts for only the most frequently used app-specific commands." — same URL
- Apple HIG standard shortcut table (fetched): Esc is "Cancel the current action or process." Control-Command-Eject "Quit all apps ... and restart the computer." Control-Option-Command-Eject "Quit all apps ... and shut the computer down." — same URL
- Apple HIG (fetched): "Respect standard keyboard shortcuts... prefer creating a custom shortcut for it instead of repurposing a standard one" — same URL
- Apple Intel and Apple silicon selection uses arrow keys or buttons, not letters: on Intel, "select one, then click the up arrow"; on Apple silicon, "select one, then click the Continue button" (fetched) — [Apple: Start up your Mac from a different disk](https://support.apple.com/guide/mac-help/start-up-your-mac-from-a-different-disk-mchlp1034/mac)
- Windows Boot Manager: no verified source. The Microsoft Learn page fetched returned HTTP 404.

### Inferences
- Single-letter hotkeys in GRUB and rEFInd are accelerators layered over arrows and Enter. A letter that boots immediately during a timeout (GRUB hotkey behavior) removes the chance to back out, so a single letter should never be the only way to reach a destructive action. This inference combines the GRUB timeout text with the HIG principle on modifier chords; no source states it directly.
- Apple's own restart and shut down keys in the HIG are modifier chords requiring three keys, which fits the general pattern of avoiding one-key destructive actions. Apple does not state the boot-picker rule explicitly.
- The HIG's "single key" allowance is limited to games. Apple's design guidance does not endorse single-letter shortcuts for app or system commands.

### Gaps
- Windows Boot Manager key model: no source fetched (Microsoft Learn returned 404). Unverified.
- No Apple HIG passage found that explicitly warns against single-letter shortcuts for destructive actions. The HIG Keyboards page does not discuss them. Searching a second time did not surface one.
- systemd-boot key list is only from a search snippet; the sd-boot(7) page was not opened.
- Whether the GRUB --hotkey boots immediately or only moves the selection in current GRUB versions is not verified beyond the timeout text quoted above.

## 2. How Apple's Startup Manager looks and behaves

### Takeaway
Apple's public support pages describe only the start-up gesture and the selection step, not the screen. Intel Macs use Option at power-on, then an up-arrow button to confirm. Apple silicon Macs use press-and-hold on the power button until "Loading startup options" appears, then a Continue button. The Apple silicon startup options screen shows startup disks and a gear icon labeled "Options". The per-volume icons, labels, Escape behavior and on-screen key hints were not found in any fetched source.

### Cited Findings
- Intel Macs: "Press and hold the Option key as you press the power button to start up your Mac." When the startup disks appear, "select one, then click the up arrow" (the page labels the arrow as "the Up arrow" icon used on other Macs) — [Apple: Start up your Mac from a different disk](https://support.apple.com/guide/mac-help/start-up-your-mac-from-a-different-disk-mchlp1034/mac) (fetched)
- Apple silicon Macs: "Press and hold the power button until 'Loading startup options' appears." On a Mac mini, "wait for the system indicator light to turn amber." When the startup disks appear, select one, then click the Continue button, which the page specifies is "on a Mac with Apple silicon" — same URL (fetched)
- Apple's "Mac startup key combinations" article (fetched; published March 13, 2025 per the page, footer 2026): holding Option "starts the Mac into the Startup Manager" and "allows you to choose other available startup disks or volumes." For Apple silicon, the startup options screen "shows your startup disks and a gear icon labeled Options." From there you can start from another disk, start in safe mode, or use macOS Recovery. — [Apple: Mac startup key combinations, HT201255](https://support.apple.com/en-us/HT201255) (fetched)
- The Boot Camp guidance in the same article says to set Startup Disk to macOS before restarting when a Mac is set to start Windows. — same URL (fetched)
- Community sources (search snippets, not verified): on Apple silicon the Option key method does not apply and the power-button method is needed; an external drive or a non-Apple keyboard may hide the picker. — [iBoysoft Startup Manager guide](https://iboysoft.com/wiki/mac-startup-manager.html), [Apple Discussions thread](https://discussions.apple.com/docs/DOC-4672)
- Apple's "Mac startup key combinations" article does not give a complete list of on-screen keys. — same URL (fetched)

### Inferences
- The Apple pages describe a two-step gesture: a single explicit confirm button (up arrow on Intel, Continue on Apple silicon) follows the disk choice. Apple's public documentation does not describe a Return key or an Escape key in the picker. Apple's support text uses "click", so the on-screen buttons are the primary controls.
- The Apple silicon screen combines disk icons with a gear labeled "Options". This means the picker is a graphical list of volumes plus a separate options entry, not a plain text list. The fetched text does not confirm per-volume icons or labels.

### Gaps
- Icons per volume, label under each icon, selection highlight, and whether arrow keys and Return work on Apple silicon: not found in any fetched Apple source. Unverified.
- Escape handling in the picker: not found. Unverified.
- Which keys are shown on screen and which are hidden: not found. Unverified.
- Difference between the Apple picker and a text list: not documented in the sources fetched.
- Apple's Option-key page for Apple silicon was not located; the search result for a current Apple silicon Option-key page showed only community content.

## 3. Firmware navigation keys (Esc, F2, Del, F12) and firmware-setup position

### Takeaway
No fetched source documents the firmware setup keys or where firmware setup sits in a boot list. rEFInd's manual shows that firmware reboot appears as a tag, and that it depends on the firmware. Placement and key bindings must be verified against vendor firmware documentation before use.

### Cited Findings
- rEFInd manual (fetched): "A tag to reboot into the firmware appears if your firmware supports this feature." — [rEFInd: Using rEFInd](https://rodsbooks.com/refind/using.html)
- rEFInd manual (fetched): on some machines you "may need to select it from your firmware's boot options or reconfigure your firmware to present rEFInd automatically." — same URL
- A search snippet from an older rEFInd mirror says Esc, Enter, or a function key (often F8 or above) opens the firmware boot menu on PCs. This came from an older copy of the manual and was not opened. Unverified — [rEFInd 0.7.7 mirror](https://lsi2.ugr.es/mcapel/tutorials/instalacion/refind-bin-0.7.7/docs/refind/using.html)

### Inferences
- The rEFInd manual treats firmware reboot as an optional tag shown when the firmware allows it, so the firmware entry is not guaranteed to appear in a fixed position.

### Gaps
- Which key each vendor uses (Esc, F2, Del, F12) for firmware setup: no vendor source fetched. Unverified.
- Whether firmware setup is usually last in the boot list: no source found. Unverified.
- The UEFI specification does not appear to set a firmware-setup key; this was not verified against the spec text.

## 4. UEFI SimpleTextInput scan codes

### Takeaway
The Rust uefi crate source, version 0.41.0, lists the scan codes the user cited: Up 0x01, Down 0x02, Right 0x03, Left 0x04, Home 0x05, End 0x06, Escape 0x17. This matches the values in the question. The primary UEFI specification could not be fetched (HTTP 403), so the values are consistent with a secondary source but not confirmed from the spec.

### Cited Findings
- uefi crate ScanCode constants (fetched source, crate 0.41.0): NULL 0x00, UP 0x01, DOWN 0x02, RIGHT 0x03, LEFT 0x04, HOME 0x05, END 0x06, INSERT 0x07, DELETE 0x08, PAGE_UP 0x09, PAGE_DOWN 0x0A, FUNCTION_1 to FUNCTION_12 as 0x0B to 0x16, ESCAPE 0x17, then higher-numbered function keys from 0x68, and media and OEM keys from 0x7F and 0x100 — [uefi crate input.rs](https://docs.rs/crate/uefi/latest/source/src/proto/console/text/input.rs) (fetched)
- The crate's doc comment says codes 0x8000 to 0xFFFF are reserved for OEM use and that the enum should not be modeled as a Rust enum. The crate does not cite the spec section. — same URL (fetched)
- UEFI 2.10 Appendix B (Console) is the expected primary source. Fetch returned HTTP 403 (not read). — [UEFI 2.10 Appendix B](https://uefi.org/specs/UEFI/2.10/Apx_B_Console.html)
- An edk2 serial driver maps ANSI sequences to scan codes, with Home as ESC [ H and End as ESC [ F for PC ANSI (search snippet, not opened). — [edk2 SimpleTextInOut.c, Linaro](https://android.googlesource.com/device/linaro/bootloader/edk2/+/245cda6/EmbeddedPkg/SimpleTextInOutSerial/SimpleTextInOut.c)

### Inferences
- The values in the question agree with the crate, which mirrors the specification. Treat them as likely correct but cite the spec before relying on them in a standard document.

### Gaps
- Primary-source confirmation from UEFI 2.10 Appendix B: not obtained (403). Unverified against spec text.
- The crate's ESCAPE value is taken from source code, not the spec. Unverified.

## 5. Showing a one-line description for the highlighted entry and listing all options

### Takeaway
No source fetched gives guidance on a one-line description for the highlighted entry, or on listing all options on screen. The fetched boot-menu manuals show their keys and menus, but none documents a per-entry description line. Treat this question as unanswered in these notes.

### Cited Findings
- rEFInd shows a tag for each loader and a tag for firmware reboot where supported. Its manual describes tags and hiding them with Delete or -. (fetched) — [rEFInd: Using rEFInd](https://rodsbooks.com/refind/using.html)
- systemd-boot's timeout and menu-force options control whether the menu is visible at all, which bears on whether the full option list is shown. (fetched) — [loader.conf(5), Arch man pages](https://man.archlinux.org/man/loader.conf.5.en.txt)
- The Apple silicon startup options screen shows disks plus a gear labeled "Options", so the option list includes a separate category. (fetched) — [Apple HT201255](https://support.apple.com/en-us/HT201255)

### Inferences
- The fetched sources show that the menu lists boot entries and a separate options or utilities entry. None shows a per-entry description.

### Gaps
- Apple HIG guidance on descriptive text for the selected item in a list or picker: not located. The HIG Keyboards page does not cover this.
- No boot-menu source found on whether all options should be visible at once versus revealed on selection.

## Source list (fetched unless marked)

- https://support.apple.com/guide/mac-help/start-up-your-mac-from-a-different-disk-mchlp1034/mac (fetched)
- https://support.apple.com/en-us/HT201255 (fetched)
- https://developer.apple.com/design/human-interface-guidelines/keyboards.md (fetched)
- https://www.gnu.org/software/grub/manual/grub/grub.html (fetched; truncated)
- https://man.archlinux.org/man/loader.conf.5.en.txt (fetched)
- https://rodsbooks.com/refind/using.html (fetched)
- https://docs.rs/crate/uefi/latest/source/src/proto/console/text/input.rs (fetched)
- https://uefi.org/specs/UEFI/2.10/Apx_B_Console.html (403, not read)
- https://www.gnu.org/software/grub/manual/grub/grub.html#menuentry (not fetched; GRUB manual menuentry page returned 429 and the search snippet was used)
- https://manpages.ubuntu.com/manpages/kinetic/man7/sd-boot.7.html (search snippet only)
- https://iboysoft.com/wiki/mac-startup-manager.html (search snippet only)
