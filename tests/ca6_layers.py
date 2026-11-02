"""CA-6 layer evidence: ready queue, scheduler trace, cooperative vs timer switching, finishing jobs, the
shared-counter race and its fix, and the allocator. Called from ca6_evidence.py, which passes itself in as
`E` (so this module shares its Session, check and metric helpers and the lists they fill).

The pipeline is the one the assignment describes: Rust kernel -> scheduler trace -> log file -> Python
parser -> analysis -> graphs -> conclusion. The parser is `parse_trace`; the log files are the raw serial
logs saved next to the results.
"""
import os
import re
import statistics
import time

TRACE_LINE = re.compile(r"^tick\s+(\d+)\s+(\d+) -> (\d+)\s+(\w+)\s*$", re.M)
SUMMARY = re.compile(r"switches (\d+) \(last (\d+) kept\): timer (\d+) yield (\d+) block (\d+) sleep (\d+) exit (\d+)")
JOB_LINE = re.compile(r"job-(\d) finished: (\d+) units; first ran (\d+) ticks after it was created, done after (\d+) ticks")
RACE_LINE = re.compile(r"race (no lock|with lock): expected (\d+), got (\d+), lost (\d+) update\(s\)")
ALLOC_OK = re.compile(r"allocated (\d+) KiB in (\d+) us: heap used (\d+) -> (\d+) KiB of (\d+) KiB")
FREED = re.compile(r"freed: heap used (\d+) KiB, (\d+) free hole")


def parse_trace(text):
    """Turns the kernel's `trace` output into a list of (tick, from_id, to_id, reason) and the summary line."""
    events = [(int(t), int(a), int(b), r) for t, a, b, r in TRACE_LINE.findall(text)]
    m = SUMMARY.search(text)
    summary = None
    if m:
        total, held, timer, yld, block, sleep, exit_ = (int(x) for x in m.groups())
        summary = {"total": total, "held": held, "timer": timer, "yield": yld, "block": block, "sleep": sleep, "exit": exit_}
    return events, summary


def run_lengths(events):
    """How many ticks each stretch of running lasted: from one switch to the next, per task that came in."""
    out = []
    for (t0, _f0, to0, _r0), (t1, _f1, _to1, _r1) in zip(events, events[1:]):
        out.append((to0, t1 - t0))
    return out


def wait_latencies(events, tasks):
    """For each task: ticks between leaving the CPU and getting it back (scheduling latency)."""
    left = {}
    lat = {t: [] for t in tasks}
    for tick, frm, to, _r in events:
        if to in lat and to in left:
            lat[to].append(tick - left.pop(to))
        if frm in lat:
            left[frm] = tick
    return lat


def periodic_fraction(seq, period):
    """Fraction of positions where seq[i] == seq[i + period]: 1.0 means a perfectly repeating order."""
    if len(seq) <= period:
        return 0.0
    same = sum(1 for i in range(len(seq) - period) if seq[i] == seq[i + period])
    return same / (len(seq) - period)


def kill_named(E, s, prefix):
    ps, _ = s.ps()
    for i, (n, st, k) in ps.items():
        if n.startswith(prefix):
            s.run(f"kill {i}")


