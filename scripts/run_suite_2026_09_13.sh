#!/usr/bin/env bash
# The 2026-09-13 bench-suite campaign queue (docs/native-mul-compare.md, "The
# suite"): integer-mult tables at odd exponents, every scheme at 1 and 10
# threads, Binius suite under round-by-round accounting, Limber at the pinned
# 100-bit Brakedown target, Plonky3-FRI at rate 1/2.
#
# Every campaign is one fresh runner process, serialized through
# scripts/bench_gate.py (machine lock + sustained-idle wait + swap guard). Run
# this ONLY when no other session is measuring (the gate waits, but a campaign
# started elsewhere without the lock will still collide) and with the source
# tree frozen — the runners reject tracked-source edits mid-campaign.
#
#   bash scripts/run_suite_2026_09_13.sh [u32] [u64] [u128] [multiswap]
#
# No arguments = all phases in that order. Paging policy: cells known to page
# are excluded, except the Binius64-family rows, which run under a raised swap
# guard (the guard aborts a runaway; an aborted campaign exits 86 and the
# queue stops there). Output: PerfRuns/suite-<label>. A label that already
# exists fails the runner: remove the directory or rename before re-running.
set -euo pipefail
cd "$(dirname "$0")/.."

REPS="${F2Z_SUITE_REPS:-5}"
ODD="15 17 19 21 23"           # the suite's odd exponents (u32-mod32)
ODD_U64_F2Z="15 17 19 21"      # f2z u64: 23 would page (non-Binius cells are not run while paging)
ODD_U64_BIN="15 17 19 21"      # binius u64: 21 pages, allowed with a raised guard; 23 is unreasonable
ODD_U64_LIMBER="15 17 19"      # limber: 21 pages
ODD_U128_F2Z="15 17 19 21"
ODD_U128_BIN="15 17 19 21"     # binius u128 2^21 pages hard (~27 GB swap); raised guard, watchdog decides
ODD_U128_LIG="15 17 19"        # opener at u128 2^21: paging slowness too large, skipped
ODD_U128_LIMBER="15 17 19"
ODD_LIMBER_U32="15 17 19"      # limber u32: 21 pages

phases="$*"
[ -z "$phases" ] && phases="sha-ecdsa hybrid-counts hybrid-witness u32 u64 u128 multiswap"
has() { case " $phases " in *" $1 "*) return 0;; *) return 1;; esac; }

