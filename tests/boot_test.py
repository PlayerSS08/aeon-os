#!/usr/bin/env python3
"""Boot tests for Aeon: QEMU + OVMF, optional swtpm and Secure Boot. Run inside WSL/Linux:

    python3 tests/boot_test.py [--efi PATH] [--faults DIR] [--shots DIR] [-k NAME]

Everything temporary lives in a fresh temp directory; nothing is written to the repo except the
screenshots in --shots (default: target/test-shots).
"""
import argparse
import json
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
    def __init__(self, esp_dir, work, code=CODE, vars_file=VARS, tpm=False, name="vm"):
        self.work = work
        self.serial = os.path.join(work, f"{name}.serial")
        self.dcon = os.path.join(work, f"{name}.debugcon")
        self.qmp_path = os.path.join(work, f"{name}.qmp")
        self.vars = os.path.join(work, f"{name}.vars.fd")
        shutil.copy(vars_file, self.vars)
        self.swtpm = None
        args = [
            "qemu-system-x86_64", "-machine", "q35,smm=on", "-cpu", "max", "-m", "256", "-vga", "std", "-display", "none",
            "-global", "driver=cfi.pflash01,property=secure,value=on",
            "-drive", f"if=pflash,format=raw,unit=0,readonly=on,file={code}",
            "-drive", f"if=pflash,format=raw,unit=1,file={self.vars}",
            "-drive", f"format=raw,file=fat:rw:{esp_dir}",
            "-serial", f"file:{self.serial}",
            "-debugcon", f"file:{self.dcon}", "-global", "isa-debugcon.iobase=0xe9",
            "-qmp", f"unix:{self.qmp_path},server,nowait", "-no-reboot",
        ]
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


def make_esp(work, efi, name="esp", extra=None):
    d = os.path.join(work, name)
    os.makedirs(os.path.join(d, "EFI", "BOOT"), exist_ok=True)
    shutil.copy(efi, os.path.join(d, "EFI", "BOOT", "BOOTX64.EFI"))
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
        r.check("seed value never logged", "seed=" not in t)
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
        r.check("menu opens on a key", vm.wait_for("Enter boot | D toggle debug"), vm.text()[-500:])
        time.sleep(0.5)
        w, h, pix = vm.shot(os.path.join(ctx.shots, "4-menu.png"))
        r.check("menu leaves the splash (log visible)", find_bar(pix, w, h) is None)
        lit = sum(1 for yy in range(h // 2, h - 20) for xx in range(0, w, 2) if px(pix, w, xx, yy) != (0, 0, 0))
        r.check("menu text is on screen", lit > 300, f"lit={lit}")
        vm.keys("ret")
        r.check("Enter boots", vm.wait_for("boot complete", 120))
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


TESTS = {
    "boot": t_boot_and_splash, "shell": t_shell, "menu": t_menu, "tpm": t_tpm, "notpm": t_no_tpm,
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