def layer_queue_and_trace(E, ctx, out, window):
    """Layers 4 and 5: the ready queue, the round-robin order, time slices, the scheduler trace."""
    tag = "L4/L5"
    s = E.Session(ctx, "layers-trace")
    data = {}
    try:
        E.check(tag, "boot", s.booted)
        q0 = s.run("queue")
        E.check(tag, "idle system: the ready queue is empty and matches the task states", "(empty" in q0 and "matches the task states: yes" in q0, q0)
        s.run("spawn")
        s.run("spawn")
        s.run("trace reset")
        time.sleep(0.5)
        q1 = s.run("queue")
        E.check(tag, "with 4 workers the queue lists 3 or 4 of them (one is running, unless a short task has the CPU)", q1.count("worker-") in (3, 4) and "matches the task states: yes" in q1, q1)
        data["queue_text"] = q1.strip()
        # The workers' task ids come from `ps` (names), not from guessing which tasks take many turns.
        ps_now, _ = s.ps()
        worker_ids = sorted(i for i, (n, _st, _k) in ps_now.items() if n.startswith("worker"))
        time.sleep(window)
        txt = s.run("trace all", timeout=120)
        events, summary = parse_trace(txt)
        E.check(tag, "the trace has events and a summary", bool(events) and summary is not None, txt[-200:])
        if not events or not summary:
            return data
        timer_share = summary["timer"] / max(1, summary["total"])
        E.metric(tag, "context switches in the window", summary["total"], "")
        E.metric(tag, "switches per second", round(summary["total"] / max(window, 1), 1), "1/s")
        E.metric(tag, "share of all switches caused by the timer", round(timer_share, 4), "")
        # The short-lived `shell` and `status` tasks wake now and then and give the CPU back by sleeping,
        # which is why not every switch is a timer switch. What matters is why the WORKERS left the CPU.
        workers = worker_ids
        E.metric(tag, "busy workers (task ids from ps)", workers, "task ids")
        E.check(tag, "four workers are running", len(workers) == 4, workers)
        left = [r for _t, f, _to, r in events if f in workers]
        wt = sum(1 for r in left if r == "timer") / max(1, len(left))
        E.metric(tag, "share of the workers' departures caused by the timer", round(wt, 4), "")
        E.check(tag, "a worker that never yields always loses the CPU to the timer (at least 99% of its departures)", wt >= 0.99 and len(left) > 100, f"{wt} of {len(left)}")
        stretches = [ln for to, ln in run_lengths(events) if to in workers]
        one_tick = sum(1 for ln in stretches if ln == 1) / max(1, len(stretches))
        E.metric(tag, "worker stretches that lasted exactly one tick (one 10 ms slice)", round(one_tick, 3), "")
        E.check(tag, "time slice: at least 90% of worker stretches last exactly one tick", one_tick >= 0.9, one_tick)
        seq = [to for _t, _f, to, _r in events if to in workers]
        pf = periodic_fraction(seq, max(1, len(workers)))
        E.metric(tag, "round-robin order repeats (fraction of turns that match one cycle earlier)", round(pf, 3), "")
        E.check(tag, "round-robin order: the workers take turns in the same cycle (at least 90%)", pf >= 0.9, pf)
        lat = wait_latencies(events, set(workers))
        flat = [x for v in lat.values() for x in v]
        if flat:
            E.metric(tag, "scheduling latency of the workers, mean / max", f"{statistics.mean(flat):.2f} / {max(flat)}", "ticks (10 ms each)")
            E.check(tag, "scheduling latency is about (workers - 1) time slices", abs(statistics.mean(flat) - (len(workers) - 1)) <= 1.0, statistics.mean(flat))
        data.update({"events": events, "summary": summary, "distinct": len(workers), "one_tick": one_tick, "pf": pf})
        kill_named(E, s, "worker")
        s.save_log(out)
        # the raw trace is the "log file" the Python parser works on
        with open(os.path.join(out, "logs", "trace-preemptive.txt"), "w", encoding="utf-8") as f:
            f.write(txt)
    finally:
        s.close()
    return data


def layer_coop(E, ctx, out, window, pre):
    """Layer 6: the same measurement with workers that yield by themselves."""
    tag = "L6"
    s = E.Session(ctx, "layers-coop")
    data = {}
    try:
        E.check(tag, "boot", s.booted)
        s.run("spawn coop")
        s.run("trace reset")
        time.sleep(window)
        txt = s.run("trace all", timeout=120)
        events, summary = parse_trace(txt)
        E.check(tag, "the trace has events and a summary", bool(events) and summary is not None, txt[-200:])
        if not events or not summary:
            return data
        yshare = summary["yield"] / max(1, summary["total"])
        E.metric(tag, "context switches in the window (cooperative)", summary["total"], "")
        E.metric(tag, "switches per second (cooperative)", round(summary["total"] / max(window, 1), 1), "1/s")
        E.metric(tag, "share of switches that were `yield`", round(yshare, 4), "")
        E.check(tag, "cooperative workers are switched by `yield` (at least 90% of switches)", yshare >= 0.9, yshare)
        if pre and pre.get("summary"):
            pre_rate = pre["summary"]["total"] / max(window, 1)
            co_rate = summary["total"] / max(window, 1)
            E.metric(tag, "switch-rate ratio cooperative / timer-driven", round(co_rate / max(pre_rate, 1e-9), 1), "x")
            E.check(tag, "cooperative switching is not limited by the 100 Hz tick (switches far more often)", co_rate > 3 * pre_rate, f"{co_rate} vs {pre_rate}")
        data.update({"summary": summary, "events": events})
        kill_named(E, s, "coop")
        s.save_log(out)
        with open(os.path.join(out, "logs", "trace-cooperative.txt"), "w", encoding="utf-8") as f:
            f.write(txt)
    finally:
        s.close()
    return data


