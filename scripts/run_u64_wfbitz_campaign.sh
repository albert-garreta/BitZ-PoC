#!/usr/bin/env bash
# The u64 multiplication rows of the paper's table through the wfbitz opener
# (branch u64-opt): one launcher campaign per (rate, extension), sizes
# 2^15..2^21, threads 1 and 10, the paper's protocol (5 timed reps after one
# warm-up, every proof verified, peak RSS from a separate child), the same
# `compare` target and flags as scripts/run_mul_matrix_2026_09_20.sh so the
# rows tabulate next to the existing ones (`scripts/mul_table.py` keys them
# `bitz-wf@<rate>`).
#
#   bash scripts/run_u64_wfbitz_campaign.sh            # rates 1/2 and 1/8, 2^15..2^21
#   BITZ_SUITE_STAMP=<label> bash scripts/run_u64_wfbitz_campaign.sh
#
# Rate 1/2 = flock's embedded `fast` ladder (BitZ as shipped, plus the
# crate's Round 0); rate 1/8 = the crate's `custom:3:4` Johnson ladder (with
# Round 0), the same selection the forest rows use at that rate.
# Set CARGO_TARGET_DIR to the tree you built; the launcher builds the bench
# there (with `bitz-parity`, since the experiment names wfbitz). Like the
# matrix script this runs with --no-gate (one idle check up front, below);
# keep the machine idle for the whole run.
set -uo pipefail
cd "$(dirname "$0")/.."
export RUSTFLAGS="${RUSTFLAGS:--C target-cpu=native}"
REPS="${BITZ_SUITE_REPS:-5}"
STAMP="${BITZ_SUITE_STAMP:-$(date +%Y%m%d)}"
OUT="PerfRuns/cs-mul-$STAMP"
mkdir -p PerfRuns
idle=$(top -l 2 -n 0 | grep "CPU usage" | tail -1 | sed -E 's/.* ([0-9.]+)% idle.*/\1/')
echo "cpu idle before the campaign: ${idle}%"

launch() { # label -- rust args
  local label=$1; shift
  local dir="$OUT-$label"
  if [ -d "$dir" ]; then echo "skip $label: $dir exists"; return 0; fi
  echo "=== $label $(date)"
  python3 scripts/run_multiplication_benchmarks.py compare --no-gate --output "$dir" -- \
    proof --reps "$REPS" --warmups 1 --memory rss --skip-unsupported --threads 1,10 "$@" \
    2>&1 | tail -3
}
wf() { launch "u64-wfbitz-r$2${EXT:-}" --workload u64 --backends bitz --opener wfbitz --bitz-profile 100 --ligerito "$3" --log-n "$1"; }

wf 15,17,19 1 fast
wf 15,17,19 3 custom:3:4
EXT=-ext
wf 21 1 fast
wf 21 3 custom:3:4
EXT=
echo "campaign done $(date)"
