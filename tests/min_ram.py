#!/usr/bin/env python3
"""Finds the smallest RAM size (in MB) Aeon boots to the shell prompt in, under QEMU + OVMF.
Run inside WSL from the repo root: python3 tests/min_ram.py"""
import os, shutil, sys, tempfile
sys.path.insert(0, os.path.dirname(__file__))
import boot_test as bt

efi = os.path.abspath("target/x86_64-unknown-uefi/release/aeon.efi")
work = tempfile.mkdtemp(prefix="aeon-ram-")
try:
    for mb in (24, 32, 48, 64, 96, 128, 192, 256):
        vm = bt.Vm(bt.make_esp(work, efi, f"esp-{mb}"), work, name=f"ram{mb}", mem=mb)
        try:
            ok = vm.wait_for("aeon> ", 120, "serial")
            t = vm.text()
            free = [l for l in t.splitlines() if "heap:" in l][:1]
            print(f"{mb:4d} MB  {'BOOTS' if ok else 'no shell'}  {free[0].strip()[:60] if free else ''}", flush=True)
        finally:
            vm.close()
finally:
    shutil.rmtree(work, ignore_errors=True)
