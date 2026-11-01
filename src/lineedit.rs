//! The shell's line editor: insert and delete anywhere in the line, move the cursor, kill to the start
//! or end or by word, recall a line. Every operation changes the text and tells the caller what bytes to
//! print so the screen matches: only printable bytes, spaces and backspace (0x08) are ever sent, and a
//! line never wraps (the caller sets the limit from the screen width). Pure, host-tested against a
//! simulated terminal.

pub const MAX_LINE: usize = 120;

pub struct LineEditor {
    buf: [u8; MAX_LINE],
    len: usize,
    pos: usize,
    max: usize,
}

/// Collects the bytes of one operation so they go out in a single write.
struct Seq {
    b: [u8; 4 * MAX_LINE + 8],
    n: usize,
}

impl Seq {
    fn new() -> Self {
        Self { b: [0; 4 * MAX_LINE + 8], n: 0 }
    }
    fn push(&mut self, byte: u8, times: usize) {
        for _ in 0..times {
            self.b[self.n] = byte;
            self.n += 1;
        }
    }
    fn extend(&mut self, s: &[u8]) {
        self.b[self.n..self.n + s.len()].copy_from_slice(s);
        self.n += s.len();
    }
    fn bytes(&self) -> &[u8] {
        &self.b[..self.n]
    }
}

impl Default for LineEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl LineEditor {
    pub const fn new() -> Self {
        Self { buf: [0; MAX_LINE], len: 0, pos: 0, max: MAX_LINE }
    }

    /// Longest line allowed (at most [`MAX_LINE`]); keeps a line on one screen row.
    pub fn set_max(&mut self, n: usize) {
        self.max = n.clamp(1, MAX_LINE);
    }

