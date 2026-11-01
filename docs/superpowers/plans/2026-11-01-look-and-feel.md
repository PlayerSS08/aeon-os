# Aeon look and feel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the chunky 8x8 pixel look with a clean modern sans font, a subtle polished boot animation, an arrow-key boot menu ("Dawn") and a nicer shell, without any dependency.

**Architecture:** Fonts are rasterised offline by a Python script into a const table (8-bit coverage per glyph) that the kernel blends onto the framebuffer; layout, easing and blending maths live in host-tested pure modules (`gfont`, `anim`). The splash and menu draw into the existing `Console`; the shell keeps its character grid but uses a mono font cell generated from the same pipeline.

**Tech Stack:** Rust `no_std` UEFI, Python 3 + Pillow (build-time only, in `tools/`), QEMU+OVMF boot tests.

**Spec:** the user's answers of 2026-11-01 in this session: font "clean modern sans", animations "subtle and polished"; names Dawn (boot menu), Nexus (kernel), Pulse (shell) are suggestions, not yet approved.

## Global Constraints

- Zero dependencies at build and run time (Pillow is only used by `tools/genfont.py`, its output is committed).
- Signed image stays under 200 KB (now about 115 KB): total generated font data at most 64 KB; if a size does not fit, drop that size.
- Fonts must be redistributable: use Ubuntu Sans and Ubuntu Sans Mono (SIL OFL); keep the licence text in `docs/licenses/`.
- Never slow boot: animations add at most 400 ms in total, and none run with `quiet`.
- `clippy::undocumented_unsafe_blocks` stays denied; stage files by path; do not push.
- Existing boot tests keep passing; tests that depend on old pixel geometry are updated in the same task that changes the geometry.

## Review Focus

- Characters outside printable ASCII (log text can contain `?`-replaced bytes, UTF-8 from firmware): must draw the fallback glyph, never panic or read out of range.
- Tiny and huge screens (640x480, 1920x1080, 3840x2160): text and logo clipped or scaled, never drawn outside the framebuffer.
- Text wider than the space available: clipped at the edge, never wrapped into the next row.
- `quiet`, recovery mode and `verbose`: no animation, no splash, the plain log path still works.
- Menu with a key held down or the firmware returning no key: no busy loop, timeout still boots.

---

### Task 1: Font pipeline and `gfont` module

**Files:**
- Create: `tools/genfont.py`, `tools/fonts/` (the two source fonts), `docs/licenses/OFL-UbuntuSans.txt`, `src/fontdata.rs` (generated), `src/gfont.rs`
- Modify: `src/lib.rs` (add `pub mod gfont; pub mod fontdata;`)
- Test: unit tests in `src/gfont.rs`

**Interfaces:**
- Produces: `gfont::Face { pub size: u8, pub ascent: u8, pub height: u8 }`, `gfont::SANS_16`, `gfont::SANS_24`, `gfont::SANS_72_WORDMARK` (only A E O N), `gfont::MONO_CELL: (usize, usize)`; `gfont::glyph(face: &Face, ch: char) -> &'static Glyph` (never fails: unknown chars give the `?` glyph); `Glyph { pub advance: u8, pub w: u8, pub h: u8, pub left: i8, pub top: i8, pub cov: &'static [u8] }`; `gfont::text_width(face: &Face, s: &str) -> usize`; `gfont::blend(bg: u32, fg: u32, coverage: u8) -> u32` for packed `0x00RRGGBB`.

- [ ] **Step 1: Write failing tests** in `src/gfont.rs`: `unknown_char_gives_question_mark_glyph`, `width_is_sum_of_advances`, `empty_text_has_zero_width`, `blend_endpoints` (coverage 0 gives bg, 255 gives fg, 128 is within 1 of the average per channel), `glyph_coverage_len_matches_w_times_h` for every glyph of every face.
- [ ] **Step 2: Run** `cargo test --lib --target x86_64-pc-windows-gnu gfont`. Expected: FAIL (module missing).
- [ ] **Step 3: Implement** `tools/genfont.py` (Pillow, fixed sizes 16, 24, wordmark 72, mono cell chosen so 160 columns fit 1280 px) writing `src/fontdata.rs`; implement `gfont` over it. Remove spaces-only glyph storage (zero-length `cov`).
- [ ] **Step 4: Run** the same command. Expected: PASS. Then `cargo clippy --release -- -D warnings` clean and image size still under 200 KB (`cargo build --release`, check `aeon.efi`).
- [ ] **Step 5: Commit** `feat(ui): sans font pipeline and gfont module`.

### Task 2: Animation maths (`anim`)

**Files:**
- Create: `src/anim.rs`; Modify: `src/lib.rs`
- Test: unit tests in `src/anim.rs`

**Interfaces:**
- Produces: `anim::ease_out_cubic(t: u16) -> u16` and `anim::ease_in_out(t: u16) -> u16` on 0..=1000 (permille in, permille out); `anim::lerp(a: u32, b: u32, t_permille: u16) -> u32`; `anim::Smoother { pub fn new() -> Self; pub fn target(&mut self, permille: u32); pub fn step(&mut self, dt_us: u64) -> u32 }` which moves the shown value toward the target at a bounded speed and never overshoots or goes backwards.