def layer_jobs(E, ctx, out):
    """Layer 5 (task completion) and metrics: jobs of 40, 80 and 160 units run together and end."""
    tag = "L5"
    s = E.Session(ctx, "layers-jobs")
    data = {}
    try:
        E.check(tag, "boot", s.booted)
        base = len(s.vm.serial_text())
        # One command starts all three in the same instant, so their turns interleave (typing three
        # separate commands takes longer than a short job runs).
        o = s.run("job 600 300 150")
        E.check(tag, "one command started three jobs", o.count("started job-") == 3, o)
        end = time.time() + 180
        found = []
        while time.time() < end:
            found = JOB_LINE.findall(s.vm.serial_text()[base:].replace("\r", ""))
            if len(found) >= 3:
                break
            time.sleep(0.5)
        E.check(tag, "all three jobs finished and reported", len(found) == 3, found)
        if len(found) == 3:
            order = [int(u) for _slot, u, _a, _b in found]
            E.check(tag, "round robin finishes the shortest job first (order 150, 300, 600)", order == [150, 300, 600], order)
            resp = [int(a) for _s, _u, a, _b in found]
            tat = [int(b) for _s, _u, _a, b in found]
            E.metric(tag, "response time of the three jobs (first run after creation)", resp, "ticks")
            E.metric(tag, "turnaround of the jobs of 150, 300, 600 units", tat, "ticks")
            E.check(tag, "every job first ran within 3 ticks of creation (three jobs share the queue)", max(resp) <= 3, resp)
            # Fluid model of round robin with equal shares: with 3 jobs the shortest needs 3 x 150 = 450
            # unit-times, the next 450 + 2 x 150 = 750, the last 750 + 1 x 300 = 1050. Compare the
            # RATIOS (they do not depend on how long one unit takes on this machine).
            pred = [450, 750, 1050]
            r1, r2 = tat[1] / tat[0], tat[2] / tat[0]
            E.metric(tag, "turnaround ratios 300:150 and 600:150 (measured)", [round(r1, 3), round(r2, 3)], "")
            E.metric(tag, "turnaround ratios predicted by equal-share round robin", [round(pred[1] / pred[0], 3), round(pred[2] / pred[0], 3)], "")
            E.check(tag, "measured turnaround follows equal-share round robin within 15%", abs(r1 / (pred[1] / pred[0]) - 1) <= 0.15 and abs(r2 / (pred[2] / pred[0]) - 1) <= 0.15, f"{r1:.3f} {r2:.3f}")
            total_units = sum(int(u) for _s, u, _a, _b in found)
            makespan = max(tat) / 100.0
            E.metric(tag, "throughput", round(total_units / makespan, 1), "units/s")
            E.metric(tag, "time for one unit of work (from the longest job)", round(max(tat) * 10.0 / 1050 / 1.0, 2), "ms per unit of work")
            data["jobs"] = sorted((int(u), int(b), int(a)) for _s, u, a, b in found)
        ps, _ = s.ps()
        E.check(tag, "finished jobs have left the task table (state terminated, slot freed)", not any(n.startswith("job-") for n, _st, _k in ps.values()), ps)
        q = s.run("queue")
        E.check(tag, "after completion the queue is empty and consistent", "matches the task states: yes" in q, q)
        s.save_log(out)
    finally:
        s.close()
    return data


