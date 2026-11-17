#!/usr/bin/env bash
# Rebuilds the Aeon user programs (user/) and repacks them for the kernel (user/initfs.bin).
# Needs the x86_64-unknown-none target: rustup target add x86_64-unknown-none
set -euo pipefail
cd "$(dirname "$0")/../user"
cargo build --release
mkdir -p bin
for p in hello echo cat wc ls forktest; do
    cp "target/x86_64-unknown-none/release/$p" "bin/$p"
done
python3 ../tools/pack_initfs.py 2>/dev/null || python ../tools/pack_initfs.py
