# Aeon: demonstration script (CA-6), about 10 minutes

Start: `powershell -ExecutionPolicy Bypass -File tools\run-qemu.ps1 -Build -Disk` from `G:\Aeon\OS`. Click inside the window to type. Say the sentence in the right column while the command runs.

| # | Type | Point at | Say |
|---|---|---|---|
| 1 | (wait for boot) | The splash, then the boot log scrolling | "UEFI loads the signed image. Boot stages are logged with timestamps; this is milestone 1 and 2." |
| 2 | `info` | Timer line (`lapic`) | "The timer is the local APIC at 100 Hz, calibrated against the TSC (milestone 5)." |
| 3 | `ps` | Tasks `idle`, `status`, `shell` | "A task is a slot with a state, a stack and a tick counter (milestone 3). The shell is a task too." |
| 4 | `spawn`, then `counters` twice, 2 seconds apart | Both numbers growing | "Two workers that never yield. Only the timer interrupt takes the CPU away, so this is preemption (milestone 4 and 5)." |
| 5 | `ps` twice | cpu-ticks of worker-a and worker-b nearly equal | "Round robin: equal ticks. Evidence run TC1 measured a fairness ratio of 1.0." |
| 6 | `kill` one worker's id, then `ps` | One worker left | "TC3, the boundary: the lone worker now has the whole CPU." |
| 7 | `block`, then `ps` | `waiter` is `blocked` with 0 ticks | "Synchronization (milestone 6): a task on a wait queue is not scheduled and uses no CPU." |
| 8 | `wake` | `woke 1 task(s)`, then `waiter: woke up` | "wake_all makes it ready again." |
| 9 | `mem`, then `run hello`, then `mem` | Free frames, heap line | "Frame bitmap plus first-fit heap (milestone 7). The first ring-3 launch takes a 64 KiB window once and reuses it." |
| 10 | `run hello` | `hello from ring 3` | "A ring-3 program, entering the kernel only through the syscall gate." |
| 11 | `run fault`, then `run gp` | `killed: #PF`, `#GP`; shell still works | "Specialization, security (milestone 8): the CPU stops the program and only that task dies. TC5." |
| 12 | `mkdir docs`, `cd docs`, `edit note`, two lines, `.`, `cd ..`, `tree` | The tree with sizes | "A RAM file system with folders and many-line files." |
| 13 | `security` | NX, W^X, SMEP, SMAP, Secure Boot, TPM lines | "What protects the kernel; `help` shows everything." |
| 14 | Show `docs\ca6\evidence\*.png` and `results.csv` | The four charts | "Every number in the report comes from this CSV. 73 checks, 0 failed." |

If something goes wrong live, say what you see and use `reboot`; recovery is part of the design (boot counting).

## Backup evidence if the VM will not start

`docs\ca6\evidence\logs\*.serial.log` hold the raw kernel output of every test VM, and `target\test-shots\` has screenshots from the boot suite.