def layer_race(E, ctx, out):
    """Layer 7: the shared-resource problem, the synchronization, the corrected execution. Twice each."""
    tag = "L7"
    s = E.Session(ctx, "layers-race")
    data = {"nolock": [], "lock": []}
    try:
        E.check(tag, "boot", s.booted)
        for mode, cmd in (("nolock", "race"), ("lock", "race lock"), ("nolock", "race"), ("lock", "race lock")):
            base = len(s.vm.serial_text())
            s.run(cmd)
            end = time.time() + 180
            m = None
            while time.time() < end:
                m = RACE_LINE.search(s.vm.serial_text()[base:].replace("\r", ""))
                if m:
                    break
                time.sleep(0.5)
            E.check(tag, f"`{cmd}` finished and reported", m is not None, s.vm.serial_text()[-200:])
            if m:
                want, got, lost = int(m.group(2)), int(m.group(3)), int(m.group(4))
                data[mode].append((want, got, lost))
                time.sleep(0.5)
        E.metric(tag, "without a lock: (expected, got, lost) per run", data["nolock"], "")
        E.metric(tag, "with the lock: (expected, got, lost) per run", data["lock"], "")
        E.check(tag, "the problem is real: every unprotected run lost updates", bool(data["nolock"]) and all(l > 0 for _w, _g, l in data["nolock"]), data["nolock"])
        E.check(tag, "the fix works: every locked run lost nothing and reached the expected total", bool(data["lock"]) and all(l == 0 and g == w for w, g, l in data["lock"]), data["lock"])
        if data["nolock"]:
            pct = [100.0 * l / w for w, _g, l in data["nolock"]]
            E.metric(tag, "share of additions lost without the lock", [round(p, 1) for p in pct], "%")
        s.save_log(out)
    finally:
        s.close()
    return data


def layer_alloc(E, ctx, out):
    """Layer 8: allocation time by size, memory statistics, and what happens when allocation fails."""
    tag = "L8"
    s = E.Session(ctx, "layers-alloc")
    data = {"sizes": []}
    try:
        E.check(tag, "boot", s.booted)
        mem0 = s.mem()
        for kib in (1, 64, 1024, 8192, 16384):
            o = s.run(f"alloc {kib}")
            m = ALLOC_OK.search(o)
            f = FREED.search(o)
            E.check(tag, f"alloc {kib} KiB succeeds and is freed", m is not None and f is not None, o)
            if m:
                data["sizes"].append((kib, int(m.group(2))))
        E.metric(tag, "allocation time by size (KiB, microseconds)", data["sizes"], "")
        o = s.run("alloc 99999999")
        E.check(tag, "an impossible request is refused cleanly and nothing changes", "refused" in o and "nothing changed" in o, o)
        mem1 = s.mem()
        E.check(tag, "the heap is intact after the refusal (used and holes unchanged)", (mem0["heap_used_kib"], mem0["heap_holes"]) == (mem1["heap_used_kib"], mem1["heap_holes"]), f"{mem0} -> {mem1}")
        o = s.run("alloc 0")
        E.check(tag, "a request of zero is accepted", "allocated 0 KiB" in o, o)
        m = s.run("echo allocator-ok")
        E.check(tag, "the kernel carries on after allocation failure", "allocator-ok" in m, m)
        total_kib = 0
        for line in s.run("mem").splitlines():
            if line.startswith("heap"):
                mm = re.search(r"of (\d+) KiB", line)
                total_kib = int(mm.group(1)) if mm else 0
        E.metric(tag, "heap capacity", total_kib, "KiB")
        E.metric(tag, "heap utilization at rest", f"{mem1['heap_used_kib']} of {total_kib}", "KiB")
        s.save_log(out)
    finally:
        s.close()
    return data