# One hybrid sweep = one (mode, rate) at one thread count over the variant's
# shapes; the table script joins them per (row, threads). Leading KEY=VAL
# arguments become environment for the bench; the rest are bench arguments.
hybrid_sweep() { # label swap_gb threads shapes [KEY=VAL...] --mode ...
  local label=$1 guard=$2 threads=$3 shapes=$4; shift 4
  local envs=()
  while [ $# -gt 0 ] && [[ $1 == *=* && $1 != --* ]]; do envs+=("$1"); shift; done
  python3 scripts/bench_gate.py run --label "$label" --swap-grow-gb "$guard" -- \
    env RUSTFLAGS="-C target-cpu=native" RAYON_NUM_THREADS="$threads" \
      ${envs[@]+"${envs[@]}"} \
      cargo +1.98.1 bench --bench hybrid_u32_sha256 --features hybrid -- \
      --sweep --shapes "$shapes" --results-dir "PerfRuns/suite-$label" "$@"
}
HY_COUNTS="9:9,10:10,11:11,12:12,13:13,14:14"
HY_WITNESS="15:7,16:8,17:9,18:10,19:11,20:12"

hybrid_phase() { # phase-name shapes
  local name=$1 shapes=$2
  for T in 10 1; do
    hybrid_sweep "$name-f2z-r2-t$T"  12 "$T" "$shapes" --mode hybrid
    hybrid_sweep "$name-f2z-r8-t$T"  12 "$T" "$shapes" --mode hybrid --profile custom:3:4
    hybrid_sweep "$name-bin-r1-t$T"  30 "$T" "$shapes" F2Z_HYBRID_BINIUS_LOG_INV_RATE=1 --mode all-binius
    hybrid_sweep "$name-bin-r3-t$T"  30 "$T" "$shapes" F2Z_HYBRID_BINIUS_LOG_INV_RATE=3 --mode all-binius
    hybrid_sweep "$name-lig-r1-t$T"  30 "$T" "$shapes" F2Z_BINIUS_LOG_INV_RATE=1 F2Z_BINIUS_LIGERITO_ACCOUNTING=rbr --mode binius-ligerito
    hybrid_sweep "$name-lig-r3-t$T"  30 "$T" "$shapes" F2Z_BINIUS_LOG_INV_RATE=3 F2Z_BINIUS_LIGERITO_ACCOUNTING=rbr --mode binius-ligerito
  done
}

campaign() { # label swap_guard_gb shapes env...
  local label=$1 guard=$2 shapes=$3; shift 3
  python3 scripts/bench_gate.py run --label "$label" --swap-grow-gb "$guard" -- \
    env "$@" F2Z_BENCH_REPS="$REPS" F2Z_BENCH_SHAPES="$shapes" \
        F2Z_MUL_COMPARE_OUTPUT_DIR="PerfRuns/suite-$label" \
        bash scripts/run_native_mul_compare.sh
}

if has sha-ecdsa; then
  # The complete SHA+ECDSA matrix (F2Z rho=1/2,1/8; Binius64 rho=1/2,1/8;
  # opener rho=1/2,1/8 rbr) at threads 1 and 10, one runner invocation.
  python3 scripts/bench_gate.py run --label sha-ecdsa --swap-grow-gb 12 -- \
    python3 scripts/run_sha256_ecdsa_compare.py \
      --output "bench_results/suite-sha256-ecdsa-$(date +%Y%m%d)" --exponents 7
fi
if has hybrid-counts;  then hybrid_phase hy-counts  "$HY_COUNTS";  fi
if has hybrid-witness; then hybrid_phase hy-witness "$HY_WITNESS"; fi

for T in 10 1; do
  if has u32; then
    campaign "u32-f2z-r2-t$T"   10 "$ODD" RAYON_NUM_THREADS="$T" F2Z_MUL_COMPARE_BACKENDS=f2z
    campaign "u32-f2z-r8-t$T"   10 "$ODD" RAYON_NUM_THREADS="$T" F2Z_MUL_COMPARE_BACKENDS=f2z F2Z_LIG_PROFILE=custom:3:4
    campaign "u32-bin-r1-t$T"   30 "$ODD" RAYON_NUM_THREADS="$T" F2Z_MUL_COMPARE_BACKENDS=binius64 F2Z_BINIUS_LOG_INV_RATE=1
    campaign "u32-bin-r3-t$T"   30 "$ODD" RAYON_NUM_THREADS="$T" F2Z_MUL_COMPARE_BACKENDS=binius64 F2Z_BINIUS_LOG_INV_RATE=3
    campaign "u32-lig-r1-t$T"   30 "$ODD" RAYON_NUM_THREADS="$T" F2Z_MUL_COMPARE_BACKENDS=binius64-ligerito F2Z_BINIUS_LOG_INV_RATE=1 F2Z_BINIUS_LIGERITO_ACCOUNTING=rbr
    campaign "u32-lig-r3-t$T"   30 "$ODD" RAYON_NUM_THREADS="$T" F2Z_MUL_COMPARE_BACKENDS=binius64-ligerito F2Z_BINIUS_LOG_INV_RATE=3 F2Z_BINIUS_LIGERITO_ACCOUNTING=rbr
    campaign "u32-fri-t$T"      10 "$ODD" RAYON_NUM_THREADS="$T" F2Z_MUL_COMPARE_BACKENDS=plonky3-fri
    campaign "u32-limber-t$T"   10 "$ODD_LIMBER_U32" RAYON_NUM_THREADS="$T" F2Z_MUL_COMPARE_BACKENDS=limber
  fi
  if has u64; then
    campaign "u64-f2z-r2-t$T"   10 "$ODD_U64_F2Z" RAYON_NUM_THREADS="$T" F2Z_MUL_COMPARE_WORKLOADS=u64 F2Z_MUL_COMPARE_BACKENDS=f2z
    campaign "u64-f2z-r8-t$T"   10 "$ODD_U64_F2Z" RAYON_NUM_THREADS="$T" F2Z_MUL_COMPARE_WORKLOADS=u64 F2Z_MUL_COMPARE_BACKENDS=f2z F2Z_LIG_PROFILE=custom:3:4
    campaign "u64-bin-r1-t$T"   30 "$ODD_U64_BIN" RAYON_NUM_THREADS="$T" F2Z_MUL_COMPARE_WORKLOADS=u64 F2Z_MUL_COMPARE_BACKENDS=binius64 F2Z_BINIUS_LOG_INV_RATE=1
    campaign "u64-bin-r3-t$T"   30 "$ODD_U64_BIN" RAYON_NUM_THREADS="$T" F2Z_MUL_COMPARE_WORKLOADS=u64 F2Z_MUL_COMPARE_BACKENDS=binius64 F2Z_BINIUS_LOG_INV_RATE=3
    campaign "u64-lig-r1-t$T"   30 "$ODD_U64_BIN" RAYON_NUM_THREADS="$T" F2Z_MUL_COMPARE_WORKLOADS=u64 F2Z_MUL_COMPARE_BACKENDS=binius64-ligerito F2Z_BINIUS_LOG_INV_RATE=1 F2Z_BINIUS_LIGERITO_ACCOUNTING=rbr
    campaign "u64-lig-r3-t$T"   30 "$ODD_U64_BIN" RAYON_NUM_THREADS="$T" F2Z_MUL_COMPARE_WORKLOADS=u64 F2Z_MUL_COMPARE_BACKENDS=binius64-ligerito F2Z_BINIUS_LOG_INV_RATE=3 F2Z_BINIUS_LIGERITO_ACCOUNTING=rbr
    campaign "u64-limber-t$T"   10 "$ODD_U64_LIMBER" RAYON_NUM_THREADS="$T" F2Z_MUL_COMPARE_WORKLOADS=u64 F2Z_MUL_COMPARE_BACKENDS=limber
  fi
  if has u128; then
    campaign "u128-f2z-r2-t$T"  10 "$ODD_U128_F2Z" RAYON_NUM_THREADS="$T" F2Z_MUL_COMPARE_WORKLOADS=u128 F2Z_MUL_COMPARE_BACKENDS=f2z
    campaign "u128-f2z-r8-t$T"  10 "$ODD_U128_F2Z" RAYON_NUM_THREADS="$T" F2Z_MUL_COMPARE_WORKLOADS=u128 F2Z_MUL_COMPARE_BACKENDS=f2z F2Z_LIG_PROFILE=custom:3:4
    campaign "u128-bin-r1-t$T"  34 "$ODD_U128_BIN" RAYON_NUM_THREADS="$T" F2Z_MUL_COMPARE_WORKLOADS=u128 F2Z_MUL_COMPARE_BACKENDS=binius64 F2Z_BINIUS_LOG_INV_RATE=1
    campaign "u128-bin-r3-t$T"  34 "$ODD_U128_BIN" RAYON_NUM_THREADS="$T" F2Z_MUL_COMPARE_WORKLOADS=u128 F2Z_MUL_COMPARE_BACKENDS=binius64 F2Z_BINIUS_LOG_INV_RATE=3
    campaign "u128-lig-r1-t$T"  30 "$ODD_U128_LIG" RAYON_NUM_THREADS="$T" F2Z_MUL_COMPARE_WORKLOADS=u128 F2Z_MUL_COMPARE_BACKENDS=binius64-ligerito F2Z_BINIUS_LOG_INV_RATE=1 F2Z_BINIUS_LIGERITO_ACCOUNTING=rbr
    campaign "u128-lig-r3-t$T"  30 "$ODD_U128_LIG" RAYON_NUM_THREADS="$T" F2Z_MUL_COMPARE_WORKLOADS=u128 F2Z_MUL_COMPARE_BACKENDS=binius64-ligerito F2Z_BINIUS_LOG_INV_RATE=3 F2Z_BINIUS_LIGERITO_ACCOUNTING=rbr
    campaign "u128-limber-t$T"  10 "$ODD_U128_LIMBER" RAYON_NUM_THREADS="$T" F2Z_MUL_COMPARE_WORKLOADS=u128 F2Z_MUL_COMPARE_BACKENDS=limber
  fi
done

if has multiswap; then
  # MultiSwap re-measure at 10 threads (was 8): the matched campaign as in
  # README "MultiSwap", with --all-threads 10. Prepare the Limber checkout
  # first (scripts/prepare_matched_limber.py). The campaign script measures
  # single-threaded and all-threads modes itself.
  python3 scripts/bench_gate.py run --label multiswap-t10 --swap-grow-gb 10 -- \
    python3 scripts/run_matched_multiswap_campaign.py \
      --draft \
      --limber-root /tmp/limber-matched114 \
      --security-bits 114 \
      --batch-counts 1,2,4,8,16 \
      --all-threads 10 \
      --warmups 1 \
      --samples 10 \
      --rustflags="-C target-cpu=native"
fi

# Export (after the campaigns; adjust --paging/--unsupported to what the runs
# actually recorded — the runner logs and memory.jsonl say which cells paged):
#   python3 scripts/native_mul_table.py PerfRuns/suite-u32-*-t10 PerfRuns/suite-u32-*-t1 \
#     --workload u32-mod32 --exponents 15,17,19,21,23 --out paper/native-mul-table.tex
#   python3 scripts/native_mul_table.py PerfRuns/suite-u64-*-t10 PerfRuns/suite-u64-*-t1 \
#     --workload u64 --out paper/native-mul-u64-table.tex
#   python3 scripts/native_mul_table.py PerfRuns/suite-u128-*-t10 PerfRuns/suite-u128-*-t1 \
#     --workload u128 --unsupported binius64:23 \
#     --unsupported-reason "Binius64's MAX_VALUES_PER_SEGMENT refuses the size" \
#     --out paper/native-mul-u128-table.tex
echo "suite queue done"
