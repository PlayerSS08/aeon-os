# Smallest useful command sets for small and hobby OSes

Research date context: 2026-11-03. Scope: BusyBox, Toybox, xv6, SerenityOS, Redox OS (coreutils / uutils), Plan 9, and hobby OS guidance.

Status legend: **[verified]** = read from the cited page in this session. **[snippet]** = seen only in a search result, page not opened. **[unverified]** = from general knowledge or not checked; treat as a lead, not a fact.

Tool-call budget: 15 calls used. Several requested sources were not reachable (see Gaps).

---

## 1. xv6 user programs (MIT 6.1810 / xv6-riscv)

### Takeaway
xv6's `UPROGS` list has 20 programs. The core shell set is cat, echo, grep, kill, ln, ls, mkdir, rm, sh, wc, plus init for boot. The small set is the closest match to the brief's list.

### Cited Findings
- **[verified]** The `UPROGS` list in the xv6-riscv Makefile, in order: `cat, echo, forktest, grep, init, kill, ln, ls, mkdir, rm, sh, stressfs, usertests, grind, wc, zombie, logstress, forphan, dorphan, sync` (20 entries). Source: [xv6-riscv Makefile](https://github.com/mit-pdos/xv6-riscv/blob/riscv/Makefile)
- **[verified]** The `user/` directory contains 23 `.c` files: cat, dorphan, echo, forktest, forphan, grep, grind, init, kill, ln, logstress, ls, mkdir, printf, rm, sh, stressfs, sync, ulib, umalloc, usertests, wc, zombie. Note that `printf.c`, `ulib.c` and `umalloc.c` are libraries and are not in `UPROGS`. The GitHub page says a "View all files" link exists, so the listing may be incomplete. Source: [xv6-riscv user/](https://github.com/mit-pdos/xv6-riscv/tree/riscv/user)
- **[unverified]** The xv6 book (MIT 6.1810 2024 `book-riscv-rev4.pdf`) describes what each program does. My fetch returned raw PDF bytes, so I could not quote it. Source to check: [xv6 book, rev4](https://pdos.csail.mit.edu/6.1810/2024/xv6/book-riscv-rev4.pdf)
- **[unverified, general knowledge]** Rough purposes: `init` starts `sh` on the console; `sh` is the shell; `cat`/`echo`/`ls`/`mkdir`/`rm`/`ln`/`kill`/`wc`/`grep` do the usual Unix jobs. `sync` flushes the disk cache; `zombie`, `forktest`, `stressfs`, `usertests`, `grind`, `logstress`, `forphan`, `dorphan` are test or stress programs, not shell tools.

### Inferences
- A "must have" set for an xv6-style OS is roughly `sh, init, cat, echo, ls, mkdir, rm, ln, kill, wc, grep`. `sync` and the stress tests are "later".
- The xv6 list is a teaching set, not a POSIX conformance set. It has no `cp`, `mv`, `touch`, `pwd` or `chmod` in `UPROGS`. `cp` and `mv` are missing from the list entirely (confirmed by the Makefile). `pwd` and `touch` are also absent.

### Gaps
- Exact per-program descriptions from the xv6 book: the PDF could not be read. Need a text-extracted copy.
- Whether xv6 has an in-memory filesystem: not checked. xv6's filesystem is disk-backed. **[unverified]**

---

## 2. BusyBox applets

### Takeaway
BusyBox ships about 300+ applets. The documentation page gives the full list of names but no defconfig or minimal set. The "minimal" set has to come from the BusyBox config files, which I did not read.

### Cited Findings
- **[verified]** The BusyBox documentation lists applets by name in its COMMANDS section, including `cat, cp, chmod, cut, date, dd, df, echo, env, false, find, grep, head, id, kill, ln, ls, mkdir, mv, pwd, rm, rmdir, sed, sleep, sort, tail, tee, test, touch, tr, true, uname, uniq, wc, which, who, whoami, xargs, yes` (and many more). Source: [BusyBox docs](https://busybox.net/downloads/BusyBox.html)
- **[verified]** The same page says BusyBox "has been written with size-optimization and limited resources in mind" and that you should "Run 'make config' or 'make menuconfig' to select the functionality that you wish to enable." Source: [BusyBox docs](https://busybox.net/downloads/BusyBox.html)
- **[verified]** The documentation page does not mention a defconfig or minimal set in the part I read. The page text was 114k characters and I read only the first 100k, so the end of the page is unchecked.

### Inferences
- The BusyBox applet list is a superset. For a tiny system, pick applets from the list and drop the networking and init ones.
- Applet names in this list include `cp` and `mv`, which xv6 lacks.

### Gaps
- BusyBox "defconfig" and "minimal" config contents: not fetched. Check `configs/` in the BusyBox source tree (busybox.net does not have a stable link I could reach in this session).
- Ranking by how often tiny systems enable each applet: not found.

---

## 3. Toybox

### Takeaway
Toybox's `make defconfig` enables "all the commands compete enough to be useful." The command list itself is on a status page I could not reach as text.

### Cited Findings
- **[verified]** "In general, configuring toybox with 'make defconfig' enables all the commands compete enough to be useful." (sic: "compete" is the page's own wording) and "Configuring 'allyesconfig' enables partially implemented commands as well, along with debugging features." Source: [Toybox home](https://landley.net/toybox/)
- **[verified]** The page says the command list is on the status page, "updated each release," and the roadmap lists planned commands for 1.0. Source: [Toybox home](https://landley.net/toybox/)
- **[verified]** Toybox "aims to provide" one of the four packages needed for a minimal native development environment: "posix-ish command line utilities, a compiler, a C library, and a kernel." Source: [Toybox home](https://landley.net/toybox/)
- **[verified]** Google merged toybox into AOSP and "began shipping toybox in Android Marshmallow in 2015." Source: [Toybox home](https://landley.net/toybox/)

### Gaps
- The actual `toybox --help` output and the defconfig command list: not retrieved. The GitHub toys/README I tried returned HTTP 404 (URL: https://github.com/landley/toybox/blob/master/toys/README). Try the status page or the built `toybox` binary's help output.

---

## 4. SerenityOS Userland

### Takeaway
SerenityOS's Userland/Utilities directory has about 200 Unix-like utilities. I could not find a written history of which were added first.

### Cited Findings
- **[verified]** The Utilities directory contains `cat.cpp, cp.cpp, chmod.cpp, chown.cpp, cmp.cpp, cut.cpp, date.cpp, dd.cpp, df.cpp, diff.cpp, du.cpp, echo.cpp, env.cpp, expr.cpp, false.cpp, find.cpp, grep.cpp, head.cpp, id.cpp, kill.cpp, killall.cpp, ln.cpp, ls.cpp, sort.cpp, tail.cpp` (plus many more, first 100 entries shown, "View all files" link present). Source: [SerenityOS Userland/Utilities](https://github.com/SerenityOS/serenity/tree/master/Userland/Utilities)
- **[snippet]** A third-party architecture doc says the project has roughly 200 utilities in `Userland/Utilities/`, including `ls, cp, grep, tar`. Source: [mintlify SerenityOS userland](https://www.mintlify.com/serenityOS/serenity/architecture/userland) (third-party, not authoritative)
- **[snippet]** Wikipedia gives the initial public release as October 10, 2018. Source: [SerenityOS, Wikipedia](https://en.wikipedia.org/wiki/SerenityOS)
- **[snippet]** Dated commits from a mirror: "Userland: Add a basic xargs" (May 28, 2020) and an `nproc` utility (April 12, 2021). These are dated commits, not the first ones. Mirror: [RGBCube/serenity git mirror](https://git.rgbcu.be/RGBCube/serenity/src/commit/49f3323fa84a122612ad7ce5a8189b6131023fd0) (snippet only, not opened)

### Gaps
- Order in which core utilities were added: **not found**. Next step: `git log --reverse -- Userland/Utilities` on the SerenityOS repo, or the early devlogs by Andreas Kling.

---

## 5. Redox OS (uutils and Redox userland)

### Takeaway
Redox uses uutils (a Rust reimplementation of GNU coreutils) and its own userutils and extrautils. The Redox book lists about 25 coreutils, 12 userutils and 18 extrautils tools, plus binutils.

### Cited Findings
- **[verified]** The Redox book's coreutils list includes `ls, cp, cat, chmod, clear, dd, df, du, env, free, kill, ln, mkdir, ps, reset, shutdown, sort, stat, tail, tee, test, time, touch, uname, uptime, which`. Source: [Redox book, system tools](https://doc.redox-os.org/book/system-tools.html)
- **[verified]** Redox userutils: `getty, id, login, passwd, su, sudo, useradd, usermod, userdel, groupadd, groupmod, groupdel`. Source: [Redox book, system tools](https://doc.redox-os.org/book/system-tools.html)
- **[verified]** Redox extrautils includes `grep, less, man, tar, unzip, watch, gzip, gunzip, dmesg, calc, keymap`. Binutils: `hex, hexdump, strings`. Source: [Redox book, system tools](https://doc.redox-os.org/book/system-tools.html)
- **[verified]** Redox "uses the Rust implementation of the GNU Coreutils, uutils." Source: [Redox book, system tools](https://doc.redox-os.org/book/system-tools.html) (the page text as returned by the fetch tool)
- **[snippet]** uutils lists Redox OS as a tier 2 platform, "supported but not actively tested." Source: [uutils coreutils platforms](https://docs.rs/crate/coreutils/0.9.0/source/docs/src/platforms.md) (snippet only)
- **[snippet]** The older Redox coreutils repo says it is "DEPRECATED ... superseded by uutils/coreutils." Source: [redox-os/uutils](https://github.com/redox-os/uutils) (snippet)

### Gaps
- Which Redox command was added first: not found. Need the Redox commit history or the early Redox release notes.

---

## 6. Plan 9 from Bell Labs, section 1

### Takeaway
The Plan 9 section 1 index has many entries (exact count **[unverified]**, I did not count them). Its small-core set is `cat, cp, ls, mkdir, rm, grep, wc, sort, tee, touch, pwd, echo, sed, tr, uniq, date, sleep, test, uniq, kill, who, ps, mk, ed, awk, bind`. **`mv` is not in the index.**

### Cited Findings
- **[verified]** The index includes these names (aliases omitted): `0intro, 8a, 8c, 8l, ... awk, basename, bc, bind, cal, cat, chgrp, chmod, cmp, comm, cp, cpu, date, dc, dd, diff, du, echo, ed, expect, file, fmt, grep, gzip, join, kill, ls, mail, man, mkdir, mk, pwd, ps, rc, rm, rio, sed, seq, sleep, sort, split, strings, tail, tar, tee, test, time, touch, tr, uniq, uptime, who, wc, ...`. Source: [Plan 9 section 1 index](https://plan9.io/sys/man/1/INDEX.html)
- **[verified]** The index does not list `mv`. The brief's Plan 9 list assumed it; it is absent from this index. Source: same page.
- **[verified]** `pwd` is listed. `touch` is listed. `who` is listed. Source: same page.
- **[snippet]** The index describes section 1 as "general publicly accessible commands." Source: [plan9port intro (search snippet)](https://plan9port.googlesource.com/plan9/+/master/install.txt) (snippet only)

### Gaps
- Order of addition in Plan 9: not found.
- Whether Plan 9 has an in-memory filesystem (ramfs or the `/tmp` mount): not checked.

---

## 7. What hobby and teaching OSes add first

### Takeaway
I found no source on hobby kernels (blog_os, OSDev wiki, Redox early releases, SerenityOS early days) that gives an ordered list of first commands. The search returned only generic Linux shell guides.

### Cited Findings
- **[snippet]** A generic beginner guide lists `ls, cp, mkdir, rm, cd, cat, echo, exit, man` as the most frequently used shell commands. Source: [atlantic.net](https://www.atlantic.net/vps-hosting/how-to-linux-freebsd-basic-shell-commands/) (snippet only; not a hobby-OS source)
- **[snippet]** `which` tells you which binary the shell will run for a command name. Source: [nccu course page](https://www.cs.nccu.edu.tw/~lien/UNIX/firstclass.htm) (snippet only)

### Inferences (labelled as my reasoning, not sourced)
- The commands that recur across all the sources above are `cat, echo, ls, mkdir, rm, grep, wc, kill`. These are the "must have" set for a shell. `cp, mv, touch, ln, sort, tee, tail, head` are the next tier. `sed, awk, tar, gzip, less, man, vi, ps, df, du` are "nice to have".
- This ordering is my judgment from the lists, not a finding from a source.

### Gaps
- No hobby-OS source was found. Next queries to try: "OSDev wiki userspace utilities", "blog_os userspace shell", "Redox early release notes 2016".

---

## 8. Commands that need no pipes and touch only an in-memory filesystem

### Takeaway
Not answered. No source fetched in this session covers it.

### Gaps
- Not checked. In general, `echo`, `cat` (with redirection), `mkdir`, `rm`, `ls` and `touch` can be run against a RAM filesystem with no pipes. Whether any of the above is wired to in-memory only is **[unverified]**.

---

## Gaps and blocked sources

| Item | Status |
|---|---|
| xv6 book (per-program descriptions) | PDF fetched as raw bytes; not readable. |
| BusyBox defconfig / minimal config | Not fetched. Check `configs/` in busybox source. |
| Toybox `toybox --help` / defconfig list | Status page and GitHub README not reached (404). |
| SerenityOS first-added commands | Not found. Use `git log --reverse`. |
| Redox first-added commands | Not found. |
| Plan 9 first-added commands | Not found. |
| Hobby OS "must have" ordering | Not found from a hobby-OS source. |
| In-memory-only commands | Not answered. |

## Sources used
- [xv6-riscv Makefile](https://github.com/mit-pdos/xv6-riscv/blob/riscv/Makefile)
- [xv6-riscv user/](https://github.com/mit-pdos/xv6-riscv/tree/riscv/user)
- [xv6 book rev4 PDF (not readable)](https://pdos.csail.mit.edu/6.1810/2024/xv6/book-riscv-rev4.pdf)
- [BusyBox docs](https://busybox.net/downloads/BusyBox.html)
- [Toybox home](https://landley.net/toybox/)
- [SerenityOS Userland/Utilities](https://github.com/SerenityOS/serenity/tree/master/Userland/Utilities)
- [Plan 9 section 1 index](https://plan9.io/sys/man/1/INDEX.html)
- [Redox book, system tools](https://doc.redox-os.org/book/system-tools.html)
- [uutils coreutils platforms (snippet)](https://docs.rs/crate/coreutils/0.9.0/source/docs/src/platforms.md)
- [Wikipedia, SerenityOS](https://en.wikipedia.org/wiki/SerenityOS)
- [mintlify, SerenityOS userland (third-party)](https://www.mintlify.com/serenityOS/serenity/architecture/userland)
