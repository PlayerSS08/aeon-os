#!/usr/bin/env python3
"""CA-6 evidence collector for Aeon. Boots the kernel in QEMU (run inside WSL/Linux, like boot_test.py),
runs the six required test cases TC1 to TC6, measures scheduler, timer and memory behaviour, and writes
raw logs, results.csv, results.json and charts into docs/ca6/evidence/.

    python3 tests/ca6_evidence.py [--efi PATH] [--out DIR] [--repeats N] [--window SECONDS]

Nothing here changes the kernel. Every number in the report comes from a file this script wrote.
"""
import argparse
import csv
import json
import os
import re
import shutil
import statistics
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import boot_test as bt  # noqa: E402  (the Vm class and make_esp live there)

PS_ROW = re.compile(r"^(\d+)\s+(\S+)\s+(\S+)\s+(\d+)\s*$", re.M)
CHECKS = []  # (test case, description, passed, detail)
METRICS = []  # (test case, metric, value, unit)


def check(tc, what, ok, detail=""):
    CHECKS.append((tc, what, bool(ok), str(detail)[:300]))
    print(f"  {'PASS' if ok else 'FAIL'} [{tc}] {what}" + (f"  ({detail})" if detail and not ok else ""))


def metric(tc, name, value, unit=""):
    METRICS.append((tc, name, value, unit))
    print(f"  .... [{tc}] {name} = {value} {unit}")


class Session:
    """One booted VM with a helper that types a command and returns its output."""

    def __init__(self, ctx, name, cfg=None, disk=None):
        self.ctx = ctx
        self.name = name
        esp = bt.make_esp(ctx.work, ctx.efi, f"esp-{name}", cfg=cfg)
        self.vm = bt.Vm(esp, ctx.work, name=name, disk=disk)
        self.booted = self.vm.wait_for("aeon> ", 180, "serial")
        self.last_enter = time.time()

    def run(self, cmd, timeout=40):
        before = len(self.vm.serial_text())
        self.vm.type(cmd)
        self.vm.keys("ret")
        self.last_enter = time.time()
        end = time.time() + timeout
        while time.time() < end:
            out = self.vm.serial_text()[before:]
            if "aeon> " in out:
                break
            time.sleep(0.1)
        out = self.vm.serial_text()[before:].replace("\r", "")
        lines = out.split("\n")
        # Drop the echo of the typed command (first line) and the next prompt (last line).
        if lines and lines[0].strip() == cmd.strip():
            lines = lines[1:]
        if lines and lines[-1].strip() in ("aeon>", ""):
            lines = lines[:-1]
        return "\n".join(lines)

    def ps(self):
        out = self.run("ps")
        t = self.last_enter
        rows = {int(i): (n, s, int(k)) for i, n, s, k in PS_ROW.findall(out)}
        return rows, t

    def mem(self):
        out = self.run("mem")
        m = re.search(r"frames\s+(\d+) free of (\d+)", out)
        h = re.search(r"heap\s+(\d+) KiB used of (\d+) KiB, (\d+) free holes", out)
        return {
            "frames_free": int(m.group(1)) if m else -1,
            "frames_total": int(m.group(2)) if m else -1,
            "heap_used_kib": int(h.group(1)) if h else -1,
            "heap_holes": int(h.group(3)) if h else -1,
        }

    def save_log(self, outdir):
        os.makedirs(os.path.join(outdir, "logs"), exist_ok=True)
        with open(os.path.join(outdir, "logs", f"{self.name}.serial.log"), "w", encoding="utf-8") as f:
            f.write(self.vm.serial_text().replace("\r", ""))

    def close(self):
        self.vm.close()


def window_stats(before, after, t0, t1, names=None):
    """CPU use between two ps snapshots, for tasks present in both. Returns per-task ticks, the total
    elapsed ticks (all tasks together = 100 Hz timer ticks) and the measured tick rate."""
    common = [i for i in before if i in after]
    delta = {i: after[i][2] - before[i][2] for i in common}
    total = sum(delta.values())
    hz = total / (t1 - t0) if t1 > t0 else 0.0
    return common, delta, total, hz