- [ ] **Step 1: Write failing tests**: `easing_endpoints_and_monotonic` (0->0, 1000->1000, never decreasing, input above 1000 clamps), `lerp_endpoints_and_no_overflow` (u32::MAX ends), `smoother_never_overshoots_or_reverses`, `smoother_reaches_target_in_bounded_time`.
- [ ] **Step 2: Run** `cargo test --lib --target x86_64-pc-windows-gnu anim`. Expected: FAIL.
- [ ] **Step 3: Implement** with integer maths only.
- [ ] **Step 4: Run.** Expected: PASS; clippy clean.
- [ ] **Step 5: Commit** `feat(ui): animation easing and smoother`.

### Task 3: Smooth drawing in `Console`

**Files:**
- Modify: `src/fb.rs` (add `draw_text(&mut self, face: &gfont::Face, x: usize, y: usize, rgb: u32, s: &str) -> usize` returning the pixel width, clipped to the framebuffer; add an off-screen buffer for the splash area: `begin_frame()` / `end_frame()` that draw into memory then copy once)
- Test: `tests/boot_test.py` new `t_text_clip` (pixel check that nothing is drawn outside the splash area at 640x480)

**Interfaces:**
- Consumes: Task 1 `gfont::{Face, glyph, blend}`.
- Produces: `Console::draw_text`, `Console::begin_frame`, `Console::end_frame`.

- [ ] **Step 1:** write `t_text_clip` (boot with `mode=640x480`, screenshot, assert the left and right 8-pixel columns and the bottom row stay background). **Step 2:** run, expect FAIL (text not drawn). **Step 3:** implement; clip every glyph rectangle against `width`/`height` with checked arithmetic. **Step 4:** run the new test and the full suite. **Step 5:** commit `feat(ui): sans text drawing and frame buffer`.

### Task 4: New splash

**Files:**
- Modify: `src/main.rs` (splash and stage reporting around `bar_to`, the fade steps near lines 965 and 1350), `src/fb.rs` (replace pixel-block logo with the wordmark), `src/splash.rs` (layout helpers for the wordmark, stage text), `tests/boot_test.py` (`find_bar`, logo and bar centre checks)

**Interfaces:**
- Consumes: Tasks 1-3. Produces: `splash::wordmark_origin(width: usize, height: usize, wordmark_w: usize, wordmark_h: usize) -> (usize, usize)`; stage name text under the bar that cross-fades between stages; the bar follows `anim::Smoother`.

- [ ] **Step 1:** update the splash tests first to the new geometry (wordmark and bar centred and inside the screen at 640x480, 1024x768, 1920x1080; stage text present; `quiet` shows no splash). **Step 2:** run `python3 tests/boot_test.py -k boot -k modes`, expect FAIL. **Step 3:** implement; total animation time at most 400 ms, none in `quiet` or recovery. **Step 4:** run those tests, then the full suite. **Step 5:** commit `feat(ui): new splash with wordmark and smooth bar`.

### Task 5: Dawn boot menu with arrow keys

**Files:**
- Modify: `src/main.rs` (boot menu loop around lines 740-800), `src/fb.rs` (menu drawing), `tests/boot_test.py` (`t_menu`)

**Interfaces:**
- Consumes: Tasks 1-3. Produces: a menu list (Boot, Recovery, Firmware shell, Debug logging, Firmware setup, Reboot, Power off), Up/Down moves the highlight, Enter runs it, the existing letter keys keep working, timeout still boots.

- [ ] **Step 1:** extend `t_menu`: Down, Down, Enter selects the third entry; the letter keys still work; no key means the timeout boots. **Step 2:** run, expect FAIL. **Step 3:** implement over the firmware's key input (scan codes for Up/Down). **Step 4:** run `-k menu` then the full suite. **Step 5:** commit `feat(boot): arrow-key boot menu`.

### Task 6: Shell look

**Files:**
- Modify: `src/fb.rs` (mono cell rendering, colours, blinking cursor, status bar), `src/shell.rs` (prompt colours)
- Test: `tests/boot_test.py` `t_shell` (prompt visible, text readable, cursor drawn)

- [ ] **Step 1:** add checks for a coloured prompt and a status bar, and that the serial output of every shell command is unchanged. **Step 2:** run, expect FAIL. **Step 3:** implement; the grid size changes with the cell, so scrolling and `clear` must be re-tested at all three screen modes. **Step 4:** run the full suite. **Step 5:** commit `feat(ui): new shell look`.

### Task 7: Release

**Files:** `README.md`, `Cargo.toml` (0.8.0), `usb/`

- [ ] **Step 1:** README: screenshots section, the new menu keys, font licence note. **Step 2:** `bash tools/release.sh`, then `bash tests/run.sh`; expect 0 failed. **Step 3:** commit `chore(release): 0.8.0`. Leave unpushed.
