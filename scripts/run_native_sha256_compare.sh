#!/usr/bin/env bash
set -euo pipefail

# Native fixed-IV SHA-256 compression comparison across F2Z, Plonky3/WHIR,
# Binius64, Limber Spartan/Hyrax, and integer-mod Limber. Existing run
# directories are never overwritten.

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"
PROFILER="${ZK_TRACE_SCRIPT:-$HOME/.ai-agent-army/skills/zk-proof-profiler/scripts/zk_trace.py}"

EXPONENTS="${F2Z_SHA_COMPARE_EXPONENTS:-10 11 12 13 14 15 16}"
REPETITIONS="${F2Z_SHA_COMPARE_REPS:-21}"
PILOT_REPETITIONS="${F2Z_SHA_COMPARE_PILOT_REPS:-5}"
THREADS="${RAYON_NUM_THREADS:-8}"
NATIVE_RUSTFLAGS="${RUSTFLAGS:--Ctarget-cpu=native}"

RUN_STAMP="$(date -u +%Y-%m-%dT%H-%M-%SZ)"
RUN_DIR="${F2Z_SHA_COMPARE_RUN_DIR:-$REPO_ROOT/PerfRuns/${RUN_STAMP}-native-sha256-five-way}"
ARTIFACT_DIR="$RUN_DIR/artifacts"
TRACE_PATH="$ARTIFACT_DIR/trace.jsonl"
REPORT_DIR="$RUN_DIR/reports/intervals"
RAW_LOG="$RUN_DIR/logs/cargo-bench.log"

if [[ -e "$RUN_DIR" ]]; then
    echo "refusing to overwrite existing run directory: $RUN_DIR" >&2
    exit 2
fi
mkdir -p -- "$ARTIFACT_DIR" "$RUN_DIR/logs"

echo "Run directory: $RUN_DIR"
echo "Compression exponents: $EXPONENTS"
echo "Measured repetitions: $REPETITIONS (plus one warmup)"
echo "Pilot repetitions: $PILOT_REPETITIONS"
echo "Rayon threads: $THREADS"
echo "RUSTFLAGS: $NATIVE_RUSTFLAGS"

(
    cd -- "$REPO_ROOT"
    RUSTFLAGS="$NATIVE_RUSTFLAGS" \
    RAYON_NUM_THREADS="$THREADS" \
    F2Z_SHA_COMPARE_THREADS="$THREADS" \
    F2Z_SHA_COMPARE_EXPONENTS="$EXPONENTS" \
    F2Z_SHA_COMPARE_REPS="$REPETITIONS" \
    F2Z_SHA_COMPARE_PILOT_REPS="$PILOT_REPETITIONS" \
    F2Z_SHA_COMPARE_OUTPUT_DIR="$ARTIFACT_DIR" \
    F2Z_SHA_COMPARE_TRACE_PATH="$TRACE_PATH" \
        cargo bench --bench sha256_e2e_compare \
        --features bench-internals,native-sha256-compare
) 2>&1 | tee "$RAW_LOG"

if [[ -f "$PROFILER" ]]; then
    python3 "$PROFILER" validate "$TRACE_PATH"
    python3 "$PROFILER" report "$TRACE_PATH" \
        --out-dir "$REPORT_DIR" \
        --title "Native SHA-256 compression: five-way comparison"
    echo "Interactive report: $REPORT_DIR/intervals.html"
else
    echo "Set ZK_TRACE_SCRIPT to zk_trace.py to render the saved trace."
fi

echo "Raw benchmark log: $RAW_LOG"
echo "Canonical trace: $TRACE_PATH"
echo "Summary: $ARTIFACT_DIR/summary.json"
echo "Metrics: $ARTIFACT_DIR/metrics.csv"
