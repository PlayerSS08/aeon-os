# Screen size, multi-row typing and Dawn first: implementation plan

> **For agentic workers:** execute task by task; steps use checkboxes.

**Goal:** (1) Aeon fills the whole screen by itself, (2) a typed line can use more than one screen row, (3) the boot menu Dawn opens first with "Boot Aeon" highlighted instead of waiting for a key press, (4) a checklist of the CA-6 assignment for the owner.

**Architecture:** Mode choice becomes a pure function (`src/gopmode.rs`) so it is unit-tested. The console learns to wrap a line and to step back across a wrapped row (`0x08` at column 0). The line editor already tells the console what to print; it only has to stop limiting itself to one row. Dawn is drawn at once and counts down.

**Spec:** the owner's messages of 2026-11-02 (typing stops at one row; "auto resize to the screen size"; menu first, choose Normal, then boot, as Linux does). Answers given: "Largest mode", and "menu first, then the choice boots".

## Global Constraints

- `no_std`, no new crates; `unsafe` only with a `SAFETY:` comment.
- Both clippy gates stay clean (`-D warnings`); host tests run with `cargo test --lib --target x86_64-pc-windows-gnu`.
- Image must stay under 262,144 bytes signed (now 260,648: **only 1.5 KB left**). Every task reports the size it adds.
- Do not push. Commit with the Scrooge-777 identity.
- Boot suite (`bash tests/run.sh`) must pass; tests that waited for "press any key within" are updated on purpose.

## Choices made without the owner (change them if you disagree)

| Choice | Why |
|---|---|
| "Largest mode" has no size cap (the owner chose it), but modes taller than 4320 or wider than 7680 are ignored, and `mode=WxH` still wins | Existing parser limits |
| Dawn counts down from `timeout=N` (default stays 2 s) and boots "Boot Aeon" when it reaches 0; any other key stops the countdown | GRUB and systemd-boot do the same; the owner asked for "menu first" |
| `nomenu` / `timeout=0` still skips Dawn (the fast path) | Needed by tests and by users who want the fastest start |
| Kernel start itself stays about 1.2 s in QEMU; 0.1 s is only reachable for the menu wait, not the whole boot | Measured: 3.27 s to the shell, of which about 2 s was the wait |

## Review Focus

- A typed line that is exactly one row long (cursor at the pending-wrap position), then Backspace.
- A line that wraps at the bottom of the screen (the view scrolls) and is then edited with Left/Home.
- A firmware that offers only blt-only modes (`pixel_format == 3`) or no modes at all.
- A 4K mode: drawing speed (measure in QEMU at 1920x1080 and 3840x2160).
- The countdown when a key was already held at power-on (stale keystrokes).

### Task 1: Pure mode choice

**Files:** Create `src/gopmode.rs`; modify `src/lib.rs`, `src/main.rs` (`pick_gop_mode`).

**Interfaces:** Produces `pub struct ModeInfo { pub index: u32, pub width: u32, pub height: u32, pub linear: bool }` and `pub fn choose(modes: &[ModeInfo], current: u32, want: Option<(u32, u32)>) -> Option<u32>`.

- [ ] Tests first: wanted size wins; otherwise the largest linear mode wins; a tie keeps `current`; blt-only modes are never chosen; empty list gives `None`; a current mode below 640x480 is still replaced.
- [ ] `choose` implemented; `pick_gop_mode` collects the firmware's modes into a fixed array (at most 64) and calls it.
- [ ] Gate: unit tests, both clippy gates.

### Task 2: Typed lines can use several rows

**Files:** Modify `src/lineedit.rs` (host terminal simulator learns to wrap), `src/fb.rs` (`put_byte`, `newline`), `src/shell.rs:1055` (`set_max`), `src/lineedit.rs` (`MAX_LINE`).

**Interfaces:** Consumes the editor's byte stream (printable bytes, space, `0x08`). Produces: `0x08` at column 0 of a row that was reached by auto-wrap returns to the last column of the row above.

- [ ] Tests first in `lineedit.rs`: the simulated terminal gets a width of 10 and wraps; a 25-character line, Backspace 12 times, Home, End, kill-to-start, and history recall all leave the simulated screen equal to the expected text.
- [ ] `Console` gets a per-row "continued" flag; `put_byte` sets it on auto-wrap; `0x08` at column 0 with the flag moves up one row and repaints when the view changes.
- [ ] `set_max(LINE_MAX)` in the shell; `MAX_LINE` raised only if the size budget allows (report bytes).
- [ ] Boot test: type a 90-character line at 640x480 (71 columns), see it on two rows, Enter runs it, the file keeps the whole line.

### Task 3: Dawn first

**Files:** Modify `src/main.rs` (`boot_menu`, `draw_menu`), `src/fb.rs` (footer repaint), `tests/boot_test.py`.

- [ ] `boot_menu` draws Dawn at once when `menu_timeout > 0`, footer "Boot Aeon in N s, any key stops the countdown"; empty the keyboard queue first; when the countdown ends it selects "Boot Aeon".
- [ ] Log line `menu open: boot starts in N s` replaces "press any key within N s"; boot tests updated.
- [ ] Gate: boot groups `boot menu cfg modes bootcount fwshell`.

### Task 4: Checklist for the owner

**Files:** Create `docs/ca6/CHECKLIST.md` (every assignment item, layer, milestone, test case and submission item, ticked only where evidence exists, with the evidence path).

- [ ] Built from `docs/ca6/README.md` and `results.csv`; unticked items are the student's own (contribution statement, AI disclosure signature, viva Q17 and Q18).

### Task 5: Release

- [ ] Full suite, evidence run, `bash tools/release.sh` (signs in WSL; the key is never read), repro check, version 0.9.2, docs and handoff updated, commit (no push).
