//! Flat RAM filesystem: named byte files with a file-count and byte budget. Pure, host-testable.

use alloc::string::String;
use alloc::vec::Vec;

pub const MAX_NAME: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FsError {
    BadName,
    Full,
    TooBig,
}

pub struct RamFs {
    files: Vec<(String, Vec<u8>)>,
    max_files: usize,
    max_bytes: usize,
    used: usize,
}

fn name_ok(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME
        && name != "."
        && name != ".."
        && !name.bytes().any(|b| b == b'/' || b == b'\\' || b < 0x20 || b == 0x7F)
}

impl RamFs {
    pub const fn new(max_files: usize, max_bytes: usize) -> RamFs {
        RamFs { files: Vec::new(), max_files, max_bytes, used: 0 }
    }

    fn find(&self, name: &str) -> Result<usize, usize> {
        self.files.binary_search_by(|(n, _)| n.as_str().cmp(name))
    }

    /// Creates or replaces a file. On error nothing changes, an existing file keeps its old content.
    pub fn write(&mut self, name: &str, data: &[u8]) -> Result<(), FsError> {
        if !name_ok(name) {
            return Err(FsError::BadName);
        }
        match self.find(name) {
            Ok(i) => {
                let old = self.files[i].1.len();
                if self.used - old + data.len() > self.max_bytes {
                    return Err(FsError::TooBig);
                }
                self.used = self.used - old + data.len();
                self.files[i].1 = data.to_vec();
            }
            Err(at) => {
                if self.files.len() >= self.max_files {
                    return Err(FsError::Full);
                }
                if self.used + data.len() > self.max_bytes {
                    return Err(FsError::TooBig);
                }
                self.used += data.len();
                self.files.insert(at, (String::from(name), data.to_vec()));
            }
        }
        Ok(())
    }

    pub fn read(&self, name: &str) -> Option<&[u8]> {
        self.find(name).ok().map(|i| self.files[i].1.as_slice())
    }

    pub fn remove(&mut self, name: &str) -> bool {
        match self.find(name) {
            Ok(i) => {
                self.used -= self.files.remove(i).1.len();
                true
            }
            Err(_) => false,
        }
    }

    /// File names with their sizes, sorted by name.
    pub fn names(&self) -> impl Iterator<Item = (&str, usize)> {
        self.files.iter().map(|(n, d)| (n.as_str(), d.len()))
    }

    pub fn used_bytes(&self) -> usize {
        self.used
    }

    pub fn count(&self) -> usize {
        self.files.len()
    }

    pub fn max_bytes(&self) -> usize {
        self.max_bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_bad_names() {
        let mut fs = RamFs::new(4, 100);
        let long = "x".repeat(MAX_NAME + 1);
        for n in ["", long.as_str(), "a/b", "a\\b", "a\0b", ".", "..", "a\nb"] {
            assert_eq!(fs.write(n, b"x"), Err(FsError::BadName), "{n:?}");
        }
        assert_eq!(fs.count(), 0);
        assert!(fs.write(&"y".repeat(MAX_NAME), b"x").is_ok());
    }

    #[test]
    fn write_read_roundtrip() {
        let mut fs = RamFs::new(4, 100);
        fs.write("a", b"hello").unwrap();
        assert_eq!(fs.read("a"), Some(&b"hello"[..]));
        assert_eq!(fs.read("b"), None);
        assert_eq!(fs.used_bytes(), 5);
    }

    #[test]
    fn overwrite_replaces_and_frees_old_bytes() {
        let mut fs = RamFs::new(4, 10);
        fs.write("a", b"0123456789").unwrap();
        fs.write("a", b"xy").unwrap();
        assert_eq!(fs.used_bytes(), 2);
        assert_eq!(fs.read("a"), Some(&b"xy"[..]));
    }

    #[test]
    fn full_by_count_keeps_existing() {
        let mut fs = RamFs::new(2, 100);
        fs.write("a", b"1").unwrap();
        fs.write("b", b"2").unwrap();
        assert_eq!(fs.write("c", b"3"), Err(FsError::Full));
        assert_eq!(fs.count(), 2);
        assert!(fs.write("a", b"9").is_ok(), "overwrite still allowed when the count is full");
    }

    #[test]
    fn full_by_bytes_keeps_existing() {
        let mut fs = RamFs::new(4, 10);
        fs.write("a", b"12345").unwrap();
        assert_eq!(fs.write("a", &[0u8; 11]), Err(FsError::TooBig));
        assert_eq!(fs.read("a"), Some(&b"12345"[..]));
        assert_eq!(fs.write("b", &[0u8; 6]), Err(FsError::TooBig));
        assert_eq!(fs.used_bytes(), 5);
    }

    #[test]
    fn remove_frees_space() {
        let mut fs = RamFs::new(4, 10);
        fs.write("a", b"12345").unwrap();
        assert!(fs.remove("a"));
        assert!(!fs.remove("a"));
        assert_eq!(fs.used_bytes(), 0);
        assert_eq!(fs.count(), 0);
    }

    #[test]
    fn names_sorted() {
        let mut fs = RamFs::new(8, 100);
        for n in ["m", "b", "z", "a"] {
            fs.write(n, b"-").unwrap();
        }
        let v: Vec<&str> = fs.names().map(|(n, _)| n).collect();
        assert_eq!(v, ["a", "b", "m", "z"]);
    }
}
