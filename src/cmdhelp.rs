//! The shell's command table: one place that says what each command is called, how it is used and
//! what it does. `help`, `help <command>` and `help all` all read from it. Pure, host-tested.

pub struct Cmd {
    pub group: &'static str,
    pub name: &'static str,
    pub usage: &'static str,
    pub about: &'static str,
}

const fn c(group: &'static str, name: &'static str, usage: &'static str, about: &'static str) -> Cmd {
    Cmd { group, name, usage, about }
}

/// Groups in the order `help` shows them.
// Aliases people bring from other systems: Windows (cls, dir, ver), Linux (man, free, uname, history).
pub const GROUPS: &[&str] = &["system", "status", "hardware", "memory", "tasks", "files", "disk", "programs", "tools", "power"];

pub const COMMANDS: &[Cmd] = &[
    c("system", "help", "help [command|all]", "this list; `help <command>` explains one command"),
    c("system", "info", "info", "firmware, CPU clock, Secure Boot"),
    c("system", "version", "version", "Aeon version and revision"),
    c("system", "cpu", "cpu", "CPUs, interrupt controllers, address sizes"),
    c("status", "security", "security", "which protections are active right now"),
    c("status", "tpm", "tpm", "measured-boot PCR values"),
    c("status", "lastpanic", "lastpanic", "how the previous boot ended"),
    c("system", "uptime", "uptime", "time since boot"),
    c("system", "uname", "uname", "system name, version and architecture"),
    c("system", "ver", "ver", "same as `version`"),
    c("system", "man", "man <command>", "same as `help <command>`"),
    c("system", "history", "history", "the commands you ran lately (Ctrl+Up/Down recall them)"),
    c("status", "log", "log", "replay the boot log"),
    c("hardware", "pci", "pci", "devices on the PCI bus"),
    c("hardware", "disks", "disks", "the disk, if there is one"),
    c("hardware", "dread", "dread <n> [off]", "32 bytes of sector n as hex"),
    c("memory", "mem", "mem", "frames, heap and RAM totals"),
    c("memory", "free", "free", "same as `mem`"),
    c("memory", "memmap", "memmap", "the firmware memory map"),
    c("tasks", "ps", "ps", "list tasks"),
    c("tasks", "spawn", "spawn", "start 2 busy workers (watch `ps` and `counters`)"),
    c("tasks", "counters", "counters", "how far the busy workers have counted"),
    c("tasks", "block", "block", "start a task that waits on a queue"),
    c("tasks", "wake", "wake", "wake the tasks waiting on that queue"),
    c("tasks", "kill", "kill <id>", "stop a task"),
    c("files", "ls", "ls", "list the files in RAM"),
    c("files", "dir", "dir", "same as `ls`"),
    c("files", "df", "df", "space used by the RAM files"),
    c("files", "cat", "cat <file>", "print a file"),
    c("files", "write", "write <file> <text>", "create or replace a file in RAM"),
    c("files", "rm", "rm <file>", "delete a file"),
    c("disk", "dls", "dls [path]", "list a directory on the disk (FAT16/FAT32)"),
    c("disk", "dcat", "dcat <path>", "print a file from the disk (first 64 KiB)"),
    c("programs", "run", "run <program>", "run a built-in in ring 3: hello fault gp spin badcall"),
    c("tools", "echo", "echo <text>", "print text"),
    c("tools", "sort", "sort <n> <n> ...", "sort numbers (uses the heap)"),
    c("tools", "clear", "clear", "clear the screen (or press Ctrl+L)"),
    c("tools", "cls", "cls", "same as `clear`"),
    c("power", "panic", "panic", "trigger a kernel panic, to see the stop screen"),
    c("power", "reboot", "reboot", "restart the machine"),
    c("power", "shutdown", "shutdown", "power off"),
];

/// The commands whose names start with `prefix`.
pub fn matching(prefix: &str) -> impl Iterator<Item = &'static Cmd> + '_ {
    COMMANDS.iter().filter(move |c| c.name.starts_with(prefix))
}

/// The longest text every command starting with `prefix` begins with (`prefix` itself when nothing
/// matches or the matches differ right after it).
pub fn common_prefix(prefix: &str) -> &'static str {
    let mut it = matching(prefix);
    let Some(first) = it.next() else { return "" };
    let mut n = first.name.len();
    for c in it {
        n = n.min(first.name.bytes().zip(c.name.bytes()).take_while(|(a, b)| a == b).count());
    }
    &first.name[..n]
}

/// The closest command name to a mistyped `word` (at most 2 edits away), for "did you mean".
pub fn suggest(word: &str) -> Option<&'static str> {
    const MAX: usize = 16;
    let w = word.as_bytes();
    if w.is_empty() || w.len() > MAX {
        return None;
    }
    let mut best: Option<(usize, &'static str)> = None;
    for c in COMMANDS {
        let d = edit_distance(w, c.name.as_bytes());
        if d <= 2 && best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, c.name));
        }
    }
    best.map(|(_, n)| n)
}

