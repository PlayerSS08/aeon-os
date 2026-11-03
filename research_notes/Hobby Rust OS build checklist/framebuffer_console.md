# Framebuffer console and terminal best practice for a UEFI hobby OS (Rust, no_std)

Research as of 2026-11-03. Fetch budget was about 15 calls. Citations marked "fetched" come from WebFetch, which runs a small model over the page, so exact wording should be checked against the source before it is quoted in code or docs. Where a claim is from memory and not from a fetched page, it is labelled Inference. Known blockers: uefi.org returned HTTP 403 to WebFetch, so the UEFI spec was not read directly. The xterm ctlseqs page was truncated at character 100000 of about 188,800.

## 1. GOP mode selection (native resolution, EDID, QueryMode, firmware pitfalls, what loaders do)

### Takeaway
Linux's EFI stub picks a GOP mode by an explicit option, or with `auto` by taking the largest width times height, with no resolution cap, and it rejects only BltOnly formats. Linux's efifb docs say GOP displays work with no parameters. The rust-osdev `bootloader` framebuffer writer supports only RGB, BGR and U8 formats, and it panics on anything else, including bitmask formats. There is no authoritative source here for true native-resolution detection through EDID.

### Cited Findings
- Linux `gop.c` skips any GOP whose pixel format is `PIXEL_BLT_ONLY` or out of range. When it looks for the primary GOP, it prefers one that also has the ConOut device GUID, which is the UEFI Console Splitter setup. Otherwise it takes the first usable GOP. Fetched: [Linux gop.c](https://github.com/torvalds/linux/blob/master/drivers/firmware/efi/libstub/gop.c)
- Linux `gop.c` `auto` mode (`match_auto()`) keeps the mode with the largest `horizontal_resolution * vertical_resolution`. On an area tie it prefers the greater bits-per-pixel. It imposes no explicit resolution cap. If no valid mode exists, the match stays at mode 0. Fetched: [Linux gop.c](https://github.com/torvalds/linux/blob/master/drivers/firmware/efi/libstub/gop.c)
- Linux `gop.c` `list` mode prints all modes, marks the current one with `*` and unusable ones with `-`, waits up to 10 seconds, and then keeps the current mode. Fetched: [Linux gop.c](https://github.com/torvalds/linux/blob/master/drivers/firmware/efi/libstub/gop.c)
- Linux `gop.c` EDID handling: it tries the `EDID_ACTIVE` protocol first, then `EDID_DISCOVERED`. If the EDID is missing or under 128 bytes, the buffer is zeroed. Fetched: [Linux gop.c](https://github.com/torvalds/linux/blob/master/drivers/firmware/efi/libstub/gop.c)
- Linux `gop.c` framebuffer pitch: for bitmask formats it computes `lfb_linelength = (pixels_per_scan_line * lfb_depth) / 8`. For 8-bit RGB or BGR it assumes 32 bpp and uses `pixels_per_scan_line * 4`. It uses `PixelsPerScanLine` for the pitch, not `HorizontalResolution`. Fetched: [Linux gop.c](https://github.com/torvalds/linux/blob/master/drivers/firmware/efi/libstub/gop.c)
- Linux efifb docs: "The system must be booted via the EFI stub for this to be usable." GOP displays "should work out of the box without any special parameters." UGA displays need machine-specific parameters because there is no autodetection. Fetched: [Linux efifb docs](https://docs.kernel.org/fb/efifb.html)
- Linux efifb: the `list` option only lists modes. "A specific mode can then be chosen using one of the above options for the next boot." Mode selection is therefore a boot-time decision. Fetched: [Linux efifb docs](https://docs.kernel.org/fb/efifb.html)
- Linux efifb: the `nowc` option disables write-combined mapping because it can cause "side-effects and slowdowns on other CPU cores when large amounts of console data are written." Fetched: [Linux efifb docs](https://docs.kernel.org/fb/efifb.html)
- rust-osdev `bootloader` `common/src/framebuffer.rs` writes only `PixelFormat::Rgb`, `Bgr` and `U8`. Any other format hits the `other` arm, sets the format to Rgb, and then panics. It has no bitmask handling. Fetched: [bootloader framebuffer.rs](https://github.com/rust-osdev/bootloader/blob/main/common/src/framebuffer.rs)

### Inferences
- The ConOut-GUID preference in `gop.c` is a sign that a Console Splitter may sit on top of GOP. Choosing the GOP that carries ConOut is one way to avoid a splitter's blt-only or scaled path, but the splitter behaviour itself is not confirmed by a fetched source.
- Choosing the mode with the largest area is a reasonable default, but it can pick a mode the panel scales or pillar-boxes. Linux does not check this. Confirming native timing needs EDID preferred-timing data, and this research did not fetch an EDID spec.
- A UEFI-side loop over `QueryMode` and a pick of the largest linear format, with bitmask formats handled explicitly, is the pattern the Linux code implies. A hobby kernel can treat PixelBltOnly as "no direct framebuffer" and fall back to text output.

### Gaps
- The UEFI spec GOP chapter (QueryMode, SetMode, PixelFormat enum, EDID protocols) could not be fetched. uefi.org returned 403. The Linux code is the only primary source used here for the field semantics.
- ConSplitter behaviour, panel-scaling pitfalls, and "high-resolution slowness" in firmware: no primary source found. The only evidence is the ConOut-GUID check in `gop.c`. Forum reports are not cited.
- What rEFInd, GRUB gfxmode, and Limine do: not fetched.
- Whether SetMode is only usable before ExitBootServices: the spec text was not retrieved. Section 5 has more detail.

## 2. Performance on large (4K) framebuffers

### Takeaway
Write-combining is the key knob for direct framebuffer writes. The Linux efifb docs say WC can slow other CPU cores under heavy console output. A shadow buffer in RAM with a single copy to video memory per frame is the standard design, but no fetched source confirms it for this OS. This research did not get a source on PAT/MTRR, on `EFI_MEMORY_WC`, or on memmove versus redraw scrolling.

### Cited Findings
- 3840 × 2160 × 4 bytes = 33,177,600 bytes, which is about 31.6 MiB for one 32-bit frame. This is arithmetic, not a sourced claim.
- Linux efifb: `nowc` exists because WC mapping can cause "side-effects and slowdowns on other CPU cores when large amounts of console data are written." Fetched: [Linux efifb docs](https://docs.kernel.org/fb/efifb.html)
- rust-osdev `bootloader` framebuffer code writes pixels one at a time with `ptr::read_volatile` after each write. It does no bulk copy. Fetched: [bootloader framebuffer.rs](https://github.com/rust-osdev/bootloader/blob/main/common/src/framebuffer.rs). This is an example of the slow path, not a recommendation.

### Inferences
- Plan a RAM shadow buffer for the cell grid or pixels, then do one large copy per frame to the framebuffer. A bulk copy is much faster than per-pixel volatile writes. The benefit depends on WC or a fast memcpy, which is not verified here.
- Scrolling by memmove within the shadow buffer and then one copy to VRAM avoids reading from VRAM. Reading from framebuffer memory is normally slow, but this is not verified against a source.
- Dirty rectangles help most when only a few glyph cells change per frame, such as typing. For full-screen scrolls, redrawing the whole screen from a text buffer may be simpler.
- Memory for a shadow buffer must be allocated before ExitBootServices, or from a pool the OS owns afterwards. That is a boot-time allocation plan.

### Gaps
- PAT/MTRR and the UEFI memory-map `EFI_MEMORY_WC` attribute: no source fetched. Check the UEFI spec memory map section and the Linux x86 PAT doc.
- Whether a byte-wise memcpy in a no_std kernel makes 4K scrolling too slow: no source. Treat it as unknown until measured.
- Linux fbcon scroll strategy (hardware copy vs redraw): the fbcon docs fetched do not cover scrolling. See Section 4.

## 3. Fonts and scaling (bitmap vs rasterised, HiDPI rules, PSF, Unicode)

### Takeaway
`noto-sans-mono-bitmap` is a no_std, allocation-free set of pre-rasterised glyphs with three weights and fixed raster heights. Linux picks its default console font with a score that favours fewer cells on high-resolution screens, which gives a usable rule of thumb. The rust-osdev bootloader uses a fixed 16 px Regular font with no scaling step and falls back to U+FFFD for missing glyphs.

### Cited Findings
- `noto-sans-mono-bitmap` 0.4.0 (as shown on docs.rs at fetch time): raster heights of 16, 20, 24 and 32 px are optional build-time features. Weights are Light, Normal and Bold. Coverage is "most important symbols, numbers, and letters as pre-rasterized constant. Unicode-ranges are selectable." The crate is `no_std` and "needs no allocations or floating point operations." Licence: MIT for the crate, SIL OFL for the font. Fetched: [docs.rs noto-sans-mono-bitmap](https://docs.rs/noto-sans-mono-bitmap/latest/noto_sans_mono_bitmap/). The fetch did not list the individual feature flags or Unicode ranges.
- Linux `lib/fonts/fonts.c` `get_default_font()` scores each built-in font. It adds 1000 when the screen is under 400 px tall and the font is 8 px or shorter, or when the screen is 400 px or taller and the font is taller than 8 px. It counts cells as `(xres / f->width) * (yres / f->height) / 1000` and applies `if (res > 20) c += 20 - res;`, which penalises fonts that produce many cells. The code comment says "prefer a bigger font for high resolution." Fetched: [Linux lib/fonts/fonts.c](https://github.com/torvalds/linux/blob/master/lib/fonts/fonts.c)
- The rust-osdev `bootloader` framebuffer writer uses `RasterHeight::Size16` with `FontWeight::Regular`. It has no scaling step. `get_char_raster` falls back to `'�'` for characters the font lacks. `LINE_SPACING = 2`, `LETTER_SPACING = 0`, `BORDER_PADDING = 1`. Fetched: [bootloader framebuffer.rs](https://github.com/rust-osdev/bootloader/blob/main/common/src/framebuffer.rs)
- Linux fbcon docs: "Note, not all drivers can handle font with widths not divisible by 8, such as vga16fb." Fetched: [Linux fbcon docs](https://docs.kernel.org/fb/fbcon.html)

### Inferences
- A HiDPI rule is not in any fetched source. The Linux rule is a cell-count heuristic in a kernel, not a DPI rule, and it does not use physical size. A scaling rule for this OS would be a design choice, such as "use the 16 px font under 1080 px high, 32 px above 2160 px high," but that threshold is invented here and should be treated as a starting guess, not a source.
- Using pre-rasterised bitmap fonts from `noto-sans-mono-bitmap` keeps the kernel no_std and allocation-free. A rasteriser (for example fontdue or ab_glyph) needs checking for no_std support first. This research did not check those crates.
- For wide Unicode (CJK, emoji), cell width must come from a width table such as `unicode-width`. This is from memory and not verified in this session.

### Gaps
- PSF (PC Screen Font) specification: no page fetched.
- `embedded-graphics` and its font support, and a no_std rasteriser: not fetched.
- A primary source for HiDPI scaling thresholds: none found. Linux's rule is the only fetched example.
- Unifont or other fallback fonts for uncovered Unicode: not fetched.
- The noto-sans-mono-bitmap feature-flag list and exact Unicode ranges: not fetched.

## 4. Terminal behaviour (wrap, cursor, backspace, ANSI/VT, scrollback, line editing, prior-art handling of wrap and resize)

### Takeaway
The `vte` crate (0.15.0 at fetch time) implements Paul Williams' ANSI/VT parser state machine, and that is a good base for escape-sequence handling. Wrap and scroll behaviour in the two framebuffer and VGA examples is simple: wrap on column overflow, and clear or shift rows at the bottom. The xterm backspace and pending-wrap rules were not confirmed, because the ctlseqs page was truncated before those sections.

### Cited Findings
- Paul Williams' parser states (fetched from vt100.net): ground, escape, escape intermediate, CSI entry, CSI param, CSI intermediate, CSI ignore, DCS entry, DCS param, DCS intermediate, DCS passthrough, DCS ignore, OSC string, SOS/PM/APC string. C0 controls are executed in ground, escape and CSI states without leaving them. CSI ignore exits on a final byte 40-7E. Fetched: [vt100.net DEC ANSI parser](https://vt100.net/emu/dec_ansi_parser)
- `vte` 0.15.0: "Parser for raw _VTE_ protocol which delegates actions to a Perform." It "is implemented according to Paul Williams' ANSI parser state machine." The docs page did not say whether the crate is no_std and did not list method names. Fetched: [docs.rs vte](https://docs.rs/vte/latest/vte/)
- xterm ctlseqs lists DEC private modes: mode 7 is "Auto-Wrap Mode (DECAWM), VT100", and mode 45 is "Reverse-wraparound mode (XTREVWRAP), _xterm_." Only the labels were fetched, not their effects. Fetched: [xterm ctlseqs](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html)
- rust-osdev `bootloader` framebuffer logger: `'\n'` calls `newline()`, which moves down one line and returns the cursor. For a normal character, if `x_pos + CHAR_RASTER_WIDTH` reaches the width, it wraps with `newline()`. At the bottom, `write_char` calls `clear()`, which erases the whole screen. It does not scroll or copy rows. Fetched: [bootloader framebuffer.rs](https://github.com/rust-osdev/bootloader/blob/main/common/src/framebuffer.rs)
- blog_os VGA text mode (`os.phil-opp.com/vga-text-mode`): wraps when the column reaches the width, and scrolls by copying rows 1 to 24 up one row, then clears the last row. Backspace, cursor, escape sequences and resize are not covered. Non-printable bytes render as `■`. Fetched: [blog_os VGA text mode](https://os.phil-opp.com/vga-text-mode/)
- Linux fbcon docs: "The framebuffer console (fbcon), as its name implies, is a text console running on top of the framebuffer device." Options include `fbcon=font:<name>`, `fbcon=margin:<color>` and `fbcon=rotate:<n>`. The docs do not cover scrolling or vt resizing. Fetched: [Linux fbcon docs](https://docs.kernel.org/fb/fbcon.html)
- Redox orbterm: the README calls it "The Orbital Terminal. Compatible with Redox and SDL2 platforms." The GitHub page is a mirror, and the README points to gitlab.redox-os.org as the primary repo. The fetch did not show the emulation dependency, wrap, resize or scrollback code. Fetched: [orbterm GitHub mirror](https://github.com/redox-os/orbterm)

### Inferences
- Plain BS (0x08) should move the cursor left, and stop at column 0 unless reverse-wraparound (DEC mode 45) is set. This is inferred from the mode names and from xterm practice in memory. The exact wording was not fetched, so verify it in ctlseqs at offset 100000 before relying on it.
- If plain BS stops at column 0, backspace across a wrapped row is the line editor's job, usually a cursor-up plus cursor-forward. That is a design decision the OS must make. It follows from the inferred BS rule, so it is inferred too.
- A deferred-wrap ("pending wrap") state, where printing in the last column sets a flag and the wrap happens on the next printable character, is the usual VT behaviour. This is from memory and not verified in this session.
- Scrollback in a framebuffer console needs a text-cell ring buffer separate from pixels. That follows from the shadow-buffer design in Section 2, not from a fetched source.
- The bootloader's clear-at-bottom and the blog_os row shift are both simple examples. Neither is a good model for a real terminal, because they lose history and do not handle escape sequences.

### Gaps
- xterm BS and pending-wrap wording: ctlseqs was truncated. Fetch it again at `offset: 100000`, or look up a specific section.
- Redox orbterm resize, reflow, wrap and scrollback code: not found in the fetch. Read `src/` and the emulation crate in the GitLab repo.
- Linux fbcon and vt scrolling and resize behaviour: the fbcon docs did not cover it. The `vt.c` source or the vt docs would be the next place.
- A no_std line editor: no search was run.
- Whether `vte` is no_std: the docs page did not show feature flags.

## 5. Boot-time resize vs runtime change (auto-resize, GOP lifetime, real GPU driver)

### Takeaway
Linux's efifb takes the framebuffer and mode from firmware at boot, and changing mode means choosing a new option on the next boot. Runtime change needs a real display driver. This research did not fetch the UEFI spec or the DRM/KMS docs, so the GOP lifetime claim and the DRM claim are not sourced here.

### Cited Findings
- Linux efifb: a mode from the `list` option takes effect only "for the next boot." Fetched: [Linux efifb docs](https://docs.kernel.org/fb/efifb.html)
- Linux efifb says GOP displays "should work out of the box without any special parameters." Fetched: [Linux efifb docs](https://docs.kernel.org/fb/efifb.html)
- The rust-osdev `bootloader` builds its writer with the framebuffer and `FrameBufferInfo` passed in at boot, and it does not change mode afterwards. Fetched: [bootloader framebuffer.rs](https://github.com/rust-osdev/bootloader/blob/main/common/src/framebuffer.rs)

### Inferences
- Auto-resize at boot is the realistic option. Read the GOP mode info, derive columns and rows from the width, height and font cell size, then allocate the shadow buffer and text grid. This is an inference from Sections 1 to 4, not a sourced rule.
- A runtime resolution change would need a real GPU or display driver with a modeset path (DRM/KMS on Linux). This is from memory, not fetched. The UEFI GOP is a boot-services protocol, so it cannot be relied on after ExitBootServices. This is from memory, and the spec text was not fetched.
- Plan the framebuffer and grid rebuild as a function of the mode info, so a later driver can call the same code.

### Gaps
- UEFI spec text saying GOP is a boot-services protocol, and that SetMode is only valid before ExitBootServices: not fetched (uefi.org 403).
- Linux simpledrm and efifb after boot (what remains of the firmware framebuffer): docs not fetched.
- DRM/KMS modeset requirements: docs not fetched.
- Real-hardware evidence of GOP behaviour after ExitBootServices: none gathered.

### Overall gaps and next steps (across all five questions)
- Fetch the UEFI spec GOP chapter by another route (a mirror, or the spec PDF), since uefi.org blocked WebFetch.
- Read the xterm ctlseqs page at offset 100000 for BS and wrap behaviour.
- Check PAT/WC in the Linux x86 PAT doc and `EFI_MEMORY_WC` in the UEFI memory map section.
- Look at Redox's orbterm `src/` on gitlab.redox-os.org for resize and wrap handling.
- Look at embedded-graphics and a no_std rasteriser (fontdue or ab_glyph) for font handling.
- Nothing was built or run. This is research only, so there are no test results to report.
