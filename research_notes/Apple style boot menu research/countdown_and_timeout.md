# Boot Manager Countdown and Timeout Behaviour

Scope: GRUB, systemd-boot, rEFInd, Windows Boot Manager (bootmgr/BCD), Apple Startup Manager. Sources were fetched in this session; anything not confirmed is marked **UNVERIFIED** or listed under Gaps. Search-snippet-only claims are marked as such.

## Key Question 1: Default timeout lengths and accessibility

### Takeaway
GRUB's documented default is 5 seconds. systemd-boot's default is "no menu" (boot immediately) unless a menu timeout is set. rEFInd's manual gives no default. Windows' 30 s default is only reported by forums (not confirmed in Microsoft docs). Apple documents no countdown at all. No source found gives a recommended accessibility timeout.

### Cited Findings
- GRUB: `GRUB_TIMEOUT` default is `5` seconds; `0` boots without showing the menu; `-1` waits indefinitely — [GNU GRUB manual, Simple configuration handling](https://www.gnu.org/software/grub/manual/grub/grub.html)
- systemd-boot: the `timeout` option is "How long the boot menu should be shown before the default entry is booted, in seconds." Default when unset: the manual says the menu is not shown and "the default entry will be booted immediately" — [systemd-boot(7)](https://www.freedesktop.org/software/systemd/man/latest/systemd-boot.html), [loader.conf(5)](https://www.freedesktop.org/software/systemd/man/latest/loader.conf.html)
- rEFInd: `timeout` "Sets the timeout period in seconds." `0` disables the timeout (waits indefinitely); `-1` boots the default immediately. The page gives example values `timeout 20` and `timeout 5` but **does not state a default** — [rEFInd configuration file reference](https://www.rodsbooks.com/refind/configfile.html)
- Windows: `bcdedit /timeout` "sets the time to wait, in seconds, before the boot manager selects a default entry." Microsoft's page uses `30` only as an example and does **not** state it is the default — [Microsoft Learn, BCDEdit /timeout](https://learn.microsoft.com/en-us/windows-hardware/drivers/devtest/bcdedit--timeout)
- Windows 30 s default: a search snippet (not a fetched page) reports a 30-second default and the top entry launching on its own — [Windows Central, search result](https://www.windowscentral.com/how-change-list-time-boot-menu-windows-10). The fetch returned only the title, so this is **UNVERIFIED** against the article body. Another forum guide (ElevenForum) also states 30 s, but that fetch returned HTTP 403, so it is also **UNVERIFIED**: [ElevenForum](https://www.elevenforum.com/t/change-boot-menu-time-out-to-choose-an-operating-system-in-windows-11.5454/)
- Apple: the Apple Support page fetched (HT204417, startup disk selection) describes holding Option as the power button is pressed (Intel) or pressing and holding the power button until "Loading startup options" appears (Apple silicon). It does not mention a timeout — [Apple Support HT204417](https://support.apple.com/en-us/HT204417). A separate Apple guide URL returned no Startup Manager content and was not usable. A search for a Startup Manager timeout found only community posts and no documented value — [search results summary](https://discussions.apple.com/thread/256009570)

### Inferences
- The menu is a per-machine choice rather than a fixed Apple-style countdown. On Apple hardware the picker is a user-held key action, not a timed menu, so there is no timeout to tune.
- A 5 s GRUB default is short for users who need more reaction time. This is an inference from the 5 s value, not a documented accessibility guideline.

### Gaps
- No source found for a recommended or accessibility-tested timeout value in any of the five managers. Needs a dedicated accessibility source (for example WCAG guidance on timing, which was not searched).
- Windows default 30 s is not confirmed by Microsoft documentation in this session.
- Apple: no official statement on whether the Startup Manager has a timeout. Absence in the fetched pages is not proof.

## Key Question 2: Does a keypress stop, restart, or keep the countdown?

### Takeaway
GRUB and systemd-boot document that a key press interrupts the timed boot (GRUB explicitly; systemd-boot only as "hold a key before launch"). rEFInd's documented key behaviour is only for the `timeout -1` case, so its countdown behaviour for positive timeouts is undocumented in the page read. Windows arrow-key behaviour rests on a Windows 2000-era source and is UNVERIFIED for current Windows. None of the sources checked describe resetting the countdown to full length on navigation.

### Cited Findings
- GRUB (`GRUB_TIMEOUT_STYLE` unset or `menu`): "Pressing a key interrupts the timeout." The menu is shown first, then the default entry boots after the timeout unless a key was pressed — [GNU GRUB manual](https://www.gnu.org/software/grub/manual/grub/grub.html)
- GRUB (`countdown` or `hidden`): ESC, F4, or holding SHIFT during the wait shows the menu and waits for input; pressing a menu entry's hotkey boots that entry immediately; if the timeout expires first the default boots — [GNU GRUB manual](https://www.gnu.org/software/grub/manual/grub/grub.html)
- systemd-boot: "the menu can be shown by pressing and holding a key before systemd-boot is launched." The page does **not** describe what a key press during the countdown does — [loader.conf(5)](https://www.freedesktop.org/software/systemd/man/latest/loader.conf.html). Its menu key bindings (`+`/`t`, `-`/`T`, `d`/`D` for default) adjust the timeout or default — [systemd-boot(7)](https://www.freedesktop.org/software/systemd/man/latest/systemd-boot.html). The claim "any key stops the countdown" from the brief is **not confirmed** by these pages.
- rEFInd (`timeout -1`): "if a shortcut key (for instance, W for Windows) is pressed, that system will boot instead"; "If any other key is pressed, the menu will show with no timeout." This is the only keypress behaviour documented, and it applies to `-1` only — [rEFInd configuration file reference](https://www.rodsbooks.com/refind/configfile.html)
- Windows: the arrow-key claim comes from an old Windows 2000 guide ("pressing an arrow key ... stops the countdown") — [O'Reilly, Windows 2000 Quick Reference](https://oreilly.com/library/view/windows-2000-quick/0596000170/ch01s08.html). Not confirmed for Windows 10/11. The ElevenForum and Windows Central pages were not readable.
- Other loaders (for context): a FreeBSD bug report says an arrow key at the loader dropped to the prompt instead of stopping the timer (the reporter recalled older behaviour as "stopping the timer"); a Libreboot/U-Boot patch disables the timer once an arrow key navigates the menu — [FreeBSD Bugzilla 254787](https://bugs.freebsd.org/bugzilla/show_bug.cgi?id=254787); [Libreboot U-Boot patch 0004](https://browse.libreboot.org/lbmk.git/tree/config/u-boot/x86/patches/0004-Support-auto-boot-timeout-delay-bootflow-menu.patch). Syslinux's Tab key halts the countdown, and a Fedora GRUB patch makes F8 or holding Shift interrupt it — [Fedora grub2 patch 0186](https://forgejoroute-communishift-forgejo.apps.fedora.cj14.p1.openshiftapps.com/rpms/grub2/src/branch/f30/0186-Accept-ESC-F8-and-holding-SHIFT-as-user-interrupt-ke.patch). These are search-snippet results, not fetched in full.

### Inferences
- In GRUB, the countdown is stopped by any interrupt key, and then the menu stays open until the user chooses. Navigation is therefore a "stop", not a "reset".
- Holding a key is the only documented way to force the menu in systemd-boot, so a key press that merely navigates would not be a documented countdown control there.

### Gaps
- systemd-boot: in-countdown key behaviour not documented in the fetched man pages. The ArchWiki page (which would cover it) returned HTTP 403 / Anubis block.
- rEFInd: keypress behaviour during a positive timeout not documented in the page read.
- Windows 10/11: current arrow-key behaviour not confirmed.
- No source found showing that a countdown is reset to full length on navigation in any of the five managers.

## Key Question 3: On timeout, does the default entry or the highlighted entry boot?

### Takeaway
Every manager with a documented timeout boots its configured default, not the highlighted entry. The manuals checked do not describe booting the highlighted entry on timeout. rEFInd's `default_selection` falls back to the last-booted entry when unset, which is still a stored default rather than the highlight. The only documented timeout action besides booting is rEFInd's optional shutdown.

### Cited Findings
- GRUB: "Boot the default entry this many seconds after the menu is displayed, unless a key is pressed." — [GNU GRUB manual](https://www.gnu.org/software/grub/manual/grub/grub.html)
- systemd-boot: `timeout` is "How long the boot menu should be shown before the default entry is booted"; `default` is a glob pattern selecting an entry by id, and `@saved` stores the last chosen entry as an EFI variable "automatically selected the next time the boot loader starts" — [systemd-boot(7)](https://www.freedesktop.org/software/systemd/man/latest/systemd-boot.html), [loader.conf(5)](https://www.freedesktop.org/software/systemd/man/latest/loader.conf.html)
- Windows: "the time to wait ... before the boot manager selects a default entry" — [Microsoft Learn, BCDEdit /timeout](https://learn.microsoft.com/en-us/windows-hardware/drivers/devtest/bcdedit--timeout). For the default itself, Microsoft points to `bcdedit /? default`, which was not fetched.
- rEFInd: "rEFInd boots the option set via default_selection when the timeout period is reached." If `default_selection` is unset: "rEFInd attempts to boot the previously-booted entry, or the first entry if there's no record" — [rEFInd configuration file reference](https://www.rodsbooks.com/refind/configfile.html)
- rEFInd `shutdown_after_timeout`: with it set to true, "rEFInd attempts to shut down the computer when the timeout period is reached." — [rEFInd configuration file reference](https://www.rodsbooks.com/refind/configfile.html)
- GRUB hotkeys (`countdown`/`hidden` styles) boot a chosen entry immediately, but this is a user keypress, not a timeout — [GNU GRUB manual](https://www.gnu.org/software/grub/manual/grub/grub.html)
- rEFInd `timeout -1` with a shortcut key: that system boots. This is a keypress, not a timeout — [rEFInd configuration file reference](https://www.rodsbooks.com/refind/configfile.html)

### Inferences
- None of the fetched timeout descriptions mention the highlighted entry, so timeout uses the configured default. "Highlight boots on timeout" is not a documented behaviour in these five managers.
- A manager that boots the highlighted non-default entry on timeout was not found. Rephrased: no source checked documents one. This is a gap, not a proof of absence.

### Gaps
- Windows: the default boot entry rule (`bcdedit /default`) was not fetched.
- systemd-boot: whether `default` can name a non-default entry on timeout is implied by the glob mechanism but not explicitly stated.

## Key Question 4: How do managers avoid auto-triggering destructive or disruptive entries?

### Takeaway
The fetched documentation for these five managers describes no safety mechanism for default entries (no reboot, power-off, firmware-setup, or shell guard). The only disruptive timeout action found is rEFInd's optional shutdown, which is an explicit opt-in (`shutdown_after_timeout`). Where a default is set, the sources treat it as a user-chosen entry, not an entry the manager checks.

### Cited Findings
- rEFInd `shutdown_after_timeout` is a documented option that powers off the machine at timeout; it is not default-on — [rEFInd configuration file reference](https://www.rodsbooks.com/refind/configfile.html)
- systemd-boot `default` supports `@saved`, which stores the last user choice on every boot. A user who selected a power-off or firmware entry would have it auto-selected later, which the manual does not warn about — [loader.conf(5)](https://www.freedesktop.org/software/systemd/man/latest/loader.conf.html)

### Inferences
- The only protection in these docs is user control: the default is configured by the user or the installer, and the manager does not second-guess it. This is an inference from the absence of any guard in the manuals.

### Gaps
- No source found on whether reboot, power-off, firmware-setup, or shell entries are ever the default in practice, or whether any manager filters them out. Not checked: the GRUB entry generator (grub-mkconfig) and the systemd-boot/Windows installer defaults.
- Apple: no documentation on this topic was fetched.

## Key Question 5: Should arrow keys reset the countdown to full length?

### Takeaway
No source checked recommends or documents resetting the countdown to full length on navigation. The documented patterns are: a key stops or interrupts the timer (GRUB, FreeBSD and Libreboot reports), or navigation disables the timer until the user confirms (Libreboot patch). The Windows arrow-key stop behaviour is UNVERIFIED for current versions.

### Cited Findings
- GRUB: "Pressing a key interrupts the timeout." — [GNU GRUB manual](https://www.gnu.org/software/grub/manual/grub/grub.html)
- Libreboot/U-Boot patch: an arrow key disables the timer and the user then presses Enter — [Libreboot patch 0004](https://browse.libreboot.org/lbmk.git/tree/config/u-boot/x86/patches/0004-Support-auto-boot-timeout-delay-bootflow-menu.patch)
- Windows (old source): arrow key stops the countdown — [O'Reilly Windows 2000 Quick Reference](https://oreilly.com/library/view/windows-2000-quick/0596000170/ch01s08.html) (UNVERIFIED for current Windows)

### Inferences
- Stopping the timer on any navigation (not resetting it) is the common pattern. Resetting to full length would be a new design, not an observed one. That is an inference; no manager was found doing it.
- For an accessibility-minded design, stopping the timer on the first key press (and not restarting it) is what the documented managers do. Whether that is "better" for users with slower reactions is not established by any source here.

### Gaps
- No user-research or accessibility source on timeout length or reset behaviour was found.
- Current Windows 10/11 arrow-key behaviour not confirmed.

## Source Access Log
- Blocked or unreadable in this session: ArchWiki systemd-boot (HTTP 403 / Anubis block, twice); GNU GRUB `timeout` command subpage and later offsets (HTTP 429, and the excerpt did not reach the `timeout` command entry); ElevenForum (HTTP 403); Windows Central (truncated); Apple Support HT102612 (wrong article, iPad charging page).
- Not fetched: Microsoft `bcdedit /? default` page; GRUB `timeout` command entry text; ArchWiki rEFInd page.
