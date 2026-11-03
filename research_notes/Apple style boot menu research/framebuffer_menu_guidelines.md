# Framebuffer boot menu guidelines (UEFI GOP, 1024x768 to 3840x2160)

Research status: as of 2026-11-03. About 20 tool calls. Primary sources were fetched where noted. Anything marked UNVERIFIED comes from my own reasoning or from search snippets only. Nothing in this file is a fetched statement about text sizes, layout or countdown UX, because no source for those was found.

## Text sizes at 1080p and 4K (menu item, description, title)

### Takeaway
No bootloader documents pixel sizes for a menu at 1080p or 4K. The only source-backed numbers are platform point sizes from Apple and the WCAG large-text threshold. Use a font-scale rule in your own code and pick the pixel values by testing on real panels.

### Cited Findings
- Apple HIG typography gives default and minimum sizes in points: iOS/iPadOS 17/11 pt, macOS 13/10 pt, tvOS 29/23 pt, visionOS 17/12 pt, watchOS 16/12 pt. It says "Use font sizes that most people can read easily" and that thin weights need larger sizes. Source: [Apple HIG Typography](https://developer.apple.com/design/human-interface-guidelines/typography.md)
- WCAG 2.2 treats text as large at 18 pt, or 14 pt bold. The page converts this to about 24px and 18.5px at 1pt = 1.333px, which is a CSS-pixel convention, not a physical measure. Source: [WCAG 2.2 Understanding SC 1.4.3](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum)
- Limine has a `term_font_size` option (glyph size in dots, must match the font file), a `term_font_scale` option (x/y scaling; unset means scaling follows resolution), and `term_font_spacing`. Its `interface_resolution` option sets the resolution for the Limine UI only, not the booted OS. Source: [Limine CONFIG.md](https://github.com/limine-bootloader/limine/blob/trunk/CONFIG.md)
- systemd-boot has no font or size option in `loader.conf`. It only sets the console mode (`console-mode`: 0 = 80x25, 1 = 80x50, 2 = first non-standard firmware mode, `auto`, `max`, `keep` default). Source: [loader.conf(5) via mankier](https://www.mankier.com/5/loader.conf)
- rEFInd uses a PNG font file (`font` token) with alpha transparency. The default is the built-in Luxi Mono 12 pt. The font size is fixed when the PNG is generated, not in refind.conf. Icon sizes are set with `big_icon_size` and `small_icon_size`, and icons that do not match are scaled. Source: [rEFInd refind.conf sample](https://code.delx.au/refind/blob/HEAD:/refind.conf-sample) (search snippet only; the exact default values were not verified against the file)
- GRUB: a 2022 mailing-list patch proposed a `gfxterm_scale` variable for gfxterm HiDPI scaling, where 0 means auto on high-resolution displays and 1 disables scaling. Source: [grub-devel 2022-07 patch thread](https://lists.gnu.org/archive/html/grub-devel/2022-07/msg00028.html) (search snippet; the released manual was not confirmed to contain it). The older workaround was a larger font made with `grub-mkfont`.

### Inferences
- Use Apple's minimum point sizes as a floor, not a target. A 1080p menu (1920x1080) on a 27-inch panel is a usable baseline, and 4K needs about 2x the pixel size of a 1080p menu to look the same.
- A safe engineering rule is: the item font height = max(16 px, round(vertical_resolution / 45)). This gives about 24 px at 1080p and about 48 px at 2160p. UNVERIFIED: it is my estimate, not a sourced figure. Check it on a real 1080p and a 4K screen.
- Title roughly 1.5x the item height, description roughly 0.8x, both as UNVERIFIED design estimates.

### Gaps
- No bootloader documents recommended pixel sizes for a menu at 1080p or 4K. Limine and rEFInd expose scaling options, but none of them give a default target size.
- rEFInd HiDPI: no dedicated HiDPI or scale option was found. The search returned only icon and font file settings. Unverified whether one exists.
- GRUB `gfxterm_scale`: the fetched manual was truncated and did not show the entry. Unverified in the released manual.

## Minimum contrast (WCAG 2.2) and how to compute and test it

### Takeaway
Normal text needs 4.5:1, and large text (18 pt, or 14 pt bold) needs 3:1. The contrast ratio is (L1 + 0.05) / (L2 + 0.05), and the result must not be rounded before the comparison. Test this with a fixed integer or fixed-point function in unit tests.

### Cited Findings
- SC 1.4.3 requires "a contrast ratio of at least 4.5:1" for text, and "at least 3:1" for large-scale text, which is "at least 18 point or 14 point bold". Source: [WCAG 2.2 Understanding SC 1.4.3](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum)
- The same page says the ratio "should not be rounded", so 4.499:1 fails 4.5:1. Source: same URL.
- Exempt text: inactive UI components, pure decoration, text not visible to anyone, and text that is part of a logo or brand name. Source: same URL.
- The contrast formula appears as (L1 + 0.05) / (L2 + 0.05). The fetched page was truncated before the glossary, so the luminance definition could not be quoted. Source: [WCAG 2.2](https://www.w3.org/TR/WCAG22/)
- Search snippets pointed to a 5:1 value in two 2007 W3C drafts. Those are old drafts and do not apply to WCAG 2.2. Source: [W3C 2007 WD](https://www.w3.org/TR/2007/WD-UNDERSTANDING-WCAG20-20071211/visual-audio-contrast-contrast.html)

### Inferences
- Relative luminance (from the sRGB transfer function, using the 0.2126 / 0.7152 / 0.0722 weights) is standard WCAG practice. UNVERIFIED for this note: the glossary was not fetched, so I am stating it from memory. Check the WCAG 2.2 glossary before relying on it.
- Integer method for unit tests: store the 8-bit channels, linearise with a 256-entry lookup table in fixed-point (for example, 16-bit scaled), compute luminance as 2126*R + 7152*G + 722*B on the 10000 scale, then compare (L1 + 500) * 10 >= 45 * (L2 + 500) to avoid rounding issues. The integer scaling is my design, not sourced.
- Hand-check value (my calculation, to verify in tests): #767676 on #FFFFFF gives about 4.54:1, which passes 4.5:1 only just. Black on white gives 21:1. A test that asserts 4.54 for #767676 on white would catch a rounding bug.
- Put the test colours (text, background, highlight background, selected text) in a table and assert each pair is at least 4.5:1 for normal text.

### Gaps
- The WCAG 2.2 glossary (relative luminance definition, linearisation threshold) was not fetched. The formula above is partly from memory.
- No source for text on a dark framebuffer background under an sRGB-to-linear assumption on a UEFI GOP display. Gamma and firmware colour handling are unverified.

## Layout: centring, width, row height, description line, footer, whitespace

### Takeaway
No source was found for this topic. The recommendations below are my design inferences.

### Cited Findings
- None. The systemd-boot, rEFInd, GRUB and Limine documentation fetched here covers configuration options only. None describes centring, list width or row height.
- Limine's `term_margin` (pixels around the terminal) and `term_margin_gradient` are the only layout-related settings found. Source: [Limine CONFIG.md](https://github.com/limine-bootloader/limine/blob/trunk/CONFIG.md)

### Inferences (UNVERIFIED)
- Centre the block vertically, with the list starting near the vertical middle, not at the top edge.
- Cap the list width at about 60% of the screen width, so long entry names do not stretch across a 4K display.
- Row height = 1.6 to 2.0 times the item font height, with consistent spacing between rows.
- One description line for the highlighted entry, placed directly under the list.
- Footer line at the bottom edge, with the countdown text.
- Whitespace: a comfortable margin is at least 1 row height on each side. Other figures have not been sourced.

### Gaps
- No source for list width, row height, or whitespace proportions. Measure on the hardware.

## Countdown UX (plain-words countdown, highlight-linked text, arrow keys, auto-run safety)

### Takeaway
systemd-boot and Limine both show a timeout and let the user change the selection. The documentation found does not describe how a countdown should look or whether arrow keys reset it. Those choices are design decisions.

### Cited Findings
- systemd-boot: `timeout` sets how many seconds the menu is shown before the default entry boots. `menu-force` shows the menu with no timeout. A selection made in the menu overrides the configured default and is stored as an EFI variable. Source: [loader.conf(5) via mankier](https://www.mankier.com/5/loader.conf)
- systemd-boot: `editor` (default on) lets the user edit the kernel command line in the menu. The documentation recommends disabling it on machines that unauthorised people can access. Source: same URL.
- Limine: `interface_help_colour_bright` sets "the brighter accent colour used for the auto-boot countdown digit", so Limine shows the countdown digit in the menu. Source: [Limine CONFIG.md](https://github.com/limine-bootloader/limine/blob/trunk/CONFIG.md)

### Inferences (UNVERIFIED)
- Show the countdown as "Boots Aeon in 12 seconds", and update the entry name in the same line when the highlight moves, so the text always names what will start.
- Keep the countdown visible while arrow keys move the highlight.
- Restart the timer on any arrow press only if you want the user to have time to read. Stopping it on the first key press is the more common safe choice. This is a choice, not a sourced rule.
- Do not auto-run destructive entries (for example, a wipe or reset entry). Require an explicit key press for those. This follows from the systemd-boot editor warning above, but no source says so directly.

### Gaps
- No sourced guidance on whether to restart the timer on key presses.

## Kernel panic screen on a framebuffer

### Takeaway
Linux drm_panic can render a QR code on the panic screen using a Rust encoder, and the QR code carries kmsg text. Windows shows a stop code and module name on a bug check screen, and the example image shows a QR code. Apple's guidance on this topic was not found. Use a muted background and a short message.

### Cited Findings
- drm_panic QR code: the Kconfig help says the encoder is written in Rust, and the QR code can carry recent kmsg lines and other debug details. A configurable base URL decides the payload format. The default maximum QR version is 40 (177x177 modules). Source: [Kconfig, wireguard-linux tree](https://git.zx2c4.com/wireguard-linux/tree/drivers/gpu/drm/Kconfig?h=devel) and [AlmaLinux kernel Kconfig](https://git.almalinux.org/almalinux/kernel/src/branch/a9/drivers/gpu/drm/Kconfig) (search snippets)
- The Rust encoder runs in the panic handler because allocation and locking are unsafe there. Source: [LKML v3 cover letter, drm/panic QR code](https://lkml.rescloud.iu.edu/2407.1/03896.html) (search snippet)
- Fedora proposed enabling DRM_PANIC. Unsupported drivers still show the old message. Source: [Fedora Changes: Enable Drm Panic](https://www.fedoraproject.org/wiki/Changes/EnableDrmPanic) (search snippet)
- Windows: the bug check screen "displays the stop code error and also the module name of the currently executing code, when it's available". The page says "The exact appearance of the stop code screen depends on the cause". The example image shows a QR code. Source: [Microsoft Learn, Bug Checks (Stop Code Errors)](https://learn.microsoft.com/windows-hardware/drivers/debugger/bug-checks--blue-screens-) (fetched)
- Windows 11 (24H2 and later) example shows "the device encountered a problem and needs to restart", with a stop code. Source: same URL.

### Inferences
- Show: a short title (for example, "Aeon could not start"), one sentence of plain text, a hint (for example, "Power off, then try the recovery entry"), and an optional short reference code (the panic reason, not a full backtrace).
- Do not show: full register dumps, raw stack traces, or long log text on the panic screen. The drm_panic approach moves the detail into a QR code instead.
- Use a muted neutral background (for example, dark grey), not a saturated blue or red. This is an inference from Windows and Linux practice, not a sourced guideline.

### Gaps
- Microsoft's documentation on the QR code's purpose was not found in the fetched page. Third-party claims about it are unverified.
- The kernel.org drm_panic documentation page returned 404 at https://www.kernel.org/doc/html/latest/gpu/drm-panic.html. The findings above come from Kconfig and patch series only. Check the kernel tree (drivers/gpu/drm/drm_panic*.c and Documentation) for the current behaviour.
- Apple HIG coverage of panic or crash screens was not searched. Apple's HIG typography page (above) is the only Apple source found.

## Sources consulted
- [WCAG 2.2 Understanding SC 1.4.3](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum) (fetched)
- [WCAG 2.2](https://www.w3.org/TR/WCAG22/) (fetch truncated before glossary)
- [Apple HIG Typography](https://developer.apple.com/design/human-interface-guidelines/typography.md) (fetched)
- [Limine CONFIG.md](https://github.com/limine-bootloader/limine/blob/trunk/CONFIG.md) (fetched)
- [loader.conf(5) via mankier](https://www.mankier.com/5/loader.conf) (fetched; the Arch man page snippet was jumbled)
- [Microsoft Learn, Bug Checks](https://learn.microsoft.com/windows-hardware/drivers/debugger/bug-checks--blue-screens-) (fetched)
- [GRUB manual](https://www.gnu.org/software/grub/manual/grub/grub.html) (fetched; truncated, gfxterm_scale not visible)
- [grub-devel 2022-07 gfxterm HiDPI patch](https://lists.gnu.org/archive/html/grub-devel/2022-07/msg00028.html) (search snippet)
- [rEFInd refind.conf sample](https://code.delx.au/refind/blob/HEAD:/refind.conf-sample) (search snippet)
- [Kernel Kconfig, drm panic](https://git.zx2c4.com/wireguard-linux/tree/drivers/gpu/drm/Kconfig?h=devel) (search snippet)
- [LKML drm/panic QR code series](https://lkml.iu.edu/hypermail/linux/kernel/2408.1/04241.html) (search snippet)
- [Fedora Changes: Enable Drm Panic](https://www.fedoraproject.org/wiki/Changes/EnableDrmPanic) (search snippet)
