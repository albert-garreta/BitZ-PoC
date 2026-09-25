#!/usr/bin/env bash
set -euo pipefail

cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-target/falcon-native}"
export RUSTFLAGS="${RUSTFLAGS:--C target-cpu=native}"

# Hardware-specific benchmark build; the library's ordinary portable build
# remains available. Output records the selected kernel and compiled features.
cargo bench --offline --profile release --features falcon-hybrid \
    --bench falcon_hybrid -- "$@"
