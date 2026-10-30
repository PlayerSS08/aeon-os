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
}

pub const DEFAULT_TIMEOUT: u8 = 2;
pub const MAX_TIMEOUT: u8 = 30;

impl Default for BootOpts {
    fn default() -> Self {
        Self { quiet: false, debug: false, menu_timeout: DEFAULT_TIMEOUT }
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
    o
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
    fn quiet_and_debug_conflict() {
        let o = parse("quiet debug");
        assert!(o.debug && !o.quiet);
    }
}
