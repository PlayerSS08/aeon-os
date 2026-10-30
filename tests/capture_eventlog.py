#!/usr/bin/env python3
"""Captures a real TCG event log (OVMF + swtpm) from an image built with --features dump-eventlog and
writes tests/fixtures/ovmf-swtpm-eventlog.{bin,json}: the raw log, where the last record starts, and
the PCR values the TPM reported. The Rust unit tests replay the fixture and must reproduce them.

    cargo build --release --features dump-eventlog --target-dir target/dump-build
    python3 tests/capture_eventlog.py target/dump-build/x86_64-unknown-uefi/release/aeon.efi
"""
import json
import os
import re
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import boot_test as b  # noqa: E402

efi = os.path.abspath(sys.argv[1])
out = os.path.join(os.path.dirname(os.path.abspath(__file__)), "fixtures")
work = tempfile.mkdtemp(prefix="aeon-cap-")
vm = b.Vm(b.make_esp(work, efi, "esp"), work, name="cap", tpm=True)
assert vm.wait_for("replayed", 180), "no replay line"
time.sleep(1.5)
text = vm.text()
vm.close()
lines = text.splitlines()
chunks = {}
for l in lines:
    m = re.search(r"evlog dump hex (\d+) ([0-9a-f]+)$", l)
    if m:
        chunks[int(m.group(1))] = m.group(2)
raw = bytes.fromhex("".join(chunks[i] for i in sorted(chunks)))
pcrs = {}
for l in lines:
    m = re.search(r"evlog dump pcr(\d+)=([0-9a-f]{64})", l)
    if m:
        pcrs[int(m.group(1))] = m.group(2)
last = int(re.search(r"dump last_offset=(\d+) len=(\d+)", text).group(1))
n = int(re.search(r"dump last_offset=(\d+) len=(\d+)", text).group(2))
assert len(raw) == n, (len(raw), n)
os.makedirs(out, exist_ok=True)
with open(os.path.join(out, "ovmf-swtpm-eventlog.bin"), "wb") as f:
    f.write(raw)
with open(os.path.join(out, "ovmf-swtpm-eventlog.json"), "w") as f:
    json.dump({"last_offset": last, "pcrs": {str(k): v for k, v in sorted(pcrs.items())}}, f, indent=1)
print(f"wrote {len(raw)} bytes, last record at {last}, {len(pcrs)} PCRs")