def charts(out, d):
    """Graphs for the layers. Returns the file names written."""
    try:
        import matplotlib
        matplotlib.use("Agg")
        import matplotlib.pyplot as plt
    except Exception as e:  # noqa: BLE001
        print("no matplotlib, skipping layer charts:", e)
        return []
    import os
    made = []
    pre = d.get("trace") or {}
    if pre.get("events"):
        ev = pre["events"][:40]
        ids = sorted({to for _t, _f, to, _r in ev})
        colors = plt.cm.tab10.colors
        fig, ax = plt.subplots(figsize=(8, 3.2))
        for (t0, _f, to, _r), (t1, _f1, _to1, _r1) in zip(ev, ev[1:]):
            ax.broken_barh([(t0, max(t1 - t0, 0.2))], (ids.index(to) - 0.4, 0.8), color=colors[ids.index(to) % 10])
        ax.set_yticks(range(len(ids)))
        ax.set_yticklabels([f"task {i}" for i in ids])
        ax.set_xlabel("timer tick (10 ms each)")
        ax.set_title("Round robin from the scheduler trace: who ran when (first 40 switches)")
        fig.tight_layout()
        p = os.path.join(out, "trace-gantt.png")
        fig.savefig(p, dpi=130)
        plt.close(fig)
        made.append(p)
    co = d.get("coop") or {}
    if pre.get("summary") and co.get("summary"):
        fig, ax = plt.subplots(figsize=(6.5, 3.4))
        labels = ["timer-driven\n(spawn)", "cooperative\n(spawn coop)"]
        reasons = ["timer", "yield", "block", "sleep", "exit"]
        bottom = [0, 0]
        cols = ["#4c78a8", "#f58518", "#54a24b", "#b279a2", "#bab0ac"]
        for r, c in zip(reasons, cols):
            vals = [pre["summary"][r], co["summary"][r]]
            ax.bar(labels, vals, bottom=bottom, label=r, color=c)
            bottom = [b + v for b, v in zip(bottom, vals)]
        ax.set_ylabel("context switches in the window")
        ax.set_title("Why the CPU changed hands")
        ax.legend(fontsize=8)
        fig.tight_layout()
        p = os.path.join(out, "switch-reasons.png")
        fig.savefig(p, dpi=130)
        plt.close(fig)
        made.append(p)
    race = d.get("race") or {}
    if race.get("nolock") and race.get("lock"):
        fig, ax = plt.subplots(figsize=(6, 3.4))
        names = [f"no lock #{i + 1}" for i in range(len(race["nolock"]))] + [f"lock #{i + 1}" for i in range(len(race["lock"]))]
        got = [g for _w, g, _l in race["nolock"]] + [g for _w, g, _l in race["lock"]]
        want = (race["nolock"] + race["lock"])[0][0]
        ax.bar(names, got, color=["#e45756"] * len(race["nolock"]) + ["#54a24b"] * len(race["lock"]))
        ax.axhline(want, color="k", ls="--", lw=1, label=f"expected {want}")
        ax.set_ylabel("final counter value")
        ax.set_title("Shared counter: lost updates, then the fix")
        ax.legend(fontsize=8)
        fig.tight_layout()
        p = os.path.join(out, "race-lock.png")
        fig.savefig(p, dpi=130)
        plt.close(fig)
        made.append(p)
    jobs = d.get("jobs", {}).get("jobs")
    if jobs:
        fig, ax = plt.subplots(figsize=(6, 3.4))
        ax.bar([f"{u} units" for u, _t, _r in jobs], [t * 10 for _u, t, _r in jobs], color="#4c78a8")
        ax.set_ylabel("turnaround (ms)")
        ax.set_title("Three jobs run together: shortest finishes first")
        fig.tight_layout()
        p = os.path.join(out, "jobs-turnaround.png")
        fig.savefig(p, dpi=130)
        plt.close(fig)
        made.append(p)
    al = d.get("alloc", {}).get("sizes")
    if al:
        fig, ax = plt.subplots(figsize=(6, 3.4))
        ax.plot([k for k, _u in al], [u for _k, u in al], "o-")
        ax.set_xscale("log")
        ax.set_xlabel("request size (KiB)")
        ax.set_ylabel("allocation time (us)")
        ax.set_title("Heap allocation time by size")
        fig.tight_layout()
        p = os.path.join(out, "alloc-time.png")
        fig.savefig(p, dpi=130)
        plt.close(fig)
        made.append(p)
    return made
