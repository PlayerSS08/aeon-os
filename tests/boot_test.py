#!/usr/bin/env python3
"""Boot tests for Aeon: QEMU + OVMF, optional swtpm and Secure Boot. Run inside WSL/Linux:

    python3 tests/boot_test.py [--efi PATH] [--faults DIR] [--shots DIR] [-k NAME]

Everything temporary lives in a fresh temp directory; nothing is written to the repo except the
screenshots in --shots (default: target/test-shots).
"""
import argparse
import json
import re
import os
import shutil
import socket
import struct
import subprocess
import sys
import tempfile
import time
import zlib
import hashlib

OVMF_DIR = "/usr/share/OVMF"
CODE = f"{OVMF_DIR}/OVMF_CODE_4M.fd"
CODE_SB = f"{OVMF_DIR}/OVMF_CODE_4M.secboot.fd"
VARS = f"{OVMF_DIR}/OVMF_VARS_4M.fd"


def ppm_to_png(ppm, png):
    with open(ppm, "rb") as f:
        data = f.read()
    parts = data.split(None, 4)
    w, h = int(parts[1]), int(parts[2])
    pix = data[len(data) - w * h * 3:]
    raw = b"".join(b"\x00" + pix[y * w * 3:(y + 1) * w * 3] for y in range(h))

    def chunk(t, d):
        c = struct.pack(">I", len(d)) + t + d
        return c + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)

    with open(png, "wb") as f:
        f.write(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)))
        f.write(chunk(b"IDAT", zlib.compress(raw, 6)) + chunk(b"IEND", b""))
    return w, h, pix


