#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"
PROFILER="${ZK_TRACE_SCRIPT:-$HOME/.ai-agent-army/skills/zk-proof-profiler/scripts/zk_trace.py}"
RUN_STAMP="$(date -u +%Y-%m-%dT%H-%M-%SZ)"
RUN_DIR="${F2Z_MUL_COMPARE_OUTPUT_DIR:-$REPO_ROOT/PerfRuns/${RUN_STAMP}-native-mul}"
case "$RUN_DIR" in /*) ;; *) RUN_DIR="$REPO_ROOT/$RUN_DIR" ;; esac
if [[ -e "$RUN_DIR" ]]; then
    echo "refusing to overwrite run directory: $RUN_DIR" >&2
    exit 2
fi
mkdir -p -- "$RUN_DIR"
(
    cd -- "$REPO_ROOT"
    RUSTFLAGS="${RUSTFLAGS:--Ctarget-cpu=native}" \
    RAYON_NUM_THREADS="${RAYON_NUM_THREADS:-8}" \
    F2Z_BENCH_SHAPES="${F2Z_BENCH_SHAPES:-15}" \
    F2Z_BENCH_REPS="${F2Z_BENCH_REPS:-5}" \
    F2Z_MUL_COMPARE_WORKLOADS="${F2Z_MUL_COMPARE_WORKLOADS:-u32 babybear}" \
    F2Z_MUL_COMPARE_BACKENDS="${F2Z_MUL_COMPARE_BACKENDS:-f2z binius64 binius64-ligerito plonky3-whir limber}" \
    F2Z_MUL_COMPARE_OUTPUT_DIR="$RUN_DIR" \
        cargo bench --bench mul_e2e_compare --features bench-internals,native-mul-compare
) 2>&1 | tee "$RUN_DIR/cargo-bench.log"
if [[ -f "$PROFILER" ]]; then
    python3 "$PROFILER" validate "$RUN_DIR/trace.jsonl"
    python3 "$PROFILER" report "$RUN_DIR/trace.jsonl" \
        --out-dir "$RUN_DIR/reports" --title "Native u32 and BabyBear multiplication"
    echo "Interactive report: $RUN_DIR/reports/intervals.html"
else
    echo "Set ZK_TRACE_SCRIPT to zk_trace.py to validate and render the saved trace."
fi
echo "Metrics: $RUN_DIR/metrics.csv"
echo "Summary: $RUN_DIR/summary.json"
echo "Memory samples: $RUN_DIR/memory.jsonl"
