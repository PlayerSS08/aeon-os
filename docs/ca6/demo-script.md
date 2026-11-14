# Aeon: demonstration script (CA-6), about 12 minutes

Start: `powershell -ExecutionPolicy Bypass -File tools\run-qemu.ps1 -Build -Disk` from `S:\Works\Projects\Aeon\OS`. Click inside the window to type. The right column is what to say while the command runs. The order follows the assignment's layers 1 to 10.

| # | Layer | Type | Point at | Say |
|---|---|---|---|---|
| 1 | 1 Boot | (wait for boot) | Splash, then the boot log with stages | "UEFI loads the signed image. The kernel starts, logs numbered stages and stays up." |
| 2 | 2 Console | `info`, then `log` | Levels (INFO, WARN, ERROR) and `key=value` fields | "Three kinds of message: information, warnings and errors, and structured task and debug output." |
| 3 | 3 Tasks | `ps` | `idle`, `status`, `shell` with states and ticks | "A task is a slot with a state, a stack and a tick counter." |
| 4 | 4 Ready queue | `queue`, then `spawn`, then `queue` again | The queue empty, then listing waiting workers; "queue matches the task states: yes" | "Tasks are inserted at the back, the scheduler takes the front, a killed task is removed from wherever it is." |
| 5 | 5 Round robin | `trace` | The last 12 switches: tick, from, to, reason | "Every context switch is recorded with its reason. Here the same four tasks take turns, one tick each." |
| 6 | 6 Timer | `trace` again after a few seconds | `timer` count growing | "These workers never yield, so every switch away from them is the timer. This is preemption." |
| 7 | 6 Cooperative | `kill` the workers, `trace reset`, `spawn coop`, wait 3 s, `trace` | `yield` count in thousands | "Now the workers give the CPU up themselves: the switch rate is about 24 times higher, because it is no longer limited by the 100 Hz tick." |
| 8 | 5 Completion | `kill` the coop workers, `job 600 300 150`, then `ps` | Three jobs finish shortest first with their times; `ps` shows them gone | "Ready, running, terminated. The turnaround ratios match what equal-share round robin predicts." |
| 9 | 7 Problem | `race` | `expected 40000, got 20000, lost 20000` | "The shared-resource problem: two tasks add to one counter. When the timer switches in the middle of read-modify-write, additions are lost." |
| 10 | 7 Fix | `race lock` | `expected 40000, got 40000, lost 0` | "The same tasks with a spinlock around the read-modify-write: interrupts are off inside it, so no switch can happen in the middle." |
| 11 | 7 Blocking | `block`, `ps`, `wake` | `blocked` with 0 ticks, then `woke 1 task(s)` | "A task on a wait queue is out of the ready queue and uses no CPU." |
| 12 | 8 Memory | `mem`, `alloc 64`, `alloc 99999999`, `mem` | Allocation time; the refusal says "nothing changed"; the heap numbers identical | "A frame bitmap under a first-fit heap. A request that cannot fit is refused cleanly." |
| 13 | 9 System interface | `run hello`, then `run fault`, then `run gp` | The greeting; `killed: #PF`, `#GP`; shell still works | "A ring-3 program reaches the kernel only through `syscall`. A hostile one is stopped by the CPU and only that task dies." |
| 14 | Specialization | `security` | NX, W^X, SMEP, SMAP, Secure Boot, TPM lines | "Security and isolation: what protects the kernel from programs, from a bad boot, and from its own interrupts." |
| 15 | 10 Integration | `spawn`, `job 300`, `race lock`, `alloc 1024` together, then `queue` | All finish; queue consistent | "Everything at once: tasks, queue, timer, scheduler, locking and memory." |
| 16 | Evidence | Open `docs\ca6\evidence\` | `trace-gantt.png`, `race-lock.png`, `switch-reasons.png`, `results.csv` | "Every number in the report comes from this CSV: 111 checks, 0 failed." |

If something goes wrong live, say what you see and use `reboot`; recovery is part of the design (boot counting).

## Backup evidence if the VM will not start

`docs\ca6\evidence\logs\*.serial.log` hold the raw kernel output of every test VM, `trace-preemptive.txt` and `trace-cooperative.txt` hold the scheduler traces the Python parser reads, and `target\test-shots\` has screenshots from the boot suite.
