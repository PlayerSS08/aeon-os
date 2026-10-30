//! PS/2 scancode set 1 to characters, with shift/caps/ctrl state. Pure, host-testable.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Char(u8),
    Enter,
    Backspace,
    Tab,
    Esc,
    /// Ctrl+C
    Interrupt,
    /// Ctrl+L
    ClearScreen,
}

#[derive(Default)]
pub struct Keyboard {
    shift: bool,
    ctrl: bool,
    caps: bool,
    extended: bool,
}

const LOWER: &[u8; 58] = b"\0\x1b1234567890-=\x08\tqwertyuiop[]\n\0asdfghjkl;'`\0\\zxcvbnm,./\0*\0 ";
const UPPER: &[u8; 58] = b"\0\x1b!@#$%^&*()_+\x08\tQWERTYUIOP{}\n\0ASDFGHJKL:\"~\0|ZXCVBNM<>?\0*\0 ";

impl Keyboard {
    pub const fn new() -> Self {
        Self { shift: false, ctrl: false, caps: false, extended: false }
    }

    /// Feeds one raw byte from port 0x60; returns a key on a key press.
    pub fn feed(&mut self, code: u8) -> Option<Key> {
        if code == 0xE0 {
            self.extended = true;
            return None;
        }
        let ext = core::mem::take(&mut self.extended);
        let release = code & 0x80 != 0;
        let make = code & 0x7F;
        match make {
            0x2A | 0x36 if !ext => {
                self.shift = !release;
                return None;
            }
            0x1D => {
                self.ctrl = !release;
                return None;
            }
            0x3A if !release => {
                self.caps = !self.caps;
                return None;
            }
            _ => {}
        }
        if release || ext {
            return None; // arrows and other extended keys are not mapped yet
        }
        let table = if self.shift { UPPER } else { LOWER };
        let mut c = *table.get(make as usize)?;
        if self.caps && c.is_ascii_alphabetic() {
            c = if self.shift { c.to_ascii_lowercase() } else { c.to_ascii_uppercase() };
        }
        if self.ctrl {
            match c.to_ascii_lowercase() {
                b'c' => return Some(Key::Interrupt),
                b'l' => return Some(Key::ClearScreen),
                _ => {}
            }
        }
        match c {
            0 => None,
            b'\n' => Some(Key::Enter),
            0x08 => Some(Key::Backspace),
            b'\t' => Some(Key::Tab),
            0x1b => Some(Key::Esc),
            c => Some(Key::Char(c)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_and_shifted_letters() {
        let mut k = Keyboard::new();
        assert_eq!(k.feed(0x1E), Some(Key::Char(b'a')));
        assert_eq!(k.feed(0x9E), None); // release
        k.feed(0x2A); // shift down
        assert_eq!(k.feed(0x1E), Some(Key::Char(b'A')));
        assert_eq!(k.feed(0x02), Some(Key::Char(b'!')));
        k.feed(0xAA); // shift up
        assert_eq!(k.feed(0x02), Some(Key::Char(b'1')));
    }

    #[test]
    fn caps_lock_inverts_with_shift() {
        let mut k = Keyboard::new();
        k.feed(0x3A);
        assert_eq!(k.feed(0x10), Some(Key::Char(b'Q')));
        k.feed(0x2A);
        assert_eq!(k.feed(0x10), Some(Key::Char(b'q')));
    }

    #[test]
    fn control_keys() {
        let mut k = Keyboard::new();
        assert_eq!(k.feed(0x1C), Some(Key::Enter));
        assert_eq!(k.feed(0x0E), Some(Key::Backspace));
        k.feed(0x1D);
        assert_eq!(k.feed(0x2E), Some(Key::Interrupt));
        assert_eq!(k.feed(0x26), Some(Key::ClearScreen));
    }

    #[test]
    fn extended_and_unknown_codes_are_ignored() {
        let mut k = Keyboard::new();
        assert_eq!(k.feed(0xE0), None);
        assert_eq!(k.feed(0x48), None); // up arrow
        assert_eq!(k.feed(0x5F), None); // beyond the table
        assert_eq!(k.feed(0x1E), Some(Key::Char(b'a')));
    }
}