/// Levenshtein distance for short ASCII words (both at most 16 bytes; longer names give a large value).
fn edit_distance(a: &[u8], b: &[u8]) -> usize {
    const MAX: usize = 16;
    if a.len() > MAX || b.len() > MAX {
        return usize::MAX / 2;
    }
    let mut prev = [0usize; MAX + 1];
    let mut cur = [0usize; MAX + 1];
    for (j, p) in prev.iter_mut().enumerate().take(b.len() + 1) {
        *p = j;
    }
    for i in 1..=a.len() {
        cur[0] = i;
        for j in 1..=b.len() {
            let sub = prev[j - 1] + usize::from(a[i - 1] != b[j - 1]);
            cur[j] = sub.min(prev[j] + 1).min(cur[j - 1] + 1);
        }
        prev = cur;
    }
    prev[b.len()]
}

/// The command called `name`, if any.
pub fn find(name: &str) -> Option<&'static Cmd> {
    COMMANDS.iter().find(|c| c.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_unique_and_usage_starts_with_the_name() {
        for (i, a) in COMMANDS.iter().enumerate() {
            assert!(!a.about.is_empty());
            assert!(a.usage == a.name || a.usage.starts_with(&format!("{} ", a.name)), "{}", a.usage);
            assert!(GROUPS.contains(&a.group), "unknown group {}", a.group);
            for b in &COMMANDS[i + 1..] {
                assert_ne!(a.name, b.name);
            }
        }
    }

    #[test]
    fn every_group_has_commands() {
        for g in GROUPS {
            assert!(COMMANDS.iter().any(|c| c.group == *g), "empty group {}", g);
        }
    }

    #[test]
    fn lines_fit_an_80_column_screen() {
        for c in COMMANDS {
            // `help all` prints "  {usage:<20} {about}".
            assert!(c.usage.len() <= 20, "{}", c.usage);
            assert!(2 + 20 + 1 + c.about.len() <= 80, "{} is too long for one line", c.name);
        }
        for g in GROUPS {
            let names: usize = COMMANDS.iter().filter(|c| c.group == *g).map(|c| c.name.len() + 1).sum();
            assert!(10 + names <= 80, "group line {} too long", g);
        }
    }

    #[test]
    fn prefix_matching_and_completion() {
        assert_eq!(matching("d").count(), 6); // dir dread dls dcat df disks
        assert_eq!(common_prefix("d"), "d");
        assert_eq!(common_prefix("dc"), "dcat");
        assert_eq!(common_prefix("he"), "help");
        assert_eq!(common_prefix("sh"), "shutdown");
        assert_eq!(common_prefix("mem"), "mem"); // mem and memmap
        assert_eq!(common_prefix("zz"), "");
        assert_eq!(common_prefix("").len(), 0, "everything matches, nothing in common");
    }

    #[test]
    fn did_you_mean() {
        assert_eq!(suggest("hlep"), Some("help"));
        assert_eq!(suggest("shutdwon"), Some("shutdown"));
        assert_eq!(suggest("lss"), Some("ls"));
        assert_eq!(suggest("xyzzyplugh"), None);
        assert_eq!(suggest(""), None);
        assert_eq!(suggest("a-very-long-mistyped-command"), None);
        assert_eq!(suggest("ls"), Some("ls"), "an exact name is its own best match");
    }

    #[test]
    fn edit_distance_basics() {
        assert_eq!(edit_distance(b"", b""), 0);
        assert_eq!(edit_distance(b"abc", b""), 3);
        assert_eq!(edit_distance(b"kitten", b"sitting"), 3);
        assert_eq!(edit_distance(b"ps", b"pc"), 1);
    }

    #[test]
    fn find_works() {
        assert_eq!(find("ls").map(|c| c.usage), Some("ls"));
        assert!(find("nosuch").is_none());
        assert!(find("").is_none());
    }

    /// Every command the shell dispatches is in the table, and every table entry is dispatched.
    #[test]
    fn table_matches_what_the_shell_runs() {
        let src = include_str!("shell.rs");
        let start = src.find("fn run(line: &str, ").expect("run() in shell.rs");
        let end = src[start..].find("/// The interactive shell task.").expect("end of run()") + start;
        let mut dispatched = Vec::new();
        for line in src[start..end].lines() {
            let t = line.trim_start();
            if !t.starts_with('"') {
                continue;
            }
            let Some((pat, _)) = t.split_once("=>") else { continue };
            for alt in pat.split('|') {
                let name = alt.trim().trim_matches('"');
                if !name.is_empty() && name != "?" && !name.contains(' ') {
                    dispatched.push(name.to_string());
                }
            }
        }
        assert!(dispatched.len() > 25, "scanner found only {}", dispatched.len());
        for name in &dispatched {
            assert!(find(name).is_some(), "`{}` is run by the shell but missing from the help table", name);
        }
        for c in COMMANDS {
            assert!(dispatched.iter().any(|d| d == c.name), "`{}` is in the help table but the shell does not run it", c.name);
        }
    }
}