def boot_seconds(sess):
    """Kernel uptime when the shell task started, from the log timestamps."""
    for line in sess.vm.serial_text().splitlines():
        if "task shell started" in line:
            m = re.match(r"\[\s*(\d+\.\d+)\]", line.strip())
            if m:
                return float(m.group(1))
    return None


def share_window(sess, seconds, extra_each=None):
    a, ta = sess.ps()
    time.sleep(seconds)
    b, tb = sess.ps()
    return a, b, ta, tb


def tc_boot(ctx, out):
    """Milestones 1 and 2: boot, console, diagnostics. Also captures the timer type and security state."""
    s = Session(ctx, "boot")
    try:
        check("M1", "kernel boots to the shell prompt", s.booted)
        boot_log = s.vm.serial_text().replace("\r", "")
        stages = [l for l in boot_log.splitlines() if "stage" in l.lower() or "boot complete" in l]
        check("M2", "boot log shows numbered stages and 'boot complete'", any("boot complete" in l for l in boot_log.splitlines()), len(stages))
        t = boot_seconds(s)
        metric("M1", "kernel uptime when the shell starts", t, "s")
        info = s.run("info")
        check("M5", "timer source is reported (lapic or pit)", "lapic" in info.lower() or "pit" in info.lower(), info[-200:])
        sec = s.run("security")
        cpu = s.run("cpu")
        ver = s.run("version")
        with open(os.path.join(out, "logs", "boot-commands.txt"), "w", encoding="utf-8") as f:
            f.write("== version\n" + ver + "\n== info\n" + info + "\n== cpu\n" + cpu + "\n== security\n" + sec + "\n")
        check("M2", "diagnostic commands answer (version, info, cpu, security)", all(x.strip() for x in (ver, info, cpu, sec)))
        s.save_log(out)
        return t
    finally:
        s.close()


def tc1(ctx, out, window, tag="TC1", name="tc1"):
    """Two controlled tasks under round robin, driven by the timer."""
    s = Session(ctx, name)
    try:
        check(tag, "boot", s.booted)
        spawn = s.run("spawn")
        check(tag, "two tasks created", "started worker-a" in spawn and "started worker-b" in spawn, spawn)
        c0 = s.run("counters").split()
        a, b, ta, tb = share_window(s, window)
        c1 = s.run("counters").split()
        common, delta, total, hz = window_stats(a, b, ta, tb)
        wk = {i: delta[i] for i in common if b[i][0].startswith("worker")}
        check(tag, "both workers got CPU (preemption works)", len(wk) == 2 and all(v > 0 for v in wk.values()), wk)
        check(tag, "both counters advanced", int(c1[0]) > int(c0[0]) and int(c1[1]) > int(c0[1]), f"{c0}->{c1}")
        vals = sorted(wk.values())
        ratio = vals[0] / vals[-1] if vals and vals[-1] else 0
        share = {b[i][0]: round(delta[i] / total, 4) for i in common if total}
        metric(tag, "worker CPU ticks (a, b)", sorted(wk.values()), "ticks")
        metric(tag, "fairness ratio min/max", round(ratio, 3), "")
        metric(tag, "measured timer rate", round(hz, 1), "Hz")
        metric(tag, "CPU share per task", json.dumps(share), "")
        check(tag, "fairness: the two workers are within 25% of each other", ratio >= 0.75, ratio)
        s.save_log(out)
        return {"ratio": ratio, "hz": hz, "share": share, "wk": wk, "counters": (c0, c1), "boot": boot_seconds(s)}
    finally:
        s.close()


