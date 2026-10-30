//! Boot options from the image's command line (UEFI load options), in the spirit of the Linux kernel
//! command line and Windows BCD switches. Pure, host-testable.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BootOpts {
    /// Only warnings and errors.
    pub quiet: bool,
    /// Show trace-level messages.
    pub debug: bool,
    /// Seconds to wait for a key before booting (0 skips the boot menu).
    pub menu_timeout: u8,
    /// Show the log on screen during boot instead of the logo and progress bar.
    pub verbose: bool,
}

pub const DEFAULT_TIMEOUT: u8 = 2;
pub const MAX_TIMEOUT: u8 = 30;

impl Default for BootOpts {
    fn default() -> Self {
        Self { quiet: false, debug: false, menu_timeout: DEFAULT_TIMEOUT, verbose: false }
    }
}

/// Parses space-separated words: `quiet`, `debug`, `nomenu`, `timeout=N`. Unknown words are ignored.
pub fn parse(cmdline: &str) -> BootOpts {
    let mut o = BootOpts::default();
    for w in cmdline.split_whitespace() {
        match w {
            "quiet" => o.quiet = true,
            "debug" => o.debug = true,
            "nomenu" => o.menu_timeout = 0,
            "verbose" => o.verbose = true,
            _ => {
                if let Some(n) = w.strip_prefix("timeout=").and_then(|n| n.parse::<u8>().ok()) {
                    o.menu_timeout = n.min(MAX_TIMEOUT);
                }
            }
        }
    }
    if o.quiet && o.debug {
        o.quiet = false; // contradictory: the more informative option wins
    }
    if o.debug {
        o.verbose = true; // debug output is for reading, so the splash would hide it
    }
    o
}

/// Hostile or garbage command lines must never panic the parser.
#[cfg(test)]
mod fuzz {
    use super::*;

    fn xorshift(s: &mut u64) -> u64 {
        *s ^= *s << 13;
        *s ^= *s >> 7;
        *s ^= *s << 17;
        *s
    }

    #[test]
    fn random_command_lines_do_not_panic() {
        let words = ["quiet", "debug", "nomenu", "timeout=", "timeout=5", "verbose", "=", "\u{0}", "\u{ff}", "é"];
        let mut s = 0x9E37_79B9_7F4A_7C15u64;
        for _ in 0..20_000 {
            let mut line = String::new();
            for _ in 0..(xorshift(&mut s) % 8) {
                line.push_str(words[(xorshift(&mut s) % words.len() as u64) as usize]);
                if xorshift(&mut s) % 3 != 0 {
                    line.push(' ');
                }
                if xorshift(&mut s) % 5 == 0 {
                    line.push_str(&(xorshift(&mut s) % 100_000).to_string());
                }
            }
            let o = parse(&line);
            assert!(o.menu_timeout <= MAX_TIMEOUT);
            assert!(!(o.quiet && o.debug));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults() {
        assert_eq!(parse(""), BootOpts::default());
    }

    #[test]
    fn words_and_timeout() {
        let o = parse("quiet timeout=7 junk");
        assert!(o.quiet && !o.debug);
        assert_eq!(o.menu_timeout, 7);
        assert_eq!(parse("nomenu").menu_timeout, 0);
    }

    #[test]
    fn timeout_is_clamped_and_garbage_ignored() {
        assert_eq!(parse("timeout=200").menu_timeout, 30);
        assert_eq!(parse("timeout=abc").menu_timeout, DEFAULT_TIMEOUT);
        assert_eq!(parse("timeout=999999").menu_timeout, DEFAULT_TIMEOUT);
    }

    #[test]
    fn verbose_and_debug_show_the_log() {
        assert!(!parse("").verbose);
        assert!(parse("verbose").verbose);
        assert!(parse("debug").verbose);
    }

    #[test]
    fn quiet_and_debug_conflict() {
        let o = parse("quiet debug");
        assert!(o.debug && !o.quiet);
    }
}
