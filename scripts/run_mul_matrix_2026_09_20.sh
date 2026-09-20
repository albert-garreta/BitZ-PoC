#!/usr/bin/env bash
# The 2026-09-20 multiplication campaign queue: every backend re-measured in
# one window on the 24 GiB M5, one launcher campaign per (workload, backend,
# rate), sizes 2^15..2^19 for everyone plus the larger sizes each backend is
# known to fit, then per-cell memory probes for the sizes that may not fit.
#
#   bash scripts/run_mul_matrix_2026_09_20.sh            # main grid + extensions + probes
#   bash scripts/run_mul_matrix_2026_09_20.sh probes     # probes only
#
# Runs under ONE outer bench_gate lock (idle wait once; the launcher takes
# --no-gate). Set CARGO_TARGET_DIR to the tree you built; the launcher builds
# the bench there and resolves the executable from Cargo's artifact record.
# Output: PerfRuns/cs-mul-<label>, PerfRuns/cs-mul-exclusions.jsonl.
set -uo pipefail
cd "$(dirname "$0")/.."
export RUSTFLAGS="${RUSTFLAGS:--C target-cpu=native}"
REPS="${BITZ_SUITE_REPS:-5}"
STAMP="${BITZ_SUITE_STAMP:-$(date +%Y%m%d)}"
OUT="PerfRuns/cs-mul-$STAMP"
mkdir -p PerfRuns
RECORD="PerfRuns/cs-mul-exclusions-$STAMP.jsonl"
phases="${*:-grid ext probes}"
has() { case " $phases " in *" $1 "*) return 0;; *) return 1;; esac; }

launch() { # label -- rust args
  local label=$1; shift
  local dir="$OUT-$label"
  if [ -d "$dir" ]; then echo "skip $label: $dir exists"; return 0; fi
  echo "=== $label $(date)"
  python3 scripts/run_multiplication_benchmarks.py compare --no-gate --output "$dir" -- \
    proof --reps "$REPS" --warmups 1 --memory rss --skip-unsupported --threads 1,10 "$@" \
    2>&1 | tail -3
}
bitz()   { launch "$1-bitz-r$3" --workload "$1" --backends bitz --bitz-profile 100 --ligerito "custom:$3:4" --log-n "$2"; }
binius() { launch "$1-binius64-r$3" --workload "$1" --backends binius64 --log-inv-rate "$3" --log-n "$2"; }
lig()    { launch "$1-lig-rbr-r$3" --workload "$1" --backends binius64-ligerito --log-inv-rate "$3" --binius-ligerito-accounting rbr --log-n "$2"; }
p3()     { launch "$1-plonky3-fri" --workload "$1" --backends plonky3-fri --log-inv-rate 1 --log-n "$2"; }
limber() { launch "$1-limber" --workload "$1" --backends limber --limber-bits 100 --log-n "$2"; }

probe() { # workload backend log_n threads rate [extra...]
  local w=$1 b=$2 n=$3 t=$4 r=$5; shift 5
  local label="$w-$b-r$r-2p$n-t$t"
  local dir="$OUT-probe-$label"
  if [ -d "$dir" ]; then echo "skip probe $label"; return 0; fi
  echo "=== probe $label $(date)"
  python3 scripts/mul_memory_probe.py --no-gate --output "$dir" --workload "$w" --backend "$b" \
    --log-n "$n" --threads "$t" --log-inv-rate "$r" --reps "$REPS" --record "$RECORD" "$@" 2>&1 | tail -1
}

if has grid; then
  # Sizes every backend is known to fit (r0918c peaks: BitZ u128 2^21 10.8 GiB,
  # u32 2^23 10.6 GiB; Limber is measured to 2^19 as in the paper).
  for w in u32-mod32 u64 u128; do
    bitz $w 15,17,19 1; bitz $w 15,17,19 3
    binius $w 15,17,19 1; binius $w 15,17,19 3
    lig $w 15,17,19 1; lig $w 15,17,19 3
    p3 $w 15,17,19
    limber $w 15,17,19
  done
fi
if has ext; then
  # Larger sizes with headroom on 24 GiB.
  bitz u32-mod32 21,23 1; bitz u32-mod32 21,23 3
  bitz u64 21 1;          bitz u64 21 3
  bitz u128 21 1;         bitz u128 21 3
  binius u32-mod32 21 1;  binius u32-mod32 21 3
  binius u64 21 1;        binius u64 21 3
  lig u32-mod32 21 1;     lig u32-mod32 21 3
  lig u64 21 1;           lig u64 21 3
  p3 u32-mod32 21
fi
if has probes; then
  # Cells that may exhaust memory: measured alone; excluded with the observed
  # peak when the probe has to kill them. Ten threads first (the smaller
  # working set), then one thread only if ten fit.
  for t in 10 1; do
    probe u64 bitz 23 $t 1 --bitz-profile custom:1:4
    probe u32-mod32 binius64 23 $t 1
    probe u64 plonky3-fri 21 $t 1
    probe u128 plonky3-fri 21 $t 1
    probe u128 binius64 21 $t 1
    probe u128 binius64-ligerito 21 $t 1 --binius-ligerito-accounting rbr
    probe u32-mod32 limber 21 $t 1
    probe u64 limber 21 $t 1
    probe u128 limber 21 $t 1
  done
fi
echo "queue done $(date)"