def tc2(ctx, out, window):
    """Several tasks (3 to 5 where feasible): four workers, one user spinner, one blocked waiter."""
    tag = "TC2"
    s = Session(ctx, "tc2")
    try:
        check(tag, "boot", s.booted)
        s.run("spawn")
        s.run("spawn")
        spin = s.run("run spin")
        check(tag, "user program (ring 3) started", "started spin" in spin, spin)
        blk = s.run("block")
        check(tag, "a task blocked on a wait queue was created", "started waiter" in blk, blk)
        a, b, ta, tb = share_window(s, window)
        common, delta, total, hz = window_stats(a, b, ta, tb)
        workers = {i: delta[i] for i in common if b[i][0].startswith("worker")}
        spinner = {i: delta[i] for i in common if b[i][0] == "spin" or "spin" in b[i][0]}
        waiter = {i: delta[i] for i in common if b[i][0] == "waiter"}
        check(tag, "4 workers all got CPU", len(workers) == 4 and all(v > 0 for v in workers.values()), workers)
        check(tag, "the ring-3 task got CPU", bool(spinner) and all(v > 0 for v in spinner.values()), spinner)
        check(tag, "the blocked task used no CPU", bool(waiter) and all(v == 0 for v in waiter.values()), waiter)
        runnable = list(workers.values()) + list(spinner.values())
        ratio = min(runnable) / max(runnable) if runnable and max(runnable) else 0
        metric(tag, "runnable tasks", len(runnable), "")
        metric(tag, "fairness ratio min/max over runnable tasks", round(ratio, 3), "")
        metric(tag, "tasks alive", len(b), "")
        check(tag, "fairness: runnable tasks within 40% of each other", ratio >= 0.6, ratio)
        wake = s.run("wake")
        check(tag, "wake releases the blocked task", "woke 1 task(s)" in wake, wake)
        s.save_log(out)
        shares = {f"{b[i][0]}#{i}": round(delta[i] / total, 4) for i in common if total}
        metric(tag, "CPU share per task", json.dumps(shares), "")
        return {"ratio": ratio, "shares": shares, "n": len(runnable)}
    finally:
        s.close()


def tc3(ctx, out, window, share_two):
    """Boundary: the smallest meaningful configuration. One worker alone, then the refused edge cases."""
    tag = "TC3"
    s = Session(ctx, "tc3")
    try:
        check(tag, "boot", s.booted)
        spawn = s.run("spawn")
        ids = [int(x) for x in re.findall(r"as task (\d+)", spawn)]
        check(tag, "spawn reported two task ids", len(ids) == 2, spawn)
        k = s.run(f"kill {ids[1]}") if len(ids) == 2 else ""
        check(tag, "one worker stopped, one remains", "stopped" in k, k)
        a, b, ta, tb = share_window(s, window)
        common, delta, total, hz = window_stats(a, b, ta, tb)
        lone = [delta[i] / total for i in common if b[i][0].startswith("worker") and total]
        check(tag, "the lone worker keeps running", bool(lone) and lone[0] > 0, lone)
        metric(tag, "CPU share of the lone worker", round(lone[0], 3) if lone else None, "of all ticks")
        if lone and share_two:
            two = max(share_two.values()) if share_two else None
            metric(tag, "CPU share of one worker when two run (TC1)", two, "of all ticks")
            check(tag, "the lone worker gets about twice the share of a worker in TC1", lone[0] > 1.5 * two and lone[0] < 2.6 * two, f"{lone[0]:.3f} vs {two}")
        for bad, expect in (("kill 0", "no killable task 0"), ("kill 99", "no killable task 99"), ("kill x", "usage: kill <id>")):
            o = s.run(bad)
            check(tag, f"`{bad}` is refused", expect in o, o)
        s.save_log(out)
    finally:
        s.close()