    pub fn text(&self) -> &[u8] {
        &self.buf[..self.len]
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Forgets the line without printing anything (the caller moves to a new row).
    pub fn reset(&mut self) {
        self.len = 0;
        self.pos = 0;
    }

    /// Reprints from index `from` (which must not be after the old cursor) so the row shows the
    /// current text, blanks what was left over and puts the cursor at `pos`.
    fn repaint(&self, out: &mut impl FnMut(&[u8]), from: usize, old_pos: usize, old_len: usize) {
        let mut s = Seq::new();
        s.push(0x08, old_pos.saturating_sub(from));
        s.extend(&self.buf[from..self.len]);
        s.push(b' ', old_len.saturating_sub(self.len));
        s.push(0x08, self.len.max(old_len).saturating_sub(self.pos));
        if s.n > 0 {
            out(s.bytes());
        }
    }

    /// Inserts one printable byte at the cursor. False when the line is full or the byte is not printable.
    pub fn insert(&mut self, c: u8, out: &mut impl FnMut(&[u8])) -> bool {
        if self.len >= self.max || !(0x20..0x7F).contains(&c) {
            return false;
        }
        let (old_pos, old_len) = (self.pos, self.len);
        self.buf.copy_within(self.pos..self.len, self.pos + 1);
        self.buf[self.pos] = c;
        self.len += 1;
        self.pos += 1;
        self.repaint(out, old_pos, old_pos, old_len);
        true
    }

    /// Deletes the character before the cursor.
    pub fn backspace(&mut self, out: &mut impl FnMut(&[u8])) {
        if self.pos == 0 {
            return;
        }
        let (old_pos, old_len) = (self.pos, self.len);
        self.buf.copy_within(self.pos..self.len, self.pos - 1);
        self.len -= 1;
        self.pos -= 1;
        self.repaint(out, self.pos, old_pos, old_len);
    }

    /// Deletes the character under the cursor.
    pub fn delete(&mut self, out: &mut impl FnMut(&[u8])) {
        if self.pos >= self.len {
            return;
        }
        let (old_pos, old_len) = (self.pos, self.len);
        self.buf.copy_within(self.pos + 1..self.len, self.pos);
        self.len -= 1;
        self.repaint(out, self.pos, old_pos, old_len);
    }

    fn move_to(&mut self, to: usize, out: &mut impl FnMut(&[u8])) {
        let to = to.min(self.len);
        if to < self.pos {
            let mut s = Seq::new();
            s.push(0x08, self.pos - to);
            out(s.bytes());
        } else if to > self.pos {
            // Retyping the characters we pass moves the cursor right.
            out(&self.buf[self.pos..to]);
        }
        self.pos = to;
    }

    pub fn left(&mut self, out: &mut impl FnMut(&[u8])) {
        self.move_to(self.pos.saturating_sub(1), out);
    }

    pub fn right(&mut self, out: &mut impl FnMut(&[u8])) {
        self.move_to(self.pos + 1, out);
    }

    pub fn home(&mut self, out: &mut impl FnMut(&[u8])) {
        self.move_to(0, out);
    }

    pub fn end(&mut self, out: &mut impl FnMut(&[u8])) {
        self.move_to(self.len, out);
    }

    /// Deletes from the cursor to the end of the line (Ctrl+K).
    pub fn kill_to_end(&mut self, out: &mut impl FnMut(&[u8])) {
        let (old_pos, old_len) = (self.pos, self.len);
        self.len = self.pos;
        self.repaint(out, self.pos, old_pos, old_len);
    }

    /// Deletes from the start of the line to the cursor (Ctrl+U).
    pub fn kill_to_start(&mut self, out: &mut impl FnMut(&[u8])) {
        let (old_pos, old_len) = (self.pos, self.len);
        self.buf.copy_within(self.pos..self.len, 0);
        self.len -= self.pos;
        self.pos = 0;
        self.repaint(out, 0, old_pos, old_len);
    }

    /// Deletes the word before the cursor (Ctrl+W): trailing spaces, then the word.
    pub fn kill_word(&mut self, out: &mut impl FnMut(&[u8])) {
        let (old_pos, old_len) = (self.pos, self.len);
        let mut start = self.pos;
        while start > 0 && self.buf[start - 1] == b' ' {
            start -= 1;
        }
        while start > 0 && self.buf[start - 1] != b' ' {
            start -= 1;
        }
        if start == self.pos {
            return;
        }
        self.buf.copy_within(self.pos..self.len, start);
        self.len -= self.pos - start;
        self.pos = start;
        self.repaint(out, start, old_pos, old_len);
    }

    /// Replaces the whole line with `text` (history recall); the cursor goes to the end.
    pub fn set(&mut self, text: &[u8], out: &mut impl FnMut(&[u8])) {
        let (old_pos, old_len) = (self.pos, self.len);
        let n = text.len().min(self.max);
        self.buf[..n].copy_from_slice(&text[..n]);
        self.len = n;
        self.pos = n;
        self.repaint(out, 0, old_pos, old_len);
    }

    /// Replaces everything before the cursor with `text` (Tab completion of the first word).
    /// False when the result would not fit.
    pub fn replace_before_cursor(&mut self, text: &[u8], out: &mut impl FnMut(&[u8])) -> bool {
        let rest = self.len - self.pos;
        if text.len() + rest > self.max {
            return false;
        }
        let (old_pos, old_len) = (self.pos, self.len);
        self.buf.copy_within(self.pos..self.len, text.len());
        self.buf[..text.len()].copy_from_slice(text);
        self.len = text.len() + rest;
        self.pos = text.len();
        self.repaint(out, 0, old_pos, old_len);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A one-row terminal: printable bytes overwrite and advance, 0x08 steps left.
    struct Term {
        row: Vec<u8>,
        col: usize,
    }

    impl Term {
        fn new() -> Self {
            Self { row: vec![b' '; 300], col: 0 }
        }
        fn feed(&mut self, bytes: &[u8]) {
            for &b in bytes {
                if b == 0x08 {
                    self.col = self.col.saturating_sub(1);
                } else {
                    assert!((0x20..0x7F).contains(&b), "only printable bytes and backspace: {b:#x}");
                    self.row[self.col] = b;
                    self.col += 1;
                }
            }
        }
    }

    /// Runs `f` on an editor wired to a terminal, then checks the screen and cursor match the editor.
    fn check(ed: &mut LineEditor, term: &mut Term, f: impl FnOnce(&mut LineEditor, &mut dyn FnMut(&[u8]))) {
        let mut out = |b: &[u8]| term.feed(b);
        f(ed, &mut out);
        let text = ed.text().to_vec();
        assert_eq!(&term.row[..text.len()], &text[..], "screen text");
        assert!(term.row[text.len()..].iter().all(|&b| b == b' '), "leftover characters: {:?}", String::from_utf8_lossy(&term.row[..40]));
        assert_eq!(term.col, ed.pos(), "cursor column");
    }

    fn typed(s: &str) -> (LineEditor, Term) {
        let mut ed = LineEditor::new();
        let mut t = Term::new();
        for c in s.bytes() {
            check(&mut ed, &mut t, |e, o| {
                assert!(e.insert(c, &mut |b| o(b)));
            });
        }
        (ed, t)
    }

    #[test]
    fn typing_and_backspace() {
        let (mut ed, mut t) = typed("hello");
        assert_eq!(ed.text(), b"hello");
        check(&mut ed, &mut t, |e, o| e.backspace(&mut |b| o(b)));
        assert_eq!(ed.text(), b"hell");
    }

    #[test]
    fn insert_in_the_middle_shifts_the_rest() {
        let (mut ed, mut t) = typed("hllo");
        check(&mut ed, &mut t, |e, o| {
            e.left(&mut |b| o(b));
            e.left(&mut |b| o(b));
            e.left(&mut |b| o(b));
        });
        check(&mut ed, &mut t, |e, o| {
            e.insert(b'e', &mut |b| o(b));
        });
        assert_eq!(ed.text(), b"hello");
        assert_eq!(ed.pos(), 2);
    }

    #[test]
    fn delete_and_backspace_in_the_middle() {
        let (mut ed, mut t) = typed("abcdef");
        check(&mut ed, &mut t, |e, o| {
            for _ in 0..3 {
                e.left(&mut |b| o(b));
            }
        });
        check(&mut ed, &mut t, |e, o| e.delete(&mut |b| o(b)));
        assert_eq!(ed.text(), b"abcef");
        check(&mut ed, &mut t, |e, o| e.backspace(&mut |b| o(b)));
        assert_eq!(ed.text(), b"abef");
        assert_eq!(ed.pos(), 2);
    }

    #[test]
    fn home_end_and_edges_do_nothing_past_the_ends() {
        let (mut ed, mut t) = typed("abc");
        check(&mut ed, &mut t, |e, o| e.right(&mut |b| o(b))); // already at the end
        check(&mut ed, &mut t, |e, o| e.home(&mut |b| o(b)));
        check(&mut ed, &mut t, |e, o| e.left(&mut |b| o(b))); // already at the start
        check(&mut ed, &mut t, |e, o| e.backspace(&mut |b| o(b)));
        check(&mut ed, &mut t, |e, o| e.right(&mut |b| o(b)));
        assert_eq!(ed.pos(), 1);
        check(&mut ed, &mut t, |e, o| e.end(&mut |b| o(b)));
        assert_eq!(ed.pos(), 3);
        check(&mut ed, &mut t, |e, o| e.delete(&mut |b| o(b))); // at the end: nothing
        assert_eq!(ed.text(), b"abc");
    }

    #[test]
    fn kills() {
        let (mut ed, mut t) = typed("echo one two");
        check(&mut ed, &mut t, |e, o| e.kill_word(&mut |b| o(b)));
        assert_eq!(ed.text(), b"echo one ");
        check(&mut ed, &mut t, |e, o| e.kill_word(&mut |b| o(b)));
        assert_eq!(ed.text(), b"echo ");
        check(&mut ed, &mut t, |e, o| e.home(&mut |b| o(b)));
        check(&mut ed, &mut t, |e, o| {
            e.right(&mut |b| o(b));
            e.right(&mut |b| o(b));
        });
        check(&mut ed, &mut t, |e, o| e.kill_to_end(&mut |b| o(b)));
        assert_eq!(ed.text(), b"ec");
        check(&mut ed, &mut t, |e, o| {
            e.left(&mut |b| o(b));
        });
        check(&mut ed, &mut t, |e, o| e.kill_to_start(&mut |b| o(b)));
        assert_eq!(ed.text(), b"c");
        assert_eq!(ed.pos(), 0);
    }

    #[test]
    fn set_replaces_the_line_longer_or_shorter() {
        let (mut ed, mut t) = typed("a long line of text");
        check(&mut ed, &mut t, |e, o| e.set(b"ls", &mut |b| o(b)));
        assert_eq!(ed.text(), b"ls");
        check(&mut ed, &mut t, |e, o| e.set(b"echo a longer one", &mut |b| o(b)));
        check(&mut ed, &mut t, |e, o| e.set(b"", &mut |b| o(b)));
        assert!(ed.is_empty());
    }

    #[test]
    fn replace_before_cursor_keeps_the_rest() {
        let (mut ed, mut t) = typed("he x");
        check(&mut ed, &mut t, |e, o| {
            e.home(&mut |b| o(b));
            e.right(&mut |b| o(b));
            e.right(&mut |b| o(b));
        });
        check(&mut ed, &mut t, |e, o| {
            assert!(e.replace_before_cursor(b"help", &mut |b| o(b)));
        });
        assert_eq!(ed.text(), b"help x");
        assert_eq!(ed.pos(), 4);
    }

    #[test]
    fn limit_and_unprintable_bytes_are_refused() {
        let mut ed = LineEditor::new();
        ed.set_max(3);
        let mut sink = |_: &[u8]| {};
        assert!(ed.insert(b'a', &mut sink) && ed.insert(b'b', &mut sink) && ed.insert(b'c', &mut sink));
        assert!(!ed.insert(b'd', &mut sink));
        ed.reset();
        assert!(!ed.insert(0x1b, &mut sink));
        assert!(!ed.insert(0x80, &mut sink));
        assert!(!ed.insert(b'\n', &mut sink));
        assert!(ed.is_empty());
        let mut t = Term::new();
        ed.set(b"abcdefgh", &mut |b| t.feed(b));
        assert_eq!(ed.text(), b"abc", "set is clipped to the limit");
        let mut ed2 = LineEditor::new();
        ed2.set_max(5);
        ed2.set(b"ab", &mut sink);
        assert!(!ed2.replace_before_cursor(b"abcdef", &mut sink));
        assert_eq!(ed2.text(), b"ab");
    }

    /// Random operations never leave the screen different from the text.
    #[test]
    fn random_operations_keep_screen_and_text_in_step() {
        let mut seed = 0x1234_5678_9abc_def0u64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let mut ed = LineEditor::new();
        ed.set_max(40);
        let mut t = Term::new();
        for _ in 0..3000 {
            let r = next();
            check(&mut ed, &mut t, |e, o| {
                let mut w = |b: &[u8]| o(b);
                match r % 11 {
                    0..=3 => {
                        e.insert(b'a' + (r >> 8) as u8 % 26, &mut w);
                    }
                    4 => e.backspace(&mut w),
                    5 => e.delete(&mut w),
                    6 => e.left(&mut w),
                    7 => e.right(&mut w),
                    8 => match (r >> 8) % 5 {
                        0 => e.kill_word(&mut w),
                        1 => e.kill_to_end(&mut w),
                        2 => e.kill_to_start(&mut w),
                        3 => e.home(&mut w),
                        _ => e.end(&mut w),
                    },
                    9 => {
                        e.insert(b' ', &mut w);
                    }
                    _ => {
                        let n = (r >> 8) as usize % 12;
                        let text: Vec<u8> = (0..n).map(|i| b'a' + i as u8).collect();
                        if (r >> 20) & 1 == 0 {
                            e.set(&text, &mut w);
                        } else {
                            e.replace_before_cursor(&text, &mut w);
                        }
                    }
                }
            });
        }
    }
}
