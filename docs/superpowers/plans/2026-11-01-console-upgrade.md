# Console upgrade plan

> **Status (checked 2026-11-02): complete.** Scrollback, new keys, history and grouped help landed in 499c228, 6247a6a and 80919c6. The "not in scope" cursor editing was built too (Left/Right/Home/End/Delete, Ctrl keys, Tab completion). 2026-11-02: `help all` lines were shortened to fit the 71 columns of a 640 px screen, and the status bar now has a short form for narrow screens.


Goal: the shell can scroll back, uses the whole screen, recalls old commands, and has a readable `help`.

Why: in the QEMU window the log area was about half the screen (the big wordmark sat above it), long output such as `help` scrolled past with no way back, and the command list was one long wall of text.

## Changes

1. `src/fb.rs`: replace the screen-sized cell grid with a 512-line history ring (`HIST`). The screen shows a window of it. `scroll_up(n)`, `scroll_down(n)`, `scroll_bottom()`, `scrolled()`. New output while scrolled keeps the view still. `fullscreen_log()` gives the log the whole screen (the wordmark is only for the splash and menu).
2. `src/keymap.rs`: new keys `PageUp`, `PageDown`, `Up`, `Down`, `Home`, `End` from the extended scan codes (0xE0 prefix). Pure, host-tested.
3. `src/shell.rs`: history of the last 8 commands (Up/Down), PageUp/PageDown scroll, any other key snaps back to the bottom. `help` becomes a grouped list of names; `help <command>` gives usage and a sentence; `help all` lists everything. One command table drives all three. Status bar tells how to scroll and shows `scrolled -N` while scrolled.
4. Tests: keymap unit tests, command-table unit tests (every dispatched command is in the table), boot tests for `help`, `help ls`, `help nosuch`.

## Not in scope

Mouse wheel (needs the USB/PS2 mouse driver), text selection, left/right cursor editing.