def tc4(ctx, out, window):
    """Stress: 8 busy workers, a user spinner, ten ring-3 launches, many files. Memory must come back."""
    tag = "TC4"
    s = Session(ctx, "tc4")
    try:
        check(tag, "boot", s.booted)
        # The first ring-3 launch allocates the 64 KiB user window once; it is kept and reused (wiped
        # before every program). Warm it up so the baseline below measures real leaks only.
        s.run("run hello")
        time.sleep(1.5)
        m0 = s.mem()
        for _ in range(4):
            s.run("spawn")
        s.run("run spin")
        a, b, ta, tb = share_window(s, window)
        common, delta, total, hz = window_stats(a, b, ta, tb)
        busy = [delta[i] for i in common if b[i][0].startswith("worker") or "spin" in b[i][0]]
        ratio = min(busy) / max(busy) if busy and max(busy) else 0
        metric(tag, "busy tasks", len(busy), "")
        metric(tag, "fairness ratio min/max with 9 busy tasks", round(ratio, 3), "")
        share_one = (sum(busy) / len(busy)) / total if busy and total else 0
        metric(tag, "CPU share of one busy task with 9 busy tasks", round(share_one, 4), "of all ticks")
        check(tag, "every busy task got CPU", len(busy) == 9 and all(v > 0 for v in busy), busy)
        alive = s.run("echo still-alive")
        check(tag, "the shell stays responsive under load", "still-alive" in alive, alive)
        # Only one ring-3 program may run at a time (a design limit), so stop the spinner first. The
        # 8 busy kernel workers keep running, so the launches still happen under load.
        ps_spin, _ = s.ps()
        for i, (n, st, k) in ps_spin.items():
            if "spin" in n:
                s.run(f"kill {i}")
        launched = 0
        greeted = 0
        for _ in range(10):
            before_n = s.vm.serial_text().count("hello from ring 3")
            o = s.run("run hello")
            launched += "started hello" in o
            end = time.time() + 15
            while time.time() < end and s.vm.serial_text().count("hello from ring 3") <= before_n:
                time.sleep(0.2)
            greeted += s.vm.serial_text().count("hello from ring 3") > before_n
            time.sleep(0.5)  # let the program exit before the next launch
        metric(tag, "ring-3 launches accepted", launched, "of 10")
        metric(tag, "ring-3 launches that printed their greeting", greeted, "of 10")
        check(tag, "ten ring-3 launches under load all worked", launched == 10 and greeted == 10, f"launched={launched} greeted={greeted}")
        # stop everything we started, then compare memory
        ps_now, _ = s.ps()
        for i, (n, st, k) in ps_now.items():
            if n.startswith("worker") or "spin" in n:
                s.run(f"kill {i}")
        time.sleep(1.5)
        m1 = s.mem()
        metric(tag, "frames free before / after", f"{m0['frames_free']} / {m1['frames_free']}", "frames")
        check(tag, "no frame leak: free frames are back to the starting value", m1["frames_free"] == m0["frames_free"], f"{m0['frames_free']} vs {m1['frames_free']}")
        # heap: allocate by writing files, free by removing them
        h0 = s.mem()["heap_used_kib"]
        n_files = 20
        for i in range(n_files):
            s.run(f"write f{i} abc{i}", timeout=15)
        df = s.run("df")
        hmid = s.mem()["heap_used_kib"]
        for i in range(n_files):
            s.run(f"rm f{i}", timeout=15)
        h1 = s.mem()["heap_used_kib"]
        metric(tag, "heap used before / with 20 files / after", f"{h0} / {hmid} / {h1}", "KiB")
        check(tag, "20 files stored", f"{n_files} file(s)" in df, df)
        check(tag, "the heap returned to its baseline after removing the files", h1 <= h0 + 1, f"{h0} -> {h1}")
        s.save_log(out)
        return {"m0": m0, "m1": m1, "heap": (h0, hmid, h1), "ratio": ratio, "share_one": share_one}
    finally:
        s.close()


