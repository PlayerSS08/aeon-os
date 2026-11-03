# Minimal Boot Splash Design for a UEFI Hobby OS

Research date: 2026-11-03. Method: web search plus fetch. Fetch results are a small model's answer to my prompt, not verbatim pages, unless quoted. Where a claim rests on a search snippet it is marked "(snippet)". Many primary sites returned 403/404 (noted in Gaps). Treat "Inferences" as my reasoning, not sourced fact.

---

## Q1. What do macOS, Windows, Plymouth, ChromeOS and Android show at boot?

### Takeaway
Apple's support page describes the classic sequence: logo first, then a progress bar, possibly alternating with blank screens. Plymouth's `spinner` and `bgrt` themes are the closest Linux analogues, and Android boot animations are frame sequences in a zip.

### Cited Findings
- Apple: the Mac shows the Apple logo "when it finds your built-in or external startup disk"; "as startup continues, you should see a progress bar, with or without the Apple logo"; "this screen might alternate with a blank screen several times" — [Apple Support, "If your Mac starts up to an Apple logo or progress bar"](https://support.apple.com/en-us/102535) (fetched, tool-summarised)
- Apple: during a macOS update or upgrade the logo or bar "can stay up much longer than usual" and the bar may pause for long stretches — [Apple Support](https://support.apple.com/en-us/102535) (fetched, tool-summarised)
- Apple: startup is a problem when it "never progresses beyond an Apple logo or progress bar"; the Mac "might need service" — [Apple Support](https://support.apple.com/en-us/102535) (fetched, tool-summarised)
- Windows: Microsoft replaced the Windows 8-era loading animation with a newer one, applied to boot, sign-in, sign-out, restart and shutdown screens. Per Pureinfotech it is hidden and enabled with ViVeTool; WinCentral reports the change without a confirmed public date — [Pureinfotech](https://pureinfotech.com/windows-11-new-spinner-replaces-windows-8-loading-animation/) (snippet); [WinCentral](https://thewincentral.com/windows-new-spinner-design-update/) (snippet)
- Windows crash screen: a black BSoD was tested in 2021 and reverted to blue; Insider builds reportedly move the crash screen to black — [Gigazine](https://gigazine.net/gsc_news/en/20210702-windows-11-blue-screen-become-black) (snippet); [Pureinfotech](https://pureinfotech.com/windows-11-blue-screen-death-black-screen/) (snippet). This is the crash screen, not the boot logo.
- Plymouth: the Arch wiki describes `bgrt` as a variant of the Spinner theme that keeps the vendor logo when firmware provides one; `bgrt-fallback.png` in the spinner theme directory is the fallback when no vendor logo exists — [Arch Wiki, Plymouth](https://wiki.archlinux.org/title/Plymouth) (snippet)
- Plymouth: `plymouth-set-default-theme` sets the default theme and regenerates the initrd (`-R`), because Plymouth runs from the initramfs before root is mounted. The `splash` kernel parameter must be present — [mankier, plymouth-set-default-theme](https://www.mankier.com/1/plymouth-set-default-theme) (snippet)
- Android: `bootanimation.zip` holds `desc.txt` plus part directories of sequentially named PNG frames (`part000.png`, ...). Line 1 of `desc.txt` gives width, height, fps and an optional progress flag. Each part line is `TYPE COUNT PAUSE PATH [#RGBHEX [CLOCK1 [CLOCK2]]]`, with TYPE `p`, `c` or `f`. Optional `trim.txt` and `audio.wav` per part — [AOSP bootanimation FORMAT.md](https://android.googlesource.com/platform/frameworks/base/+/master/cmds/bootanimation/FORMAT.md) (snippet; several AOSP revisions exist and differ)
- Android: the zip is stored uncompressed because PNG is already compressed — [AOSP FORMAT.md](https://android.googlesource.com/platform/frameworks/base/+/master/cmds/bootanimation/FORMAT.md) (snippet)

### Inferences
- Across the sourced systems, the pattern is a dark or black field, one small mark or spinner, no other content, and a hand-off to the next stage. This is inferred from what each system shows, not a stated design rule.
- A progress bar looks optional, based on the Apple and Windows examples.

### Gaps
- No source found for the common "centred, X% of screen, no text, fade duration, contrast ratio" rules. These must come from design practice or measurement.
- Whether Apple's Intel Macs draw a progress bar differently from Apple Silicon was not confirmed by any page I read.
- Windows boot: a non-authoritative guide says the boot phase is "typically" black with the Windows logo and spinning dots, controlled by Boot Manager and kernel. Not confirmed by Microsoft; not used as a finding.
- ChromeOS: no source retrieved.
- The Plymouth `bgrt` and `spinner` script logic (how they position and fade) was not read; the freedesktop wiki and GitHub pages returned 403.

---

## Q2. ACPI BGRT: reading the firmware logo, and which OSes use it

### Takeaway
BGRT is an ACPI table that points to the BMP the firmware drew at boot, with its screen offsets and a status byte. The OS can map that image and draw it at the same position to continue the vendor logo; Linux exposes it through sysfs and Plymouth's `bgrt` theme uses it.

### Cited Findings
- Table layout, byte offsets from the start of the table: Signature "BGRT" (0x00, 4 bytes); Length (0x04); Revision (0x08); Checksum (0x09); OEM ID (0x0a, 6); OEM Table ID (0x10, 8); OEM Revision (0x18); Creator ID (0x1c); Creator Revision (0x20); **Version ID, must be 1** (0x24, 2 bytes); **Status** (0x26, 1); **Image type** (0x27, 1); **Image address** (0x28, 8, physical); **Image X offset** (0x30, 4); **Image Y offset** (0x34, 4) — [osdev.wiki, BGRT](https://osdev.wiki/wiki/BGRT) (fetched, tool-summarised; the table matches the struct below)
- Status bits: bit 0 "Displayed" (1 = boot image displayed); bits 1–2 orientation offset (00 = 0°, 01 = 90°, 10 = 180°, 11 = 270°, clockwise); bits 3–7 reserved, must be 0 — [osdev.wiki, BGRT](https://osdev.wiki/wiki/BGRT) (fetched, tool-summarised)
- Image type 0x00 = bitmap; 1–255 reserved — [osdev.wiki, BGRT](https://osdev.wiki/wiki/BGRT) (fetched, tool-summarised)
- Rust struct matching the same layout, with the same comments on version ("This value must be 1") and status bits — [raw-acpi bgrt.rs](https://docs.rs/raw-acpi/0.0.2/src/raw_acpi/bgrt.rs.html) (fetched, tool-summarised)
- ACPI 5.0 introduced BGRT — [Linux sysfs ABI, sysfs-firmware-acpi](https://grok.openbmc.org/xref/openbmc/linux/Documentation/ABI/testing/sysfs-firmware-acpi?r=d1ff4b1c) (snippet; the doc itself is in `Documentation/ABI/testing/sysfs-firmware-acpi`)
- Linux sysfs under `/sys/firmware/acpi/bgrt/`: `image` is the bitmap, currently 32-bit BMP; `status` is 1 if valid, 0 if firmware invalidated it; `type` 0 = BMP; `version` currently 1; `xoffset` and `yoffset` are pixels from the screen's left and top edges to the image's edges — same ABI doc (snippet, several mirrors agree). The original support was added in 2012 by Matthew Garrett (snippet, same mirrors).
- Linux driver `drivers/acpi/bgrt.c`: `bgrt_init()` returns `-ENODEV` if `image_address` is zero, maps the image with `memremap(..., MEMREMAP_WB)`, and logs "Ignoring BGRT: failed to map image memory" on failure — [OpenBMC mirror of bgrt.c](https://grok.openbmc.org/xref/openbmc/linux/drivers/acpi/bgrt.c?r=95e9fd10) (fetched, tool-summarised). Version, status and image-type checks were not in that excerpt; they may be in the EFI side (`efi_bgrt_init`), which I did not read.
- Plymouth `bgrt` theme uses the firmware image (see Q1). An AUR comment (2019, single user) reports the theme failed after upstream removed images from its directory — [AUR, plymouth-theme-bgrt-better-luks](https://aur.archlinux.org/packages/plymouth-theme-bgrt-better-luks) (snippet; low confidence)

### Inferences
- Minimal OS-side reader: find the table with signature "BGRT"; require version == 1; require status bit 0 == 1; require image type == 0; reject a zero image address; map the address; parse the BMP header for width, height and bpp; draw at (X, Y), rotated per status bits 1–2 if needed. The offsets above are verified against the osdev.wiki table and the struct, and they agree with a 36-byte ACPI header.
- Copying the image into your own memory early, before the firmware's memory is reclaimed, avoids depending on how long that mapping stays valid. This is inference; the Linux code maps it with `memremap`, which suggests the region is kept, but I did not verify the memory-type rules for UEFI boot services.

### Gaps
- Which other OSes use BGRT beyond Linux/Plymouth was not verified. Windows BGRT use was not searched.
- ACPI spec HTML (uefi.org) returned 403; the spec text itself was not read. The osdev.wiki table is the cited layout.
- The Linux validation checks (version, status, image type) were not confirmed from the driver source I could read.

---

## Q3. Timing, and avoiding flicker between firmware, bootloader and kernel

### Takeaway
I found no source that gives a recommended splash duration or a minimum display time. The sourced points are: Plymouth starts early from the initramfs, the BGRT ABI exists for splash continuity, and a Limine user saw slow splash rendering on a high-resolution display.

### Cited Findings
- Plymouth runs from the initramfs before root is mounted — [mankier, plymouth-set-default-theme](https://www.mankier.com/1/plymouth-set-default-theme) (snippet)
- BGRT's ABI description says it lets the OS obtain the firmware boot splash so that splash applications can avoid jarring screen changes during boot — [Linux sysfs ABI, sysfs-firmware-acpi](https://grok.openbmc.org/xref/openbmc/linux/Documentation/ABI/testing/sysfs-firmware-acpi?r=d1ff4b1c) (snippet)
- Limine: a CachyOS forum thread reports slow splash loading on a 2560x1600 display on newer hardware. A reply suggests a tiled wallpaper matching the resolution exactly; the reply was self-described as AI-generated, and the original poster said lowering the interface resolution had no effect — [CachyOS forum](https://discuss.cachyos.org/t/limine-bootloader-splash-image-loads-slowly-on-newer-hardware/23898) (anecdotal)
- Limine: wallpaper formats are BMP, PNG, JPEG and QOI; `wallpaper_style` is tiled, centered or stretched (default stretched); `interface_resolution` affects only the Limine UI, not the booted OS; `RESOLUTION` sets a framebuffer mode requested by the kernel, width x height x bpp, default bpp 32 — [Limine CONFIG.md](https://github.com/limine-bootloader/limine/blob/trunk/CONFIG.md) (snippet). A 2022 blog reports a mode switch after the initramfs handoff, fixed by `RESOLUTION=800x600` — [bkhome.org](https://bkhome.org/news/202209/limine-resolution-fix-in-initrd.html) (snippet; anecdotal)
- Apple: logo or bar may persist much longer during updates; normal startup can alternate with blank screens — [Apple Support](https://support.apple.com/en-us/102535) (fetched, tool-summarised)

### Inferences
- A mode switch between bootloader and kernel is a likely flicker source; the Limine blog is one concrete case. Keeping one framebuffer mode from firmware GOP through to the kernel, and setting it once, is the obvious remedy. This is my reasoning, not a documented best practice.
- A 1-second minimum is a design choice, not a standard. For a hobby OS, show the splash for as long as early init takes and avoid artificial waits, unless a fade needs the time.
- Apple's "alternating blank screen" wording suggests a continuous image is not guaranteed. Match position and colour where you can; do not depend on pixel-perfect continuity.

### Gaps
- No source for a recommended splash duration.
- No source on gamma-correct fades or avoiding a black frame between stages.
- Prior art for flicker-free hand-off (Fedora's flicker-free boot work, GRUB `gfxpayload=keep`) was not searched.
- Limine's wallpaper option name differs between versions; check the version in use.

---

## Q4. Practical implementation in a no_std Rust UEFI binary

### Takeaway
Two no_std BMP/PNG routes exist: `tinybmp` draws BMP onto embedded-graphics targets, and `minipng` decodes PNG without dependencies. Image sizes and decode costs were not measured.

### Cited Findings
- `minipng`: tiny PNG decoder with no dependencies beyond core; no panics as a stated goal; zero-clause BSD licence. Decodes valid non-interlaced PNGs; interlaced files are not covered — [lib.rs, minipng](https://lib.rs/crates/minipng) (snippet)
- `embedded-png` 0.1.1: no_std PNG rendering implementing `ImageDrawable` — [docs.rs, embedded-png](https://docs.rs/crate/embedded-png/latest/source/src/lib.rs) (snippet)
- `png-decoder` 0.2.0: pure-Rust no_std PNG decoder, outputs RGBA8. Not updated since 2022 per the search summary — [docs.rs, png-decoder](https://docs.rs/crate/png-decoder/latest) (snippet)
- `tinybmp` 0.7.0 (released Jan 2026 per lib.rs): small BMP parser for embedded no_std. Usage: `Bmp::<Rgb888>::from_slice`, then `Image::new(&bmp, Point::new(...))`. `RawBmp` exposes header and colour table. README says `pixel()` returns `None` for RLE4/RLE8 indexed bitmaps; this may be dated. Minimum Rust 1.81 on docs.rs — [tinybmp repo](https://github.com/embedded-graphics/tinybmp); [docs.rs, tinybmp](https://docs.rs/tinybmp/latest/tinybmp/index.html) (snippet)
- The Linux ABI says the firmware's BGRT image is a 32-bit BMP; Limine accepts BMP, PNG, JPEG, QOI — [sysfs-firmware-acpi ABI](https://grok.openbmc.org/xref/openbmc/linux/Documentation/ABI/testing/sysfs-firmware-acpi?r=d1ff4b1c) (snippet); [Limine CONFIG.md](https://github.com/limine-bootloader/limine/blob/trunk/CONFIG.md) (snippet)

### Inferences (my reasoning, not cited)
- **Format.** A BMP drawn with `tinybmp` avoids a PNG decoder, and BGRT is also BMP, so one code path can draw both the firmware logo and your own mark. For a logo drawn once, BMP is the simpler path.
- **Build-time conversion.** Rasterise the SVG at build time (for example with `resvg` in `build.rs`) and embed with `include_bytes!`. Store one 8-bit alpha mask plus a foreground colour; the fade is then one multiply per pixel. Confirm the rasteriser is build-only.
- **Blending.** If the framebuffer is sRGB, blending in linear light is more correct but costs conversions. On a dark background with a light mark, plain sRGB blending is cheaper and usually acceptable; decide on real hardware.
- **Scaling.** Scale by screen height over a reference height (for example 1080). Prefer integer multiples to avoid resampling. Test at 1920x1080 and 3840x2160.
- **Size arithmetic (not measured).** A 512x512 RGBA region is 1 MiB raw. A 256x256 8-bit alpha mask is 64 KiB raw. A flat-colour logo as PNG is much smaller than either, but measure your mark before deciding.
- **Spinner or bar.** A bar is one rectangle redrawn per tick. A spinner is a few precomputed alpha masks cycled per frame, so cost is per frame.

### Gaps
- No measured byte sizes for any real logo.
- No decode-speed benchmark for `minipng` or `tinybmp` on UEFI hardware.
- No source on sRGB/linear fade correctness for boot splashes.
- `tinybmp` RLE limitation may be out of date; check the 0.7.0 source.
- Crate versions come from search snippets, not Cargo.toml. Re-check before adding dependencies.

---

## Q5. Accessibility, safety and recovery

### Takeaway
I found no source on splash accessibility. The sourced recovery facts are that Plymouth needs `splash` on the kernel line (otherwise kernel text shows), and Apple says a stuck startup may need service.

### Cited Findings
- Plymouth shows only when `splash` is on the kernel command line — [mankier, plymouth-set-default-theme](https://www.mankier.com/1/plymouth-set-default-theme) (snippet)
- Apple: if startup never gets past the logo or bar, the Mac "might need service" — [Apple Support](https://support.apple.com/en-us/102535) (fetched, tool-summarised)
- BGRT's status bit 0 tells the OS whether a firmware image was displayed — [osdev.wiki, BGRT](https://osdev.wiki/wiki/BGRT) (fetched, tool-summarised)

### Inferences
- Keep a verbose key (for example Esc or F2 during boot) that bypasses the splash and prints kernel text. Otherwise a hang on the splash is invisible.
- Make the splash non-blocking. If it fails to initialise, fall through to text. Never let the splash own the only copy of an error.
- Bound the splash with a timeout. If early init has not finished in time, hand the screen to the text console.
- Community guides (not Apple) suggest force restart, NVRAM reset and safe mode for a stuck Mac; not used as findings.
- Keep the mark's contrast high enough for a dim monitor. No source gives a ratio; WCAG was not checked.

### Gaps
- No accessibility guidance for boot screens (WCAG for non-text graphics, motion sensitivity for spinners).
- Apple's Human Interface Guidelines and Apple developer boot guidance were not checked.

---

### Source access notes (applies across all questions)

- Returned 403: freedesktop Plymouth wiki and Plymouth bgrt GitHub page; uefi.org ACPI 6.5 spec HTML; OSDev wiki (wiki.OSDev.org, the osdev.wiki mirror worked).
- Returned 404: kernel.org ABI page path I guessed; the OpenBMC sysfs ABI link worked via search.
- Not searched in this pass: Windows BGRT use on learn.microsoft.com; ChromeOS splash (frecon); Fedora flicker-free boot; GRUB `gfxpayload=keep`.
- Not measured: logo byte sizes, decode times, fade and timing.
