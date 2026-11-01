#!/usr/bin/env bash
# Runs the host unit tests under Miri (undefined-behaviour checker). Needs: rustup +nightly-x86_64-pc-windows-gnu
# component add miri rust-src. The random-input tests are skipped because they are too slow when interpreted.
set -euo pipefail
cd "$(dirname "$0")/.."
MIRIFLAGS="-Zmiri-disable-isolation" cargo +nightly-x86_64-pc-windows-gnu miri test --lib --target x86_64-pc-windows-gnu -- \
    --skip fuzz --skip never_panic --skip million_a --skip every_truncation --skip fixture --skip random_garbage