def tc5(ctx, out):
    """Failure: invalid and malicious requests are refused or contained; the kernel carries on."""
    tag = "TC5"
    s = Session(ctx, "tc5")
    try:
        check(tag, "boot", s.booted)
        # Warm up the one-time 64 KiB user window (see TC4) so the baseline measures real leaks only.
        s.run("run hello")
        time.sleep(1.5)
        m0 = s.mem()
        cases = [
            ("run nosuch", "unknown program"),
            ("run fault", "killed: #PF"),
            ("run gp", "#GP"),
            ("run badcall", "exited with code 0"),
            ("cat nothere", "no such file"),
            ("write", "bad name"),
            ("write a/b x", "no such folder"),
            ("rmdir nosuch", "no such folder"),
            ("rm nothere", "no such file"),
            ("sort 5 a", "not a number"),
            ("hlep", "Did you mean `help`"),
            ("dread 99999", "no disk"),
        ]
        for cmd, expect in cases:
            o = s.run(cmd)
            check(tag, f"`{cmd}` -> {expect}", expect in o, o[-160:])
        alive = s.run("echo kernel-alive")
        check(tag, "after every failure the kernel and shell still answer", "kernel-alive" in alive, alive)
        time.sleep(1)
        m1 = s.mem()
        check(tag, "failed ring-3 programs leaked no frames", m1["frames_free"] == m0["frames_free"], f"{m0['frames_free']} vs {m1['frames_free']}")
        ps, _ = s.ps()
        check(tag, "no ring-3 task is left behind", not any("fault" in n or n == "gp" or "badcall" in n for n, _, _ in ps.values()), ps)
        s.save_log(out)
    finally:
        s.close()


def tc_sync(ctx, out):
    """Milestone 6: the wait queue blocks a task without using CPU and wakes it on demand."""
    tag = "M6"
    s = Session(ctx, "sync")
    try:
        check(tag, "boot", s.booted)
        s.run("block")
        a, ta = s.ps()
        time.sleep(3)
        b, tb = s.ps()
        w = [i for i in b if b[i][0] == "waiter"]
        check(tag, "waiter is blocked", bool(w) and b[w[0]][1] == "blocked", b)
        check(tag, "a blocked task uses 0 CPU ticks over 3 s", bool(w) and b[w[0]][2] == a[w[0]][2], f"{a.get(w[0]) if w else None} -> {b.get(w[0]) if w else None}")
        wake = s.run("wake")
        check(tag, "wake releases exactly one waiter", "woke 1 task(s)" in wake, wake)
        time.sleep(1)
        check(tag, "the woken task ran", "waiter: woke up" in s.vm.serial_text())
        wake2 = s.run("wake")
        check(tag, "a second wake finds nobody", "woke 0 task(s)" in wake2, wake2)
        s.save_log(out)
    finally:
        s.close()


def tc6(ctx, out, window, repeats):
    """Repeatability: run TC1 again in fresh VMs and compare."""
    tag = "TC6"
    runs = []
    for i in range(repeats):
        runs.append(tc1(ctx, out, window, tag="TC6", name=f"tc6-run{i + 1}"))
    ratios = [r["ratio"] for r in runs]
    hzs = [r["hz"] for r in runs]
    boots = [r["boot"] for r in runs if r["boot"] is not None]
    metric(tag, "fairness ratios", [round(x, 3) for x in ratios], "")
    metric(tag, "timer rates", [round(x, 1) for x in hzs], "Hz")
    metric(tag, "boot times (kernel uptime at shell start)", boots, "s")
    check(tag, "all repeats keep the fairness ratio above 0.75", all(r >= 0.75 for r in ratios), ratios)
    if len(hzs) > 1:
        spread = (max(hzs) - min(hzs)) / statistics.mean(hzs)
        metric(tag, "timer-rate spread across repeats", round(spread * 100, 2), "%")
        check(tag, "timer rate repeats within 10%", spread < 0.10, spread)
    if len(boots) > 1:
        bs = (max(boots) - min(boots)) / statistics.mean(boots)
        metric(tag, "boot-time spread across repeats", round(bs * 100, 2), "%")
        check(tag, "boot time repeats within 25%", bs < 0.25, bs)
    # deterministic output must be identical every time
    outs = []
    for i in range(2):
        s = Session(ctx, f"tc6-det{i + 1}")
        try:
            outs.append((s.run("sort 5 3 9 1").strip().splitlines() or [""])[-2:])
        finally:
            s.close()
    check(tag, "a deterministic command gives identical output in two fresh boots", outs[0] == outs[1], outs)
    return runs


