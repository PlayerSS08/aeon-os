# Shell prompt design, overwrite safety, and minimal substitutes for missing shell features

*Research date 2026-11-03. Every claim below has a link. Claims marked "(snippet)" come from search-result text I did not open. Anything without a source is in Gaps.*

## 1. Prompt design: how shells show the current directory

### Takeaway
Bash, zsh and fish each show the working directory differently. Bash shows it in full by default, with `~` for home. Fish abbreviates every parent directory to one letter but keeps the last one whole. Windows shells (PowerShell and cmd) show the full path, with no `~` abbreviation. For a tiny OS, the fish-style shortening is the easiest to implement by hand, and the Bash `~`-for-home rule only makes sense once the OS has a defined home directory.

### Cited Findings
- Bash's own default PS1 is `\s-\v\$ ` (shell name, version, `$`), not `user@host:cwd$`. The `user@host:cwd$` look comes from distro-supplied bashrc files. [Bash man page, man7.org](https://man7.org/linux/man-pages/man1/bash.1.html)
- Bash `PROMPT_DIRTRIM`: "If set to a number greater than zero, the value is used as the number of trailing directory components to retain... Characters removed are replaced with an ellipsis." [Bash man page, man7.org](https://man7.org/linux/man-pages/man1/bash.1.html)
- Fish `prompt_pwd`: "will replace the home directory with "~" and shorten every path component but the last to a default of one character." Example: `/tmp/banana/sausage/with/mustard` becomes `/t/b/s/w/mustard`. [Fish docs](https://fishshell.com/docs/current/cmds/prompt_pwd.html)
- Zsh `%~`: "As %d and %/, but if the current working directory starts with $HOME, that part is replaced by a ~"; a named-directory prefix becomes `~name`, "but only if the result is shorter than the full path." `%d` / `%/` show trailing components, with a numeric argument. `%c` is deprecated and equals `%1~`. [Zsh Prompt Expansion](https://zsh.sourceforge.io/Doc/Release/Prompt-Expansion.html)
- PowerShell's built-in prompt is `PS` followed by `Get-Location` (the full path) and `>`. No `~` abbreviation. [about_Prompts, Microsoft Learn](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_prompts)
- Windows cmd: `$P` = "Current drive and path"; `$N` = current drive; `$G` = `>`. The default prompt is the full path followed by `>`. [prompt, Microsoft Learn](https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/prompt)
- Plan 9 rc: `$prompt` defaults to `'% '` (continuation `' '`), and the default prompt does not show the working directory. [rc(1), 9p.io](https://9p.io/magic/man2html/1/rc)
- Bash-prompt snippets (not opened): `\w` shows the working directory with `$HOME` as `~`, and a fish-style one-letter abbreviation can be built with `PROMPT_COMMAND` and sed. (snippet) [Sanctum blog](https://blog.sanctum.geek.nz/tag/ps1/), [Unix & Linux SE mirror](https://backiee.wasmer.app/http_unix_stackexchange_com/a/491339)

### Inferences
- Three conventions show up: full path (cmd, PowerShell, Bash's `\w` by default), `~` for home (Bash, zsh, fish), and per-parent abbreviation (fish, zsh `%~` only when the result is shorter). A beginner sees the full path in two of the three main Windows and default-Bash cases, so full path is the most expected default. Abbreviation is an enhancement, not a requirement.
- A tiny OS has no `$HOME` unless one is defined. The `~` rule needs a defined home (for example `/`).
- Since rc's default prompt shows no directory at all, a bare prompt is also a valid minimal design.

### Gaps
- What beginners expect from a prompt: no source obtained.
- Bash `\w` and `\W` exact escape text: the PROMPTING section was not reached in the man page fetch (the text was read only up to the PARAMETERS / EXPANSION region). Only the `\w` description from the snippet above is available.
- Other zsh prompt escapes beyond `%~`, `%d`, `%c`: not checked.

---

## 2. Overwrite safety: warn, block, or silent replace

### Takeaway
The main shells differ. Bash's `>` overwrites silently unless `set -C` (noclobber) is on, and then `>|` forces it. PowerShell's `Out-File` overwrites silently by default and offers `-NoClobber` to block. PowerShell's `Set-Content` has no `-NoClobber` at all. Windows `copy` prompts before overwriting in interactive use, but not in batch scripts. So silent replacement is the default in Bash and PowerShell. I found no source that establishes a consensus on warn versus block for beginners.

### Cited Findings
- POSIX shell redirection: "Output redirection using the '>' format shall fail if the noclobber option is set and the file named by the expansion of word exists and is a regular file. Otherwise, redirection using the '>' or '>|' formats shall cause the file ... to be created and opened for output." [POSIX.1-2017, Shell Command Language, sec. 2.7.2](https://pubs.opengroup.org/onlinepubs/9699919799/utilities/V3_chap02.html)
- Bash (2.05a archive): with `noclobber` set, `>` "will fail if the file ... exists and is a regular file"; `>|` "is attempted even if the file named by word exists." [GNU Bash 2.05a manual, sec. 3.6.2](https://ftp.gnu.org/old-gnu/Manuals/bash-2.05a/html_node/bashref_39.html) (The current manual was not reachable.)
- PowerShell `Out-File`: "`-NoClobber` prevents an existing file from being overwritten and displays a message that the file already exists. By default, if a file exists in the specified path, `Out-File` overwrites the file without warning." Alias `NoOverwrite`. [Out-File, Microsoft Learn](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.utility/out-file)
- PowerShell `Set-Content`: "replaces the existing content." The syntax lists `-Force`, `-Confirm`, `-WhatIf`, and no `-NoClobber`. [Set-Content, Microsoft Learn](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.management/set-content)
- Windows `copy`: `/y` "suppresses prompting to confirm that you want to overwrite an existing destination file." By default "you are prompted when you replace this setting, unless the copy command is executed in a batch script." The `COPYCMD` variable can preset `/y`. [copy, Microsoft Learn](https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/copy)

### Inferences
- The examples split three ways: silent replace (Bash default `>`, PowerShell `Out-File`, `Set-Content`), prompt (Windows `copy` interactive), and block only when asked (Bash `set -C`, PowerShell `Out-File -NoClobber`).
- For a teaching OS, one defensible design is: refuse the overwrite by default with a message that names the file and the flag that forces it, and provide the force flag. A silent-replace design is also defensible, because Bash and PowerShell both do it. Pick one and document it.
- Whichever default is chosen, print the file name in any refusal or overwrite message.

### Gaps
- Coreutils `cp -i`, `cp -n`, `mv -i`, `mv -n`, `rm -i`: the coreutils manual returned HTTP 429. Not verified. The POSIX `cp`/`mv`/`rm` pages were not fetched.
- "Warn-by-default is the norm" or "silent replace is the norm": no source found for either.
- Beginner usability research on destructive commands: not searched.

---

## 3. Text tools without pipes or redirection

### Takeaway
xv6's `grep` and `wc` accept file names as arguments, and with no file name they read from file descriptor 0 (stdin). In a shell without pipes, the stdin branch can be dropped, and each tool can take file arguments and print a usage error when none are given.

### Cited Findings
- xv6 `grep`: if `argc <= 2`, it calls `grep(pattern, 0)` (stdin). Otherwise it opens each file from `argv[2]` in turn, and on open failure prints `grep: cannot open %s` and exits 1. [xv6-riscv user/grep.c](https://github.com/mit-pdos/xv6-riscv/blob/riscv/user/grep.c)
- xv6 `wc`: if `argc <= 1`, it runs `wc(0, "")` on stdin. Otherwise it loops over each file argument, prints `wc: cannot open %s` and exits 1 on failure, and prints one line per file with `printf("%d %d %d %s\n", l, w, c, name)`. [xv6-riscv user/wc.c](https://github.com/mit-pdos/xv6-riscv/blob/riscv/user/wc.c)
- xv6 `user/` listing (partial): includes `grep.c`, `wc.c`, `cat.c`, `echo.c`, `ls.c`, `rm.c`, `ln.c`, `mkdir.c`, `sh.c`. No `head.c` or `tail.c` in the listed set. [xv6-riscv user/](https://github.com/mit-pdos/xv6-riscv/tree/riscv/user)

### Inferences
- The stdin branch is unreachable without pipes, so removing it avoids dead code. The tool then needs only the file-argument loop.
- `wc` per-file output with one line per file matches the xv6 code. A total line is not in the xv6 code I read.
- `grep` in xv6 does not print the file name with each match in the code I read, so a teaching shell can copy that behavior or add the name. Decide and document.

### Gaps
- `head` and `tail`: no xv6 source checked. The xv6 listing is partial.
- How other minimal shells (Plan 9 rc, minimal BSD) implement these tools: not researched.
- The xv6 Book's description of pipelines (`dup` before `exec`): seen only as a snippet, not fetched. (snippet) [xv6 Book, Ch.1](https://xv6-guide.github.io/xv6-riscv-book/Ch1.S3.html)

---

## 4. Clock: date and uptime

### Takeaway
`date` is a POSIX utility, so a shell that lacks a real-time clock should still print something honest rather than vanish. xv6 declares an `uptime()` system call, but I did not confirm a user command for it.

### Cited Findings
- POSIX utilities index lists `date`. [POSIX utilities index](https://pubs.opengroup.org/onlinepubs/9699919799/idx/utilities.html)
- POSIX utilities index does **not** list `uptime`. [POSIX utilities index](https://pubs.opengroup.org/onlinepubs/9699919799/idx/utilities.html)
- xv6 `user/user.h` declares the system call wrapper `int uptime(void);`. It does not declare `date`, `whoami`, or `hostname`. [xv6-riscv user/user.h](https://raw.githubusercontent.com/mit-pdos/xv6-riscv/riscv/user/user.h)

### Inferences
- If the OS has a tick counter, `uptime` is a natural substitute for the clock, and the system call already exists in xv6's user library. Whether a user command `uptime` exists is not confirmed (no `uptime.c` was in the partial listing).
- A clockless `date` could print a plain message such as `date: no real-time clock on this system`. This is my design suggestion, not a sourced convention.

### Gaps
- Whether a `uptime` command exists in xv6: the partial listing is inconclusive. The full `user/` listing was not fetched.
- What other tiny OSes print for `date`: not found.

---

## 5. Users and hostname

### Takeaway
POSIX defines `id` and `logname` for identity, and does not define `whoami` or `hostname` in its utilities index. A single-user OS can still print a fixed name, but I found no source for what tiny OSes print.

### Cited Findings
- POSIX utilities index lists `id`, `logname`, `uname`, and `date`; it does not list `whoami` or `hostname`. [POSIX utilities index](https://pubs.opengroup.org/onlinepubs/9699919799/idx/utilities.html)
- xv6 `user/user.h` does not declare `whoami` or `hostname`. [xv6-riscv user/user.h](https://raw.githubusercontent.com/mit-pdos/xv6-riscv/riscv/user/user.h)

### Inferences
- Since POSIX has no `whoami`, a teaching OS can add one as an OS-specific tool. If the OS has no users, printing a fixed name (for example `root` for single-user) is a design choice. Whether `hostname` should exist depends on whether there is a network.
- With no network, `hostname` can print a fixed name or report `no network configured`. Pick one and document it.

### Gaps
- What single-user systems print for `whoami`: no source found (the Plan 9 `cons(3)` search was not run).
- Exact `id -un` behavior: not fetched.

---

## 6. Long input lines and argument limits

### Takeaway
POSIX gives minimums, not maximums. `LINE_MAX` (minimum 2048) limits a utility's input line when it processes text. `ARG_MAX` (minimum 4096) limits the size of arguments passed to `exec`, including environment data. A small shell should pick its own limit for a typed command line, and reject longer lines with a clear message.

### Cited Findings
- `LINE_MAX`: "the maximum length, in bytes, of a utility's input line (either standard input or another file), when the utility is described as processing text files. The length includes room for the trailing <newline>." Minimum: `_POSIX2_LINE_MAX` = 2048. [POSIX.1-2017 basedefs limits.h](https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/limits.h.html)
- `ARG_MAX`: "Maximum length of argument to the exec functions including environment data." Minimum: `_POSIX_ARG_MAX` = 4096. [POSIX.1-2017 basedefs limits.h](https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/limits.h.html)
- The minimums rule: "For each of these limits, a conforming implementation shall provide a value at least this large or shall have no limit." [POSIX.1-2017 basedefs limits.h](https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/limits.h.html)

### Inferences
- A typed command line is not a utility's input file and not an `exec` argument list, so neither POSIX limit applies directly. The shell is free to choose. A fixed buffer of 2048 bytes matches the LINE_MAX minimum and is a safe default for a small shell.
- When the line is too long, the shell should refuse it with `line too long` and not silently truncate it.

### Gaps
- Bash's real limit on a typed line: the Bash manual was not reachable. "Bash has no fixed limit" is unverified.
- `execve(2)` man page on argument-size limits (suggested by the review): not fetched.
- Failed checks: GNU Bash current manual and coreutils `cp`/`mv`/`rm` pages (gnu.org) returned HTTP 429. POSIX `whoami` page returned HTTP 404 (replaced by the utilities index). The Bash PROMPTING section (the `\w` escape text) was not reached. Plan 9 `cons(3)` was not searched. The xv6 `user/` directory was read only as a partial listing.
