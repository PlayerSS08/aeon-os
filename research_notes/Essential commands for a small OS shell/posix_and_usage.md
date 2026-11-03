# POSIX Utilities and Real-World Command Usage

Research date: 2026-11-03. All URLs were fetched or searched during this session. Items marked **[unverified]** were not confirmed from a fetched source.

Note on the POSIX edition: the fetched index page (https://pubs.opengroup.org/onlinepubs/9699919799/idx/utilities.html) shows no version text. The `9699919799` path is the Open Group base-spec directory that I understand to be POSIX.1-2017 (Issue 7) **[unverified from the page itself]**. The Issue 8 (POSIX.1-2024) page I fetched (https://pubs.opengroup.org/onlinepubs/9799919799/utilities/V3_chap02.html) is the Shell Command Language chapter, not the utilities index, so it gave no utility list.

## Key Question 1: The POSIX utilities list, grouped by purpose

### Takeaway
The index lists about 160 utility names. Several commands in the user's candidate list are **not** in it: `seq`, `hostname`, `whoami`, `stat`, `rev`, and `hexdump`. Their POSIX equivalents are `od`, `uname -n`, and `id`/`logname`.

### Cited Findings
Full index as fetched, in order (https://pubs.opengroup.org/onlinepubs/9699919799/idx/utilities.html):

admin, alias, ar, asa, at, awk, basename, batch, bc, bg, c99, cal, cat, cd, cflow, chgrp, chmod, chown, cksum, cmp, comm, command, compress, cp, crontab, csplit, ctags, cut, cxref, date, dd, delta, df, diff, dirname, du, echo, ed, env, ex, expand, expr, false, fc, fg, file, find, fold, fort77, fuser, gencat, get, getconf, getopts, grep, hash, head, iconv, id, ipcrm, ipcs, jobs, join, kill, lex, link, ln, locale, localedef, logger, logname, lp, ls, m4, mailx, make, man, mesg, mkdir, mkfifo, more, mv, newgrp, nice, nl, nm, nohup, od, paste, patch, pathchk, pax, pr, printf, prs, ps, pwd, qalter, qdel, qhold, qmove, qmsg, qrerun, qrls, qselect, qsig, qstat, qsub, read, renice, rm, rmdel, rmdir, sact, sccs, sed, sh, sleep, sort, split, strings, strip, stty, tabs, tail, talk, tee, test, time, touch, tput, tr, true, tsort, tty, type, ulimit, umask, unalias, uname, uncompress, unexpand, unget, uniq, unlink, uucp, uudecode, uuencode, uustat, uux, val, vi, wait, wc, what, who, write, xargs, yacc, zcat

Absent from that list (checked by name): `seq`, `hostname`, `whoami`, `stat`, `rev`, `hexdump`, `tar`, `cpio`. The page shows `pax` but no separate `tar` or `cpio` entry.

Grouping by purpose (my grouping of the names above, not the spec's own categories):

- **Files and directories:** cd, cp, ln, link, ls, mkdir, mkfifo, mv, pathchk, pwd, rm, rmdir, touch, unlink, file, find, du, df, split, csplit, pax, dd, cmp, diff, comm, chmod, chgrp, chown, umask
- **Text and filters:** awk, cat, cut, ed, ex, expand, unexpand, fold, grep, head, tail, join, nl, paste, pr, sed, sort, tr, tee, uniq, wc, vi, more, strings, iconv, printf, echo, expr, bc, read
- **Shell and job control:** alias, unalias, bg, fg, jobs, command, getopts, hash, type, ulimit, wait, sh, test, true, false, time, nohup, nice, renice, kill, fc, xargs, env
- **System info and identity:** date, cal, getconf, id, logname, locale, localedef, uname, who, tty, stty, tabs, tput, mesg, ps, ipcs, ipcrm
- **Scheduling and processes:** at, batch, crontab, time, kill, ps, nohup, nice, fuser
- **Networking and mail:** mailx, talk, write, uucp, uux, uustat, uuencode, uudecode, lp, logger
- **Development:** make, ar, nm, strip, lex, yacc, c99, cflow, ctags, cxref, fort77, m4, patch, cksum, compress, uncompress, zcat, man
- **Source control (SCCS):** admin, delta, get, prs, rmdel, sact, sccs, unget, val, what
- **Print and job queue:** lp, qalter, qdel, qhold, qmove, qmsg, qrerun, qrls, qselect, qsig, qstat, qsub

Sources: https://pubs.opengroup.org/onlinepubs/9699919799/idx/utilities.html

### Inferences
- A tiny shell with no pipes, processes, users, or network cannot honestly claim POSIX conformance for most of the list. A subset is a "POSIX-style" command set, not a conforming one.

### Gaps
- The index page has no version or date text, so the edition is inferred from the URL path only.
- The fetched index does not list the special built-ins in full. The Issue 8 page that mentions them (https://pubs.opengroup.org/onlinepubs/9799919799/utilities/V3_chap02.html) is only partly readable in the fetch.
- The GNU coreutils index (https://www.gnu.org/software/coreutils/manual/html_node/index.html) returned HTTP 429 twice, so it was not read.

## Key Question 2: Which commands people actually use most

### Takeaway
No rigorous, large-sample survey of shell command frequency turned up. The only frequency data I could read is one self-selected site dataset (Webminal, 2017 and 2019), where `ls`, `pwd`, `clear`, `cd ..`, and `ls -l` lead. Individual shell-history posts agree that `ls`, `cd`, `sudo`, and `man` are common, but those are anecdotes.

### Cited Findings
- Webminal's user-submitted dataset, 2019 status (last updated May 2019): `ls` 1,044,749 (about 9.55% by the page's total); `clear` 354,638; `pwd` 335,139; `cd ..` 225,533; `ls -l` 171,348. The 2017 table ranks `ls` first, then `pwd`, `clear`, `cd ..`, `ls -l`. Source: http://webminal.org/fulc/ . The page says the data is "from users around the world" and gives no sampling method. Its percentages are my own calculation against a total the page shows near the top.
- Superuser and Kubuntu forum histories (search snippets, not fetched): one user's history has `ls` first, then `python3`, `cd`, and `clear`; another has `sudo` at 234 uses, then `cd` and `man`. Sources: https://www.kubuntuforums.net/forum/general/community-cafe/42780-my-top-ten-most-used-shell-commands (Kubuntu thread) and a Superuser thread whose URL was not captured in the search output (**[unverified: snippet only]**). A related search result page is https://notes.mrkaran.dev/Shell/Most%20used%20shell%20commands/ (not fetched).
- A "how to find your most used commands" method is the pipeline `history | awk '{print $2}' | sort | uniq -c | sort -nr | head -5` (snippet from https://besthub.dev/articles/how-to-quickly-find-your-most-frequently-used-linux-commands-da83471abe1f; **[unverified: snippet only]**).
- A Command Linux page reports Git as the most-used command-line tool after basic navigation. It cites no methodology, so I rate it low confidence: https://commandlinux.com/statistics/linux-command-usage-frequency-analysis/ (**[unverified]**).

### Inferences
- The most-used commands in every source I found are navigation and listing: `ls`, `cd`, `pwd`, `clear`. These all need only a filesystem and a prompt, not processes or users.
- `sudo` and `python3` are high in some histories, but they need privilege or external programs, so they are not realistic for this shell.

### Gaps
- Linux Journey (https://linuxjourney.com/lesson/the-shell) redirects to https://labex.io/lesson/the-shell. The redirect was not followed, so I have no beginner-tutorial command list from it.
- The Ubuntu command-line tutorials and the GNU coreutils overview were not read in this session (GNU returned HTTP 429).
- No published large-sample statistic on command frequency was found.

## Key Question 3: Which POSIX utilities work with only an in-memory filesystem and one shell

### Takeaway
A useful split for the shell: utilities that only read or write files and print text (cp, mv, touch, head, tail, wc, grep, find, od, sleep, basename, dirname, echo, cat, tr, cut, uniq, test, true, false) are feasible. Utilities that need permissions, users, processes, a clock, or networking (chmod, chown, ps, kill, ping, date, ssh) need more of the OS. `seq`, `hostname`, `whoami`, `stat`, `rev`, and `hexdump` are not POSIX at all.

### Cited Findings
Verified from the spec pages fetched in this session:
- `cp` copies file contents and needs `-R` for directories: "If the **-R** option was not specified, _cp_ shall write a diagnostic message to standard error" (https://pubs.opengroup.org/onlinepubs/9699919799/utilities/cp.html).
- `touch` calls `creat()` for a missing file and `utimensat()` for an existing one. `-c` suppresses creation (https://pubs.opengroup.org/onlinepubs/9699919799/utilities/touch.html).
- `head`, `tail`, `wc`, `grep`, `find`, `sleep`, `od`, and `cp` are all listed in the index (https://pubs.opengroup.org/onlinepubs/9699919799/idx/utilities.html).
- `sleep` takes a non-negative decimal integer number of seconds: "A non-negative decimal integer specifying the number of seconds for which to suspend execution." Fractional seconds are not part of the spec (https://pubs.opengroup.org/onlinepubs/9699919799/utilities/sleep.html).

Dependencies that disqualify a command for this shell (my classification):
- `chmod`, `chown`, `chgrp`, `umask`: need a permissions and ownership model.
- `ps`, `kill`, `nice`, `renice`, `nohup`, `wait`, `jobs`, `fg`, `bg`, `time`: need processes or job control.
- `date`, `at`, `batch`, `crontab`, `sleep` (for real time): need a clock.
- `who`, `id`, `logname`, `mesg`, `write`, `talk`: need users or sessions.
- `mailx`, `uucp`, `uux`, `lp`: need networking or print queues.
- `pipe` users (`tee`, `xargs`, and any filter used with `|`): need pipes. They still work on file arguments without a pipe.

### Inferences
- A file-only subset that fits the constraints: `cat`, `cp`, `mv`, `rm`, `ln`, `mkdir`, `rmdir`, `touch`, `ls`, `pwd`, `cd`, `find`, `head`, `tail`, `wc`, `grep`, `cut`, `tr`, `uniq`, `sort`, `od`, `echo`, `printf`, `test`, `true`, `false`, `basename`, `dirname`, `sleep` (as a no-op delay). Each is classified from the spec's stated behavior, not from a test run.
- `ln` and `link` may need the notion of a second name for an inode. I did not check whether `ln` is meaningful in an in-memory filesystem.

### Gaps
- Not verified: `mv`, `ls`, `mkdir`, `rm`, `cut`, `tr`, `uniq`, `sort`, `echo`, `cat`, `basename`, `dirname`, `test`, `true`, `false`, `printf`, `pwd`, `cd`. Their spec pages were not fetched in this session, so their dependencies are inferred from their names.

## Key Question 4: Minimal behaviour of the listed commands

### Takeaway
The smallest useful forms verified from spec pages are below. Commands marked "not POSIX" have no POSIX definition, so their behaviour comes from GNU or BSD documentation, which I did not fetch.

### Cited Findings (POSIX.1-2017 pages unless noted)
- **cp**: `cp source_file target_file` copies the contents of the source to the target, and "If the target already exists as a regular file, the existing file is overwritten." Synopsis: `cp [-Pfip] source_file target_file` (https://pubs.opengroup.org/onlinepubs/9699919799/utilities/cp.html).
- **mv**: not fetched. **[unverified]**
- **touch**: `touch file` creates a missing file with `creat()` and sets its times with `futimens()`. An existing file gets its times updated with `utimensat()` and its contents left alone. `-c` skips creation without a message (https://pubs.opengroup.org/onlinepubs/9699919799/utilities/touch.html).
- **head**: synopsis `head [-n number] [file...]`. "If no options are specified, _head_ shall act as if **-n 10** had been specified." The spec has no `-c` option: "There is no **-c** option (as there is in _tail_)" (https://pubs.opengroup.org/onlinepubs/9699919799/utilities/head.html).
- **tail**: synopsis `tail [-f] [-c number | -n number] [file]`. "If neither **-c** nor **-n** is specified, **-n** 10 shall be assumed." `-c` counts bytes, and `+` counts from the start (https://pubs.opengroup.org/onlinepubs/9699919799/utilities/tail.html).
- **wc**: synopsis `wc [-c|-m] [-lw] [file...]`. Default output is one line per file in the order newlines, words, bytes, filename: `"%d %d %d %s\n"`. "When any option is specified, wc shall report only the information requested" (https://pubs.opengroup.org/onlinepubs/9699919799/utilities/wc.html).
- **grep**: default regex is BRE. Exit status is 0 if a line was selected, 1 if none, and greater than 1 on error. `-q` writes nothing to stdout. `-s` suppresses messages for nonexistent or unreadable files. Synopsis `grep [-E|-F] [-c|-l|-q] [-insvx] pattern_list [file...]` (https://pubs.opengroup.org/onlinepubs/9699919799/utilities/grep.html).
- **find**: synopsis `find [-H|-L] path... [operand_expression...]`. "If no _expression_ is present, **-print** shall be used as the expression." So `find .` is the same as `find . -print` (https://pubs.opengroup.org/onlinepubs/9699919799/utilities/find.html).
- **sleep**: `sleep time`, where time is a non-negative decimal integer of seconds. Exit 0 on completion, greater than 0 on error. Implementations must accept up to 2147483647 seconds (https://pubs.opengroup.org/onlinepubs/9699919799/utilities/sleep.html).
- **od**: default output is `-t oS`, an octal dump of 2-byte words. `-t` takes types a, c, d, f, o, u, x, with optional byte-count digits, e.g. `-t x1` for hex bytes. Repeated identical lines print as `*` unless `-v` is given (https://pubs.opengroup.org/onlinepubs/9699919799/utilities/od.html).
- **seq** (not POSIX; GNU coreutils): `seq 3` prints 1, 2, 3 one per line. `seq 2 5` prints 2, 3, 4, 5. Increment defaults to 1 even when first is greater than last. Source: https://www.gnu.org/software/coreutils/manual/html_node/seq-invocation.html.
- **whoami** (not POSIX): the POSIX equivalents are `id` and `logname`. The fetched index lists both. Their behaviour was not fetched. **[unverified]**
- **hostname** (not POSIX): the POSIX equivalent is `uname -n`. `uname` is in the index. Its behaviour was not fetched. **[unverified]**
- **stat** (not POSIX): no POSIX replacement was confirmed. **[unverified]**
- **rev** and **hexdump** (not POSIX): `od` is the POSIX dumper. `rev` has no POSIX equivalent. Both need the GNU or util-linux documentation, which I did not fetch. **[unverified]**

### Inferences
- If the shell targets POSIX behaviour, use `od -t x1` instead of `hexdump`, `uname -n` instead of `hostname`, and `id -un` (**[unverified option]**) instead of `whoami`.
- `seq` is cheap to add by hand, but it is not POSIX, so a POSIX-only shell should leave it out.

### Gaps
- GNU coreutils pages for head, tail, wc, grep, cp, touch, and find were not read: the head page returned HTTP 429. Behaviour for those comes from POSIX only.
- Exact `stat`, `rev`, `hexdump`, `whoami`, and `hostname` behaviour: not verified.

## Sources
- POSIX utilities index: https://pubs.opengroup.org/onlinepubs/9699919799/idx/utilities.html
- POSIX cp: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/cp.html
- POSIX touch: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/touch.html
- POSIX head: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/head.html
- POSIX tail: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/tail.html
- POSIX wc: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/wc.html
- POSIX grep: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/grep.html
- POSIX find: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/find.html
- POSIX sleep: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/sleep.html
- POSIX od: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/od.html
- GNU seq: https://www.gnu.org/software/coreutils/manual/html_node/seq-invocation.html
- Webminal command frequency: http://webminal.org/fulc/
- Kubuntu forum history thread: https://www.kubuntuforums.net/forum/general/community-cafe/42780-my-top-ten-most-used-shell-commands
- Shell-history method (snippet): https://besthub.dev/articles/how-to-quickly-find-your-most-frequently-used-linux-commands-da83471abe1f
- Command Linux stats (low confidence): https://commandlinux.com/statistics/linux-command-usage-frequency-analysis/