def make_charts(out, data):
    try:
        import matplotlib
        matplotlib.use("Agg")
        import matplotlib.pyplot as plt
    except Exception as e:  # charts are optional; the CSV is the evidence
        print("no matplotlib, skipping charts:", e)
        return []
    made = []
    fig, ax = plt.subplots(figsize=(7, 3.6))
    shares = data["tc2"]["shares"]
    names = list(shares)
    ax.bar(names, [v * 100 for v in shares.values()], color=["#4c78a8" if "worker" in n else "#f58518" if "spin" in n else "#bab0ac" for n in names])
    ax.set_ylabel("CPU share (%)")
    ax.set_title("TC2: CPU share per task over the window (round robin, 100 Hz timer)")
    ax.tick_params(axis="x", rotation=45)
    fig.tight_layout()
    p = os.path.join(out, "cpu-share-tc2.png")
    fig.savefig(p, dpi=130)
    plt.close(fig)
    made.append(p)

    fig, ax = plt.subplots(figsize=(6, 3.4))
    cfg = ["2 workers (TC1)", "1 worker (TC3)", "9 busy tasks (TC4)"]
    t1 = data["tc1"]["share"]
    two = max(t1.values()) if t1 else 0
    vals = [two * 100, data["tc3_share"] * 100 if data.get("tc3_share") else 0, data["tc4"]["share_one"] * 100]
    ax.bar(cfg, vals, color="#4c78a8")
    ax.set_ylabel("share of one busy task (%)")
    ax.set_title("Share of one busy task as load changes")
    fig.tight_layout()
    p = os.path.join(out, "share-vs-load.png")
    fig.savefig(p, dpi=130)
    plt.close(fig)
    made.append(p)

    fig, ax = plt.subplots(figsize=(6, 3.4))
    rr = data["tc6"]
    ax.plot(range(1, len(rr) + 1), [r["ratio"] for r in rr], "o-", label="fairness min/max")
    ax.plot(range(1, len(rr) + 1), [r["hz"] / 100 for r in rr], "s--", label="timer rate / 100 Hz")
    ax.set_ylim(0, 1.3)
    ax.set_xlabel("repeat")
    ax.set_xticks(range(1, len(rr) + 1))
    ax.set_title("TC6: repeatability across fresh boots")
    ax.legend(fontsize=8)
    fig.tight_layout()
    p = os.path.join(out, "repeatability-tc6.png")
    fig.savefig(p, dpi=130)
    plt.close(fig)
    made.append(p)

    m0, m1 = data["tc4"]["m0"], data["tc4"]["m1"]
    h0, hm, h1 = data["tc4"]["heap"]
    fig, axs = plt.subplots(1, 2, figsize=(7, 3.2))
    axs[0].bar(["before", "after"], [m0["frames_free"], m1["frames_free"]], color=["#4c78a8", "#54a24b"])
    axs[0].set_title("free frames (TC4 stress)")
    lo = min(m0["frames_free"], m1["frames_free"])
    axs[0].set_ylim(lo * 0.999, max(m0["frames_free"], m1["frames_free"]) * 1.001)
    axs[1].bar(["start", "20 files", "removed"], [h0, hm, h1], color=["#4c78a8", "#f58518", "#54a24b"])
    axs[1].set_title("heap used (KiB)")
    fig.tight_layout()
    p = os.path.join(out, "memory-tc4.png")
    fig.savefig(p, dpi=130)
    plt.close(fig)
    made.append(p)
    return made


