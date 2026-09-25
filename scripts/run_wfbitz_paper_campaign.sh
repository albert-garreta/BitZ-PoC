#!/bin/zsh
# The paper's BitZ benches that have a wfbitz path, run with the worldfnd/BitZ
# scheme (`crate::wfbitz`, the paper's protocol otherwise): the native
# multiplication tables (u32-mod32, u64, u128 at 2^15..2^21, rates 1/2 and
# 1/8, threads 1 and 10, plus the width/throughput tables' extra sizes), the
# raw-performance sweep (n = 22..30, 8 threads; wfbitz cannot go below
# m = 22) and the fields-witch shapes (m = 23, 25, 27, 29, W = 1, both rates,
# threads 1 and 10). The multiplication rows go through the opener (Round 0
# included); the raw rows use `examples/wfbitz_bench` on the paper's ladder
# WITHOUT Round 0 (about 2 % of prover time, a few bytes of proof).
# Outputs: PerfRuns/cs-mul-<stamp>-* (mul-bench/v2 campaign dirs, the table
# generators read them with the scheme key `bitz-wf@<rate>`) and
# PerfRuns/wfbitz-raw-<stamp>/*.txt (RESULT lines of wfbitz_bench).
set -uo pipefail
cd "$(dirname "$0")/.."
export RUSTFLAGS="${RUSTFLAGS:--C target-cpu=native}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$PWD/target}"
REPS="${BITZ_SUITE_REPS:-5}"
STAMP="${BITZ_SUITE_STAMP:-$(date +%Y%m%d)-wfbitz}"
OUT="PerfRuns/cs-mul-$STAMP"
RAW="PerfRuns/wfbitz-raw-$STAMP"
B="$CARGO_TARGET_DIR/release/examples/wfbitz_bench"
mkdir -p PerfRuns "$RAW"
[ -x "$B" ] || { echo "missing $B (build: cargo build --release --features bitz-parity --example wfbitz_bench)"; exit 1; }
idle=$(top -l 2 -n 0 | grep "CPU usage" | tail -1 | sed -E 's/.* ([0-9.]+)% idle.*/\1/')
echo "campaign start $(date) | cpu idle ${idle}% | tree $(git rev-parse --short HEAD)$(git status --porcelain | grep -q . && echo -dirty)"
launch() { # label -- rust args
  local label=$1; shift
  local dir="$OUT-$label"
  if [ -d "$dir" ]; then echo "skip $label: $dir exists"; return 0; fi
  echo "=== $label $(date '+%H:%M:%S')"
  python3 scripts/run_multiplication_benchmarks.py compare --no-gate --output "$dir" -- \
    proof --reps "$REPS" --warmups 1 --memory rss --skip-unsupported --threads 1,10 "$@" \
    2>&1 | tail -3
  sleep 20
}
wf() { # width rate-tag ladder log-n [suffix]
  launch "$1-wfbitz-r$2${5:-}" --workload "$1" --backends bitz --opener wfbitz --bitz-profile 100 --ligerito "$3" --log-n "$4"
}
# The native-multiplication tables.
for w in u32-mod32 u64 u128; do
  wf $w 1 custom:1:4 15,17,19
  wf $w 3 custom:3:4 15,17,19
done
for w in u32-mod32 u64; do
  wf $w 1 custom:1:4 21 -ext
  wf $w 3 custom:3:4 21 -ext
done
# The width/throughput tables' extra sizes (the rate-1/8 ceilings).
wf u32-mod32 1 custom:1:4 22 -ext22
wf u32-mod32 3 custom:3:4 22 -ext22
wf u32-mod32 1 custom:1:4 23 -ext23
wf u32-mod32 3 custom:3:4 23 -ext23
wf u64 1 custom:1:4 22 -ext22
wf u64 3 custom:3:4 22 -ext22
# The raw-performance sweep, the paper's 8 threads.
for n in 22 23 24 25 26 27 28 29 30; do
  f="$RAW/raw_n${n}_t8.txt"
  [ -s "$f" ] && { echo "skip raw n=$n"; continue; }
  echo "=== raw n=$n $(date '+%H:%M:%S')"
  RAYON_NUM_THREADS=8 "$B" "$n" --reps "$REPS" --ladder custom:1:4 > "$f" 2>&1
  sleep 10
done
# The fields-witch shapes (W = 1), both rates, 1 and 10 threads.
for m in 23 25 27 29; do
  for thr in 1 10; do
    for L in custom:1:4 custom:3:4; do
      f="$RAW/fw_m${m}_t${thr}_${L//:/-}.txt"
      [ -s "$f" ] && continue
      echo "=== fields-witch m=$m threads=$thr $L $(date '+%H:%M:%S')"
      RAYON_NUM_THREADS=$thr "$B" "$m" --reps "$REPS" --ladder "$L" > "$f" 2>&1
      sleep 5
    done
  done
done
# Last: u128 at 2^21 (the heaviest case: one process per rate).
wf u128 1 custom:1:4 21 -ext
wf u128 3 custom:3:4 21 -ext
echo "campaign done $(date)"
