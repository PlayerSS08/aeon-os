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
pub const GROUPS: &[&str] = &["system", "hardware", "memory", "tasks", "files", "disk", "programs", "tools", "power"];

pub const COMMANDS: &[Cmd] = &[
    c("system", "help", "help [command|all]", "this list; `help <command>` explains one command"),
    c("system", "info", "info", "firmware, CPU clock, Secure Boot"),
    c("system", "version", "version", "Aeon version and revision"),
    c("system", "cpu", "cpu", "CPUs, interrupt controllers, address sizes"),
    c("system", "security", "security", "which protections are active right now"),
    c("system", "tpm", "tpm", "measured-boot PCR values"),
    c("system", "lastpanic", "lastpanic", "how the previous boot ended"),
    c("system", "uptime", "uptime", "time since boot"),
    c("system", "log", "log", "replay the boot log"),
    c("hardware", "pci", "pci", "devices on the PCI bus"),
    c("hardware", "disks", "disks", "the disk, if there is one"),
    c("hardware", "dread", "dread <n> [off]", "32 bytes of sector n as hex"),
    c("memory", "mem", "mem", "frames, heap and RAM totals"),
    c("memory", "memmap", "memmap", "the firmware memory map"),
    c("tasks", "ps", "ps", "list tasks"),
    c("tasks", "spawn", "spawn", "start 2 busy workers (watch `ps` and `counters`)"),
    c("tasks", "counters", "counters", "how far the busy workers have counted"),
    c("tasks", "block", "block", "start a task that waits on a queue"),
    c("tasks", "wake", "wake", "wake the tasks waiting on that queue"),
    c("tasks", "kill", "kill <id>", "stop a task"),
    c("files", "ls", "ls", "list the files in RAM"),
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
    c("power", "panic", "panic", "trigger a kernel panic, to see the stop screen"),
    c("power", "reboot", "reboot", "restart the machine"),
    c("power", "shutdown", "shutdown", "power off"),
];

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
    fn find_works() {
        assert_eq!(find("ls").map(|c| c.usage), Some("ls"));
        assert!(find("nosuch").is_none());
        assert!(find("").is_none());
    }

    /// Every command the shell dispatches is in the table, and every table entry is dispatched.
    #[test]
    fn table_matches_what_the_shell_runs() {
        let src = include_str!("shell.rs");
        let start = src.find("fn run(line: &str)").expect("run() in shell.rs");
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