def guarded(tc, fn, *args):
    """A crashed scenario is a failed check, not a crashed run."""
    try:
        return fn(*args)
    except Exception as e:  # noqa: BLE001
        check(tc, "scenario ran to the end without an error", False, repr(e))
        return None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--efi", default="target/x86_64-unknown-uefi/release/aeon.efi")
    ap.add_argument("--out", default="docs/ca6/evidence")
    ap.add_argument("--repeats", type=int, default=3)
    ap.add_argument("--window", type=float, default=6.0, help="seconds each CPU-share window lasts")
    a = ap.parse_args()

    class Ctx:
        pass

    ctx = Ctx()
    ctx.efi = os.path.abspath(a.efi)
    ctx.work = tempfile.mkdtemp(prefix="aeon-ca6-")
    out = os.path.abspath(a.out)
    os.makedirs(os.path.join(out, "logs"), exist_ok=True)
    data = {}
    t_start = time.time()
    try:
        print("[M1/M2/M5] boot, console, diagnostics, timer")
        data["boot"] = guarded("M1", tc_boot, ctx, out)
        print("[TC1] minimum configuration: two tasks")
        data["tc1"] = guarded("TC1", tc1, ctx, out, a.window)
        print("[TC2] multiple tasks")
        data["tc2"] = guarded("TC2", tc2, ctx, out, a.window)
        print("[TC3] boundary: one task, refused edge cases")
        guarded("TC3", tc3, ctx, out, a.window, (data["tc1"] or {}).get("share"))
        lone = [m for m in METRICS if m[0] == "TC3" and m[1].startswith("CPU share of the lone")]
        data["tc3_share"] = lone[0][2] if lone and lone[0][2] else 0
        print("[TC4] stress")
        data["tc4"] = guarded("TC4", tc4, ctx, out, a.window)
        print("[TC5] failure")
        guarded("TC5", tc5, ctx, out)
        print("[M6] synchronization")
        guarded("M6", tc_sync, ctx, out)
        print("[TC6] repeatability")
        data["tc6"] = guarded("TC6", tc6, ctx, out, a.window, a.repeats)
    finally:
        shutil.rmtree(ctx.work, ignore_errors=True)

    img = os.path.getsize(ctx.efi)
    metric("size", "kernel image (unsigned build)", img, "bytes")
    charts = make_charts(out, data) if all(data.get(k) for k in ("tc1", "tc2", "tc4", "tc6")) else []
    with open(os.path.join(out, "results.csv"), "w", newline="", encoding="utf-8") as f:
        w = csv.writer(f)
        w.writerow(["kind", "test_case", "name", "value", "unit_or_detail"])
        for tc, what, ok, detail in CHECKS:
            w.writerow(["check", tc, what, "PASS" if ok else "FAIL", detail])
        for tc, name, value, unit in METRICS:
            w.writerow(["metric", tc, name, value, unit])
    with open(os.path.join(out, "results.json"), "w", encoding="utf-8") as f:
        json.dump({
            "checks": [{"tc": c[0], "check": c[1], "pass": c[2], "detail": c[3]} for c in CHECKS],
            "metrics": [{"tc": m[0], "name": m[1], "value": m[2], "unit": m[3]} for m in METRICS],
            "seconds": round(time.time() - t_start, 1),
            "window_s": a.window,
            "repeats": a.repeats,
        }, f, indent=2, default=str)
    passed = sum(1 for c in CHECKS if c[2])
    print(f"\n{passed} passed, {len(CHECKS) - passed} failed, {len(METRICS)} metrics, {len(charts)} charts, {time.time() - t_start:.0f} s")
    sys.exit(0 if passed == len(CHECKS) else 1)


if __name__ == "__main__":
    main()
