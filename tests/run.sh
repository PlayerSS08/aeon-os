#!/usr/bin/env bash
# Full test run from Git Bash on Windows: lint, host unit tests, then boot tests in QEMU inside WSL.
# Fault-injection images are built into target/ only, never into usb/ (cargo build does not touch usb/).
set -euo pipefail
cd "$(dirname "$0")/.."

cargo clippy --release -- -D warnings
cargo test --lib --target x86_64-pc-windows-gnu
cargo build --release

for f in fault-ud fault-pf fault-stack fault-wx fault-nx fault-panic; do
    cargo build --release --features "$f" --target-dir "target/fault-build/$f"
    mkdir -p "target/fault-images/$f"
    cp "target/fault-build/$f/x86_64-unknown-uefi/release/aeon.efi" "target/fault-images/$f/aeon.efi"
done

wsl -d Ubuntu -- bash -c "cd '$(wslpath -a . 2>/dev/null || echo /mnt/g/Aeon)' && python3 tests/boot_test.py $*"
