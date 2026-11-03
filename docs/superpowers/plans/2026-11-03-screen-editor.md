# Full-screen text editor for `edit`: implementation plan

**Goal:** `edit <file>` opens the file (or an empty one), shows what is in it, and lets you move between lines with
the arrow keys and hotkeys, change any line, save and leave. Replaces the old "type lines, end with ." mode.

**Architecture:** a pure text buffer (`editbuf.rs`: lines, cursor, scrolling, edits, limits) that is host-tested;
two small console calls (`edit_draw`, `edit_end` in `fb.rs`) that paint a header, the visible lines and a help
line, and hide the log while the editor owns the screen; the shell loop sends keys to the editor while it is open.

## Keys

| Key | Does |
|---|---|
| Up, Down, Ctrl+P, Ctrl+N | previous / next line (the column is kept where it can be) |
| Left, Right | one character; at a line's end they cross to the next or previous line |
| Home, End, Ctrl+A, Ctrl+E | start / end of the line |
| PageUp, PageDown | one screen up / down |
| Ctrl+Home, Ctrl+End | first / last line |
| Enter, Backspace, Delete | split the line / join with the line above / join with the line below |
| Tab | four spaces |
| Ctrl+K | cut the whole line |
| Ctrl+S | save (stays in the editor) |
| Ctrl+X, Ctrl+C | leave; when there are unsaved changes the first press only warns, the second discards |

## Limits (checked, never panic)

Line up to 1024 characters; up to 4096 lines; the whole text must fit the RAM disk (1 MiB); printable ASCII only.
A refused key shows a message in the help line and changes nothing.

## Tasks

1. `src/editbuf.rs`: `Doc` with load/save-to-bytes, all edits, `fit(rows, cols)` scrolling; tests first (edits at
   the ends, join and split, column memory, limits, random key storms keep the invariants).
2. `src/fb.rs`: `edit_draw` (diffs against what is on screen, repaints only changed rows), `edit_end`; `write_line`,
   `status` and the cursor ignore the console while the editor is open.
3. `src/shell.rs`: `edit <file>` opens the editor (folder, bad name, missing parent folder are refused first);
   key handling; save and leave messages; the prompt comes back.
4. Tests: host tests for the buffer; QEMU group `editor`: edit, move up and down, change, save, `cat`; unsaved
   changes are kept out of the file on discard; existing text is loaded; screenshots checked by eye.
5. Replace the old edit steps in the other test groups and the test guide.
6. A sweep test that runs every command in `help all` at least once (and fails when a new command is not in
   the sweep), plus a folder workflow test (files inside folders inside folders).