class Vm:
    def __init__(self, esp_dir, work, code=CODE, vars_file=VARS, tpm=False, name="vm", mem=256):
        self.work = work
        self.serial = os.path.join(work, f"{name}.serial")
        self.dcon = os.path.join(work, f"{name}.debugcon")
        self.qmp_path = os.path.join(work, f"{name}.qmp")
        self.vars = os.path.join(work, f"{name}.vars.fd")
        shutil.copy(vars_file, self.vars)
        self.swtpm = None
        args = [
            "qemu-system-x86_64", "-machine", "q35,smm=on", "-cpu", "max", "-m", str(mem), "-vga", "std", "-display", "none",
            "-drive", f"if=pflash,format=raw,unit=0,readonly=on,file={code}",
            "-drive", f"if=pflash,format=raw,unit=1,file={self.vars}",
            "-drive", f"format=raw,file=fat:rw:{esp_dir}",
            "-serial", f"file:{self.serial}",
            "-debugcon", f"file:{self.dcon}", "-global", "isa-debugcon.iobase=0xe9",
            "-qmp", f"unix:{self.qmp_path},server,nowait", "-no-reboot",
        ]
        if code == CODE_SB:
            # The SMM-enabled firmware build keeps its variable flash writable only from SMM.
            args += ["-global", "driver=cfi.pflash01,property=secure,value=on"]
        if tpm:
            tdir = os.path.join(work, f"{name}.tpm")
            os.makedirs(tdir)
            sock = os.path.join(work, f"{name}.swtpm")
            self.swtpm = subprocess.Popen(
                ["swtpm", "socket", "--tpmstate", f"dir={tdir}", "--ctrl", f"type=unixio,path={sock}", "--tpm2"],
                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            for _ in range(50):
                if os.path.exists(sock):
                    break
                time.sleep(0.1)
            args += ["-chardev", f"socket,id=chrtpm,path={sock}", "-tpmdev", "emulator,id=tpm0,chardev=chrtpm",
                     "-device", "tpm-tis,tpmdev=tpm0"]
        self.proc = subprocess.Popen(args, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        self.q = None
        for _ in range(100):
            try:
                self.q = socket.socket(socket.AF_UNIX)
                self.q.connect(self.qmp_path)
                break
            except OSError:
                self.q.close()
                self.q = None
                time.sleep(0.1)
        assert self.q, "QMP socket never came up: " + self.proc.stderr.read().decode(errors="replace")
        self.qf = self.q.makefile("rw")
        self.qf.readline()
        self.cmd("qmp_capabilities")

    def cmd(self, execute, **arguments):
        self.qf.write(json.dumps({"execute": execute, "arguments": arguments}) + "\n")
        self.qf.flush()
        while True:
            line = json.loads(self.qf.readline())
            if "return" in line or "error" in line:
                return line

    def text(self):
        out = ""
        for p in (self.serial, self.dcon):
            try:
                with open(p, "rb") as f:
                    out += f.read().decode(errors="replace")
            except FileNotFoundError:
                pass
        return out

    def serial_text(self):
        try:
            with open(self.serial, "rb") as f:
                return f.read().decode(errors="replace")
        except FileNotFoundError:
            return ""

    def wait_for(self, needle, timeout=90, source="text"):
        end = time.time() + timeout
        while time.time() < end:
            t = self.serial_text() if source == "serial" else self.text()
            if needle in t:
                return True
            if self.proc.poll() is not None:
                return needle in (self.serial_text() if source == "serial" else self.text())
            time.sleep(0.1)
        return False

    def keys(self, *names):
        for n in names:
            self.cmd("send-key", keys=[{"type": "qcode", "data": n}], **{"hold-time": 60})
            time.sleep(0.15)

    def type(self, s):
        m = {" ": "spc", "-": "minus", "=": "equal"}
        self.keys(*[m.get(c, c) for c in s])

    def shot(self, path_png):
        ppm = path_png[:-4] + ".ppm"
        r = self.cmd("screendump", filename=ppm)
        assert "error" not in r, r
        time.sleep(0.3)
        w, h, pix = ppm_to_png(ppm, path_png)
        os.unlink(ppm)
        return w, h, pix

    def close(self):
        try:
            self.proc.kill()
        except Exception:
            pass
        self.proc.wait()
        if self.swtpm:
            self.swtpm.kill()
            self.swtpm.wait()


def make_esp(work, efi, name="esp", cfg=None):
    d = os.path.join(work, name)
    os.makedirs(os.path.join(d, "EFI", "BOOT"), exist_ok=True)
    shutil.copy(efi, os.path.join(d, "EFI", "BOOT", "BOOTX64.EFI"))
    if cfg is not None:
        with open(os.path.join(d, "EFI", "BOOT", "aeon.cfg"), "w") as f:
            f.write(cfg)
    return d


def px(pix, w, x, y):
    i = (y * w + x) * 3
    return pix[i], pix[i + 1], pix[i + 2]


def find_bar(pix, w, h):
    """Returns (y, x0, x1, filled_x1) of the splash progress bar: a row with a white run next to a track-grey run."""
    for y in range(h):
        row = [px(pix, w, x, y) for x in range(w)]
        white = [x for x, p in enumerate(row) if p == (255, 255, 255)]
        track = [x for x, p in enumerate(row) if p == (38, 44, 58)]
        if len(track) > 50 and len(white) > 20 and max(white) - min(white) + 1 == len(white):
            return y, min(white), max(track), max(white)
    return None


class Results:
    def __init__(self):
        self.ok = []
        self.bad = []

    def check(self, name, cond, detail=""):
        (self.ok if cond else self.bad).append(name)
        print(("  PASS " if cond else "  FAIL ") + name + ("" if cond or not detail else f"\n       {detail}"))
        return cond


# ------------------------------------------------------------------------------------------------ tests

def t_boot_and_splash(ctx):
    """Normal boot: all stages, seed, splash shown during the menu wait, shell reached, splash gone."""
    vm = Vm(make_esp(ctx.work, ctx.efi), ctx.work, name="boot")
    try:
        r = ctx.r
        r.check("menu wait reached", vm.wait_for("press any key within"), vm.text()[-600:])
        time.sleep(0.3)
        w, h, pix = vm.shot(os.path.join(ctx.shots, "1-splash.png"))
        bar = find_bar(pix, w, h)
        r.check("splash shows a progress bar", bar is not None)
        if bar:
            y, x0, x1, fx = bar
            frac = (fx - x0 + 1) / (x1 - x0 + 1)
            r.check("bar is partly filled mid-boot (stage 4 of 8)", 0.25 < frac < 0.75, f"fill={frac:.2f}")
            logo_px = sum(1 for yy in range(0, y) for xx in range(0, w, 2) if px(pix, w, xx, yy) == (255, 255, 255))
            r.check("logo is drawn above the bar", logo_px > 200, f"white px={logo_px}")
        log_rows_lit = sum(1 for yy in range(h - 40, h) for xx in range(0, w, 4) if px(pix, w, xx, yy) != (0, 0, 0))
        r.check("no log text on the splash screen", log_rows_lit == 0)
        r.check("boot completes", vm.wait_for("boot complete"), vm.text()[-800:])
        t = vm.text()
        for st in ["firmware-entry", "early-init", "platform-discovery", "security-init", "exit-boot-services",
                   "kernel-env", "kernel-memory", "kernel-tasks"]:
            r.check(f"stage {st}", f"enter {st}" in t)
        r.check("random seed gathered", "boot seed ready from" in t, "no seed line")
        import re
        r.check("seed value never logged", re.search(r"(?<![a-z])seed=", t) is None)
        r.check("shell prompt", vm.wait_for("aeon> ", 30))
        time.sleep(1.0)
        w, h, pix2 = vm.shot(os.path.join(ctx.shots, "2-shell.png"))
        r.check("splash cleared after boot", find_bar(pix2, w, h) is None)
        status_lit = sum(1 for xx in range(0, w, 2) if px(pix2, w, xx, h - 8) != (0, 0, 0))
        r.check("status bar painted", status_lit > w // 4)
    finally:
        vm.close()


def t_shell(ctx):
    vm = Vm(make_esp(ctx.work, ctx.efi), ctx.work, name="shell")
    try:
        r = ctx.r
        r.check("shell prompt", vm.wait_for("aeon> ", 120, "serial"))
        vm.type("echo hello-aeon")
        vm.keys("ret")
        # the typed line is echoed once and the command prints it once more
        end = time.time() + 20
        while time.time() < end and vm.serial_text().count("hello-aeon") < 2:
            time.sleep(0.2)
        r.check("echo works", vm.serial_text().count("hello-aeon") >= 2, vm.serial_text()[-300:])
        vm.type("sort 3 1 2")
        vm.keys("ret")
        r.check("sort works", vm.wait_for("1 2 3", 20, "serial"))
        vm.type("spawn")
        vm.keys("ret")
        r.check("spawn starts workers", vm.wait_for("started worker-b", 20, "serial"))
        a = None
        vm.type("counters")
        vm.keys("ret")
        time.sleep(1.5)
        vm.type("counters")
        vm.keys("ret")
        time.sleep(1.0)
        nums = [l.split() for l in vm.serial_text().splitlines() if l.strip().replace(" ", "").isdigit() and len(l.split()) == 2]
        r.check("workers preempted (counters advance)", len(nums) >= 2 and int(nums[-1][0]) > int(nums[0][0]) and int(nums[-1][1]) > int(nums[0][1]), str(nums[-3:]))
        vm.shot(os.path.join(ctx.shots, "3-shell-spawn.png"))
    finally:
        vm.close()


def t_menu(ctx):
    vm = Vm(make_esp(ctx.work, ctx.efi), ctx.work, name="menu")
    try:
        r = ctx.r
        r.check("menu wait reached", vm.wait_for("press any key within"))
        vm.keys("spc")
        r.check("menu opens on a key", vm.wait_for("Enter boot | C recovery mode"), vm.text()[-500:])
        time.sleep(0.5)
        w, h, pix = vm.shot(os.path.join(ctx.shots, "4-menu.png"))
        r.check("menu leaves the splash (log visible)", find_bar(pix, w, h) is None)
        lit = sum(1 for yy in range(h // 2, h - 20) for xx in range(0, w, 2) if px(pix, w, xx, yy) != (0, 0, 0))
        r.check("menu text is on screen", lit > 300, f"lit={lit}")
        vm.keys("ret")
        r.check("Enter boots", vm.wait_for("boot complete", 120))
    finally:
        vm.close()
    # Arrow keys move the highlight: Down once is "Recovery mode".
    vm = Vm(make_esp(ctx.work, ctx.efi, "esp-arrows"), ctx.work, name="menu-arrows")
    try:
        r.check("arrows: menu wait reached", vm.wait_for("press any key within"))
        vm.keys("spc")
        r.check("arrows: menu opens", vm.wait_for("Enter boot | C recovery mode"), vm.text()[-400:])
        time.sleep(0.4)
        w, h, pix_a = vm.shot(os.path.join(ctx.shots, "4-menu-a.png"))
        vm.keys("down")
        time.sleep(0.4)
        w, h, pix_b = vm.shot(os.path.join(ctx.shots, "4-menu-b.png"))
        changed = sum(1 for i in range(0, len(pix_a), 3) if pix_a[i:i + 3] != pix_b[i:i + 3])
        r.check("Down moves the highlight (screen changes)", changed > 500, f"changed={changed}")
        vm.keys("ret")
        r.check("Down then Enter picks recovery mode", vm.wait_for("recovery mode selected", 120) and vm.wait_for("boot complete", 150) and "timer and scheduler are off" in vm.text(), vm.text()[-400:])
    finally:
        vm.close()


def t_tpm(ctx):
    vm = Vm(make_esp(ctx.work, ctx.efi), ctx.work, name="tpm", tpm=True)
    try:
        r = ctx.r
        r.check("boot complete with TPM", vm.wait_for("boot complete", 120), vm.text()[-800:])
        t = vm.text()
        r.check("measured cmdline reported verified", "measured cmdline into pcr=12" in t and "verified=true" in t, t[-1500:])
        after = [l.split("pcr12_after=")[1].strip() for l in t.splitlines() if "pcr12_after=" in l]
        before = [l.split("pcr12_before=")[1].strip() for l in t.splitlines() if "pcr12_before=" in l]
        r.check("PCR 12 read back", bool(after) and bool(before))
        if after and before:
            data = b"\x00\x00"  # empty command line: UTF-16LE NUL only
            expect = hashlib.sha256(bytes.fromhex(before[0]) + hashlib.sha256(data).digest()).hexdigest()
            r.check("PCR 12 equals SHA256(before || SHA256(cmdline)) computed independently", after[0] == expect, f"{after[0]} != {expect}")
            r.check("PCR 12 started at zero", before[0] == "0" * 64)
        r.check("event log located", "event log at" in t)
        r.check("event log replay matches the TPM for every PCR it compared", "compared 10 PCRs with the TPM, 0 differ" in t, t[-800:])
        # PCR 11 holds the boot-phase marker measured just before leaving the firmware
        p11a = [l.split("pcr11_after=")[1].strip() for l in t.splitlines() if "pcr11_after=" in l]
        p11b = [l.split("pcr11_before=")[1].strip() for l in t.splitlines() if "pcr11_before=" in l]
        r.check("PCR 11 read back", bool(p11a) and bool(p11b))
        if p11a and p11b:
            want = hashlib.sha256(bytes.fromhex(p11b[0]) + hashlib.sha256(b"aeon:exit-boot-services").digest()).hexdigest()
            r.check("PCR 11 equals SHA256(before || SHA256(phase)) computed independently", p11a[0] == want, f"{p11a[0]} != {want}")
        r.check("phase measured after the command line", t.find("measured cmdline") < t.find("measured phase"))
    finally:
        vm.close()


def pe_sections(path):
    """Name -> raw bytes of each PE section, read straight from the file."""
    with open(path, "rb") as f:
        d = f.read()
    pe = struct.unpack_from("<I", d, 0x3C)[0]
    nsec = struct.unpack_from("<H", d, pe + 6)[0]
    opt = struct.unpack_from("<H", d, pe + 20)[0]
    out = {}
    for i in range(nsec):
        o = pe + 24 + opt + i * 40
        name = d[o:o + 8].rstrip(b"\0").decode()
        vsize, _, rsize, roff = struct.unpack_from("<IIII", d, o + 8)
        out[name] = d[roff:roff + min(vsize, rsize)]
    return out


def t_image(ctx):
    """The image file: SBAT revocation metadata is present, in both the build and the signed release."""
    r = ctx.r
    usb = os.path.abspath(os.path.join(os.path.dirname(ctx.efi), "..", "..", "..", "usb", "EFI", "BOOT", "BOOTX64.EFI"))
    for label, path in (("build", ctx.efi), ("signed release", usb)):
        sec = pe_sections(path)
        sbat = sec.get(".sbat", b"")
        r.check(f"{label}: .sbat section present", ".sbat" in sec, str(list(sec)))
        r.check(f"{label}: .sbat names the SBAT version and the aeon component",
                sbat.startswith(b"sbat,1,SBAT Version,sbat,1,") and b"\naeon,1,Aeon,aeon," in sbat, repr(sbat[:120]))
        r.check(f"{label}: .sbat is NUL-terminated", sbat.endswith(b"\n\0"))
    r.check("image is small (under 200 KiB)", os.path.getsize(ctx.efi) < 200 * 1024, str(os.path.getsize(ctx.efi)))


def t_commands(ctx):
    """New shell commands: version, security, tpm."""
    vm = Vm(make_esp(ctx.work, ctx.efi), ctx.work, name="cmds", tpm=True)
    try:
        r = ctx.r
        r.check("shell prompt", vm.wait_for("aeon> ", 150, "serial"))
        for cmd, expect in (("version", "sbat generation 1"), ("security", "ring 3"), ("tpm", "pcr11 (boot phase)")):
            vm.type(cmd)
            vm.keys("ret")
            r.check(f"`{cmd}` prints", vm.wait_for(expect, 30, "serial"), vm.serial_text()[-400:])
        s = vm.serial_text()
        for needle in ("no-execute (NX)    yes", "write protect      yes", "measured boot      yes", "shadow stack (CET) not enabled", "replayed, matches the TPM", "0 failed boot(s) before this one; recovery mode: no", "0 of 10 PCRs differ"):
            r.check(f"security shows: {needle}", needle in s)
        pcr12 = [l.split()[-1] for l in s.splitlines() if l.startswith("pcr12 (command line)")]
        pcr11 = [l.split()[-1] for l in s.splitlines() if l.startswith("pcr11 (boot phase)")]
        r.check("tpm command shows non-zero PCR values", bool(pcr12) and bool(pcr11) and pcr12[0] != "0" * 64 and pcr11[0] != "0" * 64)
    finally:
        vm.close()


def t_config(ctx):
    """aeon.cfg on the ESP: options are read, merged, shown and measured into PCR 12."""
    r = ctx.r
    vm = Vm(make_esp(ctx.work, ctx.efi, "esp-cfg", cfg="# test config\nverbose\nnomenu\n"), ctx.work, name="cfg", tpm=True)
    try:
        r.check("boots", vm.wait_for("boot complete", 150), vm.text()[-600:])
        t = vm.text()
        r.check("config file was read", "options from aeon.cfg" in t and 'effective options "verbose nomenu"' in t, t[:600])
        r.check("nomenu skips the boot menu", "press any key within" not in t)
        w, h, pix = vm.shot(os.path.join(ctx.shots, "6-config-verbose.png"))
        r.check("verbose: no splash bar, log on screen", find_bar(pix, w, h) is None)
        after = [l.split("pcr12_after=")[1].strip() for l in t.splitlines() if "pcr12_after=" in l]
        data = "verbose nomenu".encode("utf-16-le") + b"\x00\x00"
        want = hashlib.sha256(b"\x00" * 32 + hashlib.sha256(data).digest()).hexdigest()
        r.check("the effective options (load options + config) are what PCR 12 measured", bool(after) and after[0] == want, f"{after} vs {want}")
    finally:
        vm.close()


def t_modes(ctx):
    """mode=WxH picks a firmware screen mode; the splash lays out correctly at each size."""
    r = ctx.r
    for (mw, mh) in ((640, 480), (1024, 768), (1920, 1080)):
        vm = Vm(make_esp(ctx.work, ctx.efi, f"esp-{mw}", cfg=f"mode={mw}x{mh}\n"), ctx.work, name=f"mode{mw}")
        try:
            r.check(f"{mw}x{mh}: menu wait reached", vm.wait_for("press any key within", 150), vm.text()[-400:])
            t = vm.text()
            if f"console=" not in t:
                time.sleep(0.2)
            time.sleep(0.4)
            w, h, pix = vm.shot(os.path.join(ctx.shots, f"7-mode-{mw}x{mh}.png"))
            r.check(f"{mw}x{mh}: screen has the requested size", (w, h) == (mw, mh), f"got {w}x{h}")
            bar = find_bar(pix, w, h)
            r.check(f"{mw}x{mh}: progress bar drawn", bar is not None)
            if bar:
                y, x0, x1, fx = bar
                r.check(f"{mw}x{mh}: bar is centred", abs((x0 + x1) / 2 - w / 2) < 6, f"x0={x0} x1={x1}")
                r.check(f"{mw}x{mh}: bar is inside the screen", 0 < y < h - 40 and x0 > 0 and x1 < w - 1)
                edge = sum(1 for yy in range(h) for xx in list(range(4)) + list(range(w - 4, w)) if px(pix, w, xx, yy) != (0, 0, 0))
                edge += sum(1 for xx in range(w) for yy in list(range(4)) + list(range(h - 4, h)) if px(pix, w, xx, yy) != (0, 0, 0))
                r.check(f"{mw}x{mh}: nothing is drawn in the outer 4 pixel frame", edge == 0, f"{edge} lit pixels")
                lit = [yy for yy in range(h) for xx in range(0, w, 3) if px(pix, w, xx, yy) == (255, 255, 255)]
                r.check(f"{mw}x{mh}: logo block is vertically centred with the bar", lit and abs(((min(lit) + y) / 2) - h / 2) < h * 0.12, f"top={min(lit) if lit else None} bar={y}")
        finally:
            vm.close()


def t_bootcount(ctx):
    """Three failed boots in a row switch the next boot to recovery mode; a good boot clears the counter."""
    r = ctx.r
    bad = os.path.join(ctx.faults, "fault-panic", "aeon.efi")
    vars_file = VARS
    for i in range(1, 4):
        vm = Vm(make_esp(ctx.work, bad, f"esp-bad{i}"), ctx.work, vars_file=vars_file, name=f"bad{i}")
        try:
            r.check(f"failing boot {i} reaches its panic", vm.wait_for("deliberate panic", 150), vm.text()[-300:])
            r.check(f"failing boot {i} counts {i - 1} earlier failure(s)", f"failed_boots={i - 1} " in vm.text())
        finally:
            vm.close()
        vars_file = vm.vars
    # Same NVRAM, a good image: must enter recovery.
    vm = Vm(make_esp(ctx.work, ctx.efi, "esp-rec"), ctx.work, vars_file=vars_file, tpm=True, name="rec")
    try:
        r.check("recovery boot completes", vm.wait_for("boot complete", 180), vm.text()[-600:])
        t = vm.text()
        r.check("3 failures trigger recovery", "failed_boots=3 counting=true recovery=true" in t)
        r.check("recovery banner logged", "RECOVERY MODE" in t)
        r.check("no timer or scheduler in recovery", "timer and scheduler are off" in t and "pit irq0 ok" not in t and "task shell started" not in t)
        r.check("menu waits in recovery", "press any key within 10 s" in t)
        r.check("a different phase marker is measured", "measured phase" in t)
        p11a = [l.split("pcr11_after=")[1].strip() for l in t.splitlines() if "pcr11_after=" in l]
        normal = hashlib.sha256(b"\x00" * 32 + hashlib.sha256(b"aeon:exit-boot-services").digest()).hexdigest()
        rec = hashlib.sha256(b"\x00" * 32 + hashlib.sha256(b"aeon:recovery").digest()).hexdigest()
        r.check("PCR 11 holds the recovery marker, not the normal one", bool(p11a) and p11a[0] == rec and p11a[0] != normal, str(p11a))
        r.check("shell still works in recovery", vm.wait_for("aeon> ", 60, "serial"))
        r.check("successful recovery boot clears the counter", "boot succeeded, counter cleared" in vm.text())
        w, h, pix = vm.shot(os.path.join(ctx.shots, "8-recovery.png"))
        r.check("recovery shows the log, not the splash", find_bar(pix, w, h) is None)
    finally:
        vm.close()
    vars_file = vm.vars
    vm = Vm(make_esp(ctx.work, ctx.efi, "esp-after"), ctx.work, vars_file=vars_file, name="after")
    try:
        r.check("next boot is normal again", vm.wait_for("boot complete", 150) and "failed_boots=0 counting=true recovery=false" in vm.text(), vm.text()[:500])
    finally:
        vm.close()
    # nocount ignores the counter entirely
    vars_file = vm.vars
    vm0 = Vm(make_esp(ctx.work, bad, "esp-bad-again"), ctx.work, vars_file=vars_file, name="bad-again")
    vm0.wait_for("deliberate panic", 150)
    vm0.close()
    vm1 = Vm(make_esp(ctx.work, bad, "esp-bad-again2"), ctx.work, vars_file=vm0.vars, name="bad-again2")
    vm1.wait_for("deliberate panic", 150)
    vm1.close()
    vm2 = Vm(make_esp(ctx.work, bad, "esp-bad-again3"), ctx.work, vars_file=vm1.vars, name="bad-again3")
    vm2.wait_for("deliberate panic", 150)
    vm2.close()
    vm = Vm(make_esp(ctx.work, ctx.efi, "esp-nocount", cfg="nocount\n"), ctx.work, vars_file=vm2.vars, name="nocount")
    try:
        t_ok = vm.wait_for("boot complete", 150)
        r.check("nocount: no recovery and no counter writes", t_ok and "counting=false recovery=false" in vm.text(), vm.text()[:500])
    finally:
        vm.close()


def t_fwshell(ctx):
    """Menu key S: a shell that runs before the kernel, through the firmware's input (USB keyboards work)."""
    r = ctx.r
    vm = Vm(make_esp(ctx.work, ctx.efi), ctx.work, name="fwsh", tpm=True)
    try:
        r.check("menu wait reached", vm.wait_for("press any key within"))
        vm.keys("spc")
        r.check("menu offers the firmware shell", vm.wait_for("S firmware shell"), vm.text()[-500:])
        vm.keys("s")
        r.check("firmware shell prompt", vm.wait_for("fw> ", 30, "serial"), vm.serial_text()[-300:])
        for cmd, expect in (("version", "sbat generation 1"), ("tpm", "pcr12 (command line)"), ("security", "secure boot keys")):
            vm.type(cmd)
            vm.keys("ret")
            r.check(f"`{cmd}` works before the kernel", vm.wait_for(expect, 30, "serial"), vm.serial_text()[-300:])
        vm.type("mem")
        vm.keys("ret")
        r.check("kernel-only commands are refused politely", vm.wait_for("not available before the kernel starts", 30, "serial"))
        vm.type("boot")
        vm.keys("ret")
        r.check("`boot` continues into the kernel", vm.wait_for("boot complete", 150))
        r.check("boot after the shell is a normal boot", "recovery=false" in vm.text())
    finally:
        vm.close()
    vm = Vm(make_esp(ctx.work, ctx.efi, "esp-c"), ctx.work, name="menu-c")
    try:
        vm.wait_for("press any key within")
        vm.keys("spc")
        vm.wait_for("C recovery mode")
        vm.keys("c")
        r.check("menu key C boots recovery mode", vm.wait_for("boot complete", 150) and "timer and scheduler are off" in vm.text(), vm.text()[-400:])
        r.check("recovery shell prompt", vm.wait_for("aeon> ", 60, "serial"))
        run_cmds(vm, r, [("run hello", "user programs are not available"), ("run hello", "user programs are not available")])
    finally:
        vm.close()


def t_lastpanic(ctx):
    """A panic or CPU exception leaves a record; the next boot shows it once."""
    r = ctx.r
    for name, expect in (("fault-panic", "deliberate panic (fault-panic feature)"), ("fault-ud", "CPU exception vector=6")):
        bad = os.path.join(ctx.faults, name, "aeon.efi")
        vm = Vm(make_esp(ctx.work, bad, f"esp-{name}"), ctx.work, name=f"lp-{name}")
        vm.wait_for(expect, 150)
        time.sleep(1)
        vm.close()
        vm2 = Vm(make_esp(ctx.work, ctx.efi, f"esp-ok-{name}"), ctx.work, vars_file=vm.vars, name=f"lp-ok-{name}")
        try:
            r.check(f"{name}: next boot reports the record", vm2.wait_for("previous boot ended in:", 150) and expect in vm2.text(), vm2.text()[:700])
            r.check(f"{name}: record has no addresses", "0x" not in [l for l in vm2.text().splitlines() if "previous boot ended in" in l][0])
            vm2.wait_for("aeon> ", 150, "serial")
            vm2.type("lastpanic")
            vm2.keys("ret")
            r.check(f"{name}: `lastpanic` shows it", vm2.wait_for("the previous boot ended in:", 30, "serial"))
        finally:
            vm2.close()
        vm3 = Vm(make_esp(ctx.work, ctx.efi, f"esp-ok2-{name}"), ctx.work, vars_file=vm2.vars, name=f"lp-ok2-{name}")
        try:
            r.check(f"{name}: the record is shown only once", vm3.wait_for("boot complete", 150) and "previous boot ended in" not in vm3.text())
        finally:
            vm3.close()


def t_seed_history(ctx):
    """The seed is carried to the next boot (derived, never raw) and the next boot reports using it."""
    r = ctx.r
    vm = Vm(make_esp(ctx.work, ctx.efi), ctx.work, name="seed1")
    try:
        r.check("first boot: no earlier seed", vm.wait_for("boot complete", 150) and "mixed in: false" in vm.text())
    finally:
        vm.close()
    vm2 = Vm(make_esp(ctx.work, ctx.efi, "esp2"), ctx.work, vars_file=vm.vars, name="seed2")
    try:
        r.check("second boot: previous seed mixed in", vm2.wait_for("boot complete", 150) and "mixed in: true" in vm2.text())
        r.check("seed bytes never logged", "AeonSeed" not in vm2.text())
        r.check("stored seed is invisible once the kernel runs", vm2.wait_for("stored seed hidden from runtime access: true", 150), vm2.text()[-600:])
    finally:
        vm2.close()


def t_loader_iface(ctx):
    """Boot Loader Interface variables are published and read back; Secure Boot detail is reported."""
    r = ctx.r
    vm = Vm(make_esp(ctx.work, ctx.efi), ctx.work, name="loader")
    try:
        r.check("5 loader variables published", vm.wait_for("Boot Loader Interface: 5 of 5 variables published", 150), vm.text()[:800])
        vm.wait_for("aeon> ", 150, "serial")
        vm.type("security")
        vm.keys("ret")
        r.check("security shows key detail", vm.wait_for("secure boot keys   platform key enrolled: no", 30, "serial"), vm.serial_text()[-900:])
    finally:
        vm.close()
    key, crt, vars_out = sb_vars(ctx.work, "ldr")
    signed = os.path.join(ctx.work, "ldr-signed.efi")
    subprocess.run(["sbsign", "--key", key, "--cert", crt, "--output", signed, ctx.efi], check=True, capture_output=True)
    vm = Vm(make_esp(ctx.work, signed, "esp-ldr-sb"), ctx.work, code=CODE_SB, vars_file=vars_out, name="loader-sb")
    try:
        vm.wait_for("aeon> ", 150, "serial")
        vm.type("security")
        vm.keys("ret")
        r.check("Secure Boot on: platform key enrolled is reported", vm.wait_for("platform key enrolled: yes", 30, "serial"), vm.serial_text()[-900:])
    finally:
        vm.close()


def t_no_tpm(ctx):
    vm = Vm(make_esp(ctx.work, ctx.efi), ctx.work, name="notpm")
    try:
        ctx.r.check("boots without a TPM and says so", vm.wait_for("measured boot unavailable", 90) and vm.wait_for("boot complete", 90))
    finally:
        vm.close()


def sb_vars(work, name):
    """Creates a throwaway key and OVMF vars with that cert enrolled as PK/KEK/db and Secure Boot on."""
    key, crt = os.path.join(work, f"{name}.key"), os.path.join(work, f"{name}.crt")
    subprocess.run(["openssl", "req", "-new", "-x509", "-newkey", "rsa:2048", "-nodes", "-keyout", key, "-out", crt,
                    "-days", "2", "-subj", "/CN=Aeon test key", "-sha256"], check=True, capture_output=True)
    vars_out = os.path.join(work, f"{name}.sbvars.fd")
    guid = "11111111-2222-3333-4444-555555555555"
    subprocess.run(["virt-fw-vars", "--input", VARS, "--output", vars_out, "--set-pk", guid, crt, "--add-kek", guid, crt,
                    "--add-db", guid, crt, "--secure-boot"], check=True, capture_output=True)
    return key, crt, vars_out


def t_secure_boot(ctx):
    r = ctx.r
    key, crt, vars_out = sb_vars(ctx.work, "sb")
    signed = os.path.join(ctx.work, "signed.efi")
    subprocess.run(["sbsign", "--key", key, "--cert", crt, "--output", signed, ctx.efi], check=True, capture_output=True)
    vm = Vm(make_esp(ctx.work, signed, "esp-signed"), ctx.work, code=CODE_SB, vars_file=vars_out, name="sb-ok")
    try:
        r.check("signed image boots under Secure Boot", vm.wait_for("boot complete", 120), vm.text()[-800:])
        r.check("Aeon reports secure_boot=enabled", "secure_boot=enabled" in vm.text())
    finally:
        vm.close()
    vm = Vm(make_esp(ctx.work, ctx.efi, "esp-unsigned"), ctx.work, code=CODE_SB, vars_file=vars_out, name="sb-bad")
    try:
        time.sleep(25)
        t = vm.text()
        r.check("unsigned image is refused under Secure Boot", "Aeon " not in t and "enter firmware-entry" not in t, t[-300:])
    finally:
        vm.close()
    # a signature from a different key must be refused as well
    key2, crt2, _ = sb_vars(ctx.work, "other")
    wrong = os.path.join(ctx.work, "wrong.efi")
    subprocess.run(["sbsign", "--key", key2, "--cert", crt2, "--output", wrong, ctx.efi], check=True, capture_output=True)
    vm = Vm(make_esp(ctx.work, wrong, "esp-wrong"), ctx.work, code=CODE_SB, vars_file=vars_out, name="sb-wrong")
    try:
        time.sleep(25)
        r.check("image signed by an unknown key is refused", "enter firmware-entry" not in vm.text())
    finally:
        vm.close()


def t_release(ctx):
    """The shipped usb/ folder: checksums match, and the signed image boots after enrolling aeon-secureboot.cer."""
    r = ctx.r
    usb = os.path.abspath(os.path.join(os.path.dirname(ctx.efi), "..", "..", "..", "usb"))
    r.check("usb/ folder exists", os.path.isdir(usb), usb)
    if not os.path.isdir(usb):
        return
    ok = True
    for line in open(os.path.join(usb, "SHA256.txt")):
        digest, name = line.split()
        with open(os.path.join(usb, name), "rb") as f:
            ok &= hashlib.sha256(f.read()).hexdigest() == digest
    r.check("SHA256.txt matches the shipped files", ok)
    r.check("usb/ holds only the release files", sorted(os.listdir(usb)) == ["EFI", "SHA256.txt", "aeon-secureboot.cer"], str(os.listdir(usb)))
    # Throwaway PK/KEK; the db gets only the certificate users would enroll.
    pk_key, pk_crt, _ = sb_vars(ctx.work, "relpk")
    pem = os.path.join(ctx.work, "release.pem")
    subprocess.run(["openssl", "x509", "-inform", "DER", "-in", os.path.join(usb, "aeon-secureboot.cer"), "-out", pem], check=True)
    guid = "11111111-2222-3333-4444-555555555555"
    vars_out = os.path.join(ctx.work, "release-enrolled.fd")
    subprocess.run(["virt-fw-vars", "--input", VARS, "--output", vars_out, "--set-pk", guid, pk_crt, "--add-kek", guid, pk_crt,
                    "--add-db", guid, pem, "--secure-boot"], check=True, capture_output=True)
    esp = shutil.copytree(usb, os.path.join(ctx.work, "usbcopy"))
    vm = Vm(esp, ctx.work,
            code=CODE_SB, vars_file=vars_out, name="release")
    try:
        r.check("shipped image boots under Secure Boot", vm.wait_for("boot complete", 120), vm.text()[-600:])
        r.check("secure_boot=enabled", "secure_boot=enabled" in vm.text())
    finally:
        vm.close()


FAULTS = {
    "fault-ud": "EXCEPTION vector=6",
    "fault-pf": "EXCEPTION vector=14",
    "fault-stack": "EXCEPTION vector=8",
    "fault-wx": "EXCEPTION vector=14",
    "fault-nx": "EXCEPTION vector=14",
    "fault-panic": "deliberate panic (fault-panic feature)",
}


def t_faults(ctx):
    r = ctx.r
    for name, needle in FAULTS.items():
        efi = os.path.join(ctx.faults, name, "aeon.efi")
        if not os.path.exists(efi):
            r.check(f"{name} image present", False, efi)
            continue
        vm = Vm(make_esp(ctx.work, efi, f"esp-{name}"), ctx.work, name=name)
        try:
            ok = vm.wait_for(needle, 120)
            r.check(f"{name} is caught: {needle}", ok, vm.text()[-600:])
            if ok:
                time.sleep(1.0)
                w, h, pix = vm.shot(os.path.join(ctx.shots, f"5-{name}.png"))
                red = sum(1 for yy in range(0, h, 4) for xx in range(0, w, 4) if px(pix, w, xx, yy) == (28, 8, 10))
                r.check(f"{name} shows the stop screen", red > (w * h) // 32, f"bg px={red}")
                scr = vm.text()
                r.check(f"{name}: halted, no further boot", "boot complete" not in scr.split(needle)[-1])
        finally:
            vm.close()


def run_cmds(vm, r, steps):
    """Type each command, wait for its expected output (searched after the command's echo)."""
    for cmd, expect in steps:
        before = len(vm.serial_text())
        vm.type(cmd)
        vm.keys("ret")
        end = time.time() + 30
        ok = False
        while time.time() < end:
            if expect in vm.serial_text()[before:]:
                ok = True
                break
            time.sleep(0.2)
        r.check(f"`{cmd}` -> {expect!r}", ok, vm.serial_text()[before:][-300:])


def t_fs(ctx):
    """RAM filesystem: write, cat, ls, df, rm, bad names."""
    vm = Vm(make_esp(ctx.work, ctx.efi), ctx.work, name="fs")
    try:
        r = ctx.r
        r.check("shell prompt", vm.wait_for("aeon> ", 150, "serial"))
        run_cmds(vm, r, [
            ("ls", "(no files)"),
            ("write note hello world", "wrote 11 byte(s) to note"),
            ("cat note", "hello world"),
            ("ls", "11  note"),
            ("df", "1 file(s), used 11 of 1048576 bytes"),
            ("write " + "x" * 40 + " a", "bad name"),
            ("cat nothere", "no such file 'nothere'"),
            ("rm note", "removed note"),
            ("cat note", "no such file 'note'"),
            ("df", "0 file(s), used 0 of"),
        ])
    finally:
        vm.close()


def t_cpu(ctx):
    """CPU setup: the MADT is parsed and `cpu` reports it."""
    vm = Vm(make_esp(ctx.work, ctx.efi), ctx.work, name="cpu")
    try:
        r = ctx.r
        r.check("shell prompt", vm.wait_for("aeon> ", 150, "serial"))
        run_cmds(vm, r, [
            ("cpu", "cpus        1 usable (ACPI MADT)"),
            ("cpu", "local APIC  0xfee00000"),
            ("cpu", "I/O APIC    0xfec00000"),
            ("cpu", "APIC flag   yes"),
            ("cpu", "running on  CPU 0 only"),
            ("help", "cpu                CPUs"),
        ])
    finally:
        vm.close()


def _counters(vm):
    """Sends `counters` and returns the two numbers it prints."""
    before = len(vm.serial_text())
    vm.type("counters")
    vm.keys("ret")
    end = time.time() + 10
    while time.time() < end:
        m = re.search(r"^(\d+) (\d+)\s*$", vm.serial_text()[before:], re.M)
        if m:
            return int(m.group(1)), int(m.group(2))
        time.sleep(0.2)
    return None


def _timer_case(ctx, name, cfg, want_timer, want_log):
    vm = Vm(make_esp(ctx.work, ctx.efi, "esp-" + name, cfg=cfg), ctx.work, name=name)
    try:
        r = ctx.r
        r.check(f"{name}: shell prompt", vm.wait_for("aeon> ", 150, "serial"))
        r.check(f"{name}: boot log says {want_log!r}", want_log in vm.text(), vm.text()[-400:])
        run_cmds(vm, r, [("info", f"timer      {want_timer}")])
        run_cmds(vm, r, [("spawn", "aeon> ")])
        a = _counters(vm)
        time.sleep(2)
        b = _counters(vm)
        r.check(f"{name}: preemption works (counters advance)", a is not None and b is not None and b[0] > a[0] and b[1] > a[1], f"{a} -> {b}")
    finally:
        vm.close()


def t_apic(ctx):
    """Timer: the local APIC drives the tick by default; `nolapic` keeps the PIT. Both preempt."""
    _timer_case(ctx, "lapic", None, "lapic", "lapic timer ok")
    _timer_case(ctx, "nolapic", "nolapic\n", "pit", "pit irq0 ok")


def t_user(ctx):
    """Ring 3: hello, faulting and bad programs are contained, a spinning program is preempted and killable."""
    vm = Vm(make_esp(ctx.work, ctx.efi), ctx.work, name="user")
    try:
        r = ctx.r
        r.check("shell prompt", vm.wait_for("aeon> ", 150, "serial"))
        run_cmds(vm, r, [
            ("security", "ring 3             yes"),
            ("run hello", "hello from ring 3"),
            ("ps", "aeon> "),
            ("run fault", "[user] task killed: #PF"),
            ("uptime", "up 00:"),
            ("run gp", "[user] task killed: #GP"),
            ("run badcall", "exited with code 0"),
            ("run nosuch", "unknown program"),
        ])
        run_cmds(vm, r, [("run spin", "started spin as task")])
        s = vm.serial_text()
        m = re.search(r"started spin as task (\d+)", s)
        r.check("spin task id reported", bool(m))
        run_cmds(vm, r, [("uptime", "up 00:"), ("run hello", "already running")])
        if m:
            run_cmds(vm, r, [("kill " + m.group(1), "stopped")])
            time.sleep(1.0)
            before = len(vm.serial_text())
            run_cmds(vm, r, [("run hello", "hello from ring 3")])
        r.check("no kernel panic or exception", "EXCEPTION" not in vm.serial_text() and "panic" not in vm.serial_text().lower().replace("lastpanic", ""))
    finally:
        vm.close()


TESTS = {
    "image": t_image, "boot": t_boot_and_splash, "shell": t_shell, "commands": t_commands, "fs": t_fs, "cpu": t_cpu, "apic": t_apic, "user": t_user, "config": t_config, "modes": t_modes,
    "bootcount": t_bootcount, "fwshell": t_fwshell, "lastpanic": t_lastpanic, "seed": t_seed_history, "loader": t_loader_iface, "menu": t_menu, "tpm": t_tpm, "notpm": t_no_tpm,
    "secureboot": t_secure_boot, "release": t_release, "faults": t_faults,
}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--efi", default="target/x86_64-unknown-uefi/release/aeon.efi")
    ap.add_argument("--faults", default="target/fault-images")
    ap.add_argument("--shots", default="target/test-shots")
    ap.add_argument("-k", action="append", help="run only these tests: " + ", ".join(TESTS))
    a = ap.parse_args()

    class Ctx:
        pass

    ctx = Ctx()
    ctx.efi, ctx.faults, ctx.shots = os.path.abspath(a.efi), os.path.abspath(a.faults), os.path.abspath(a.shots)
    os.makedirs(ctx.shots, exist_ok=True)
    ctx.r = Results()
    ctx.work = tempfile.mkdtemp(prefix="aeon-test-")
    try:
        for name, fn in TESTS.items():
            if a.k and name not in a.k:
                continue
            print(f"[{name}] {fn.__doc__ or ''}".rstrip())
            try:
                fn(ctx)
            except Exception as e:  # a crashed test is a failed test, not a crashed run
                ctx.r.check(f"{name} ran without error", False, repr(e))
    finally:
        shutil.rmtree(ctx.work, ignore_errors=True)
    print(f"\n{len(ctx.r.ok)} passed, {len(ctx.r.bad)} failed")
    sys.exit(1 if ctx.r.bad else 0)


if __name__ == "__main__":
    main()
