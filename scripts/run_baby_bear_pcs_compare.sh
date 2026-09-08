#!/usr/bin/env bash
set -euo pipefail

# Controlled prescribed-point PCS comparison for one logical BabyBear
# multiplication witness. Existing artifacts are never overwritten.

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"
PROFILER="/Users/johnwu/.ai-agent-army/skills/zk-proof-profiler/scripts/zk_trace.py"

SHAPES="${F2Z_BENCH_SHAPES:-15 16 17 18 19 20 21 22 23 24}"
REPETITIONS="${F2Z_BENCH_REPS:-21}"
THREADS="${RAYON_NUM_THREADS:-10}"
ROOT_SEED="${F2Z_BENCH_SEED:-0x4242504353000064}"
BACKENDS="${F2Z_PCS_COMPARE_BACKENDS:-f2z plonky3-whir}"
WHIR_DEGREE="${F2Z_PCS_COMPARE_WHIR_DEGREE:-5}"
NATIVE_RUSTFLAGS="${RUSTFLAGS:--Ctarget-cpu=native}"

case "$WHIR_DEGREE" in
    4)
        BENCH_FEATURES="bench-internals,plonky3-whir-bench,plonky3-whir-degree4-bench"
        ;;
    5)
        BENCH_FEATURES="bench-internals,plonky3-whir-bench"
        ;;
    *)
        echo "F2Z_PCS_COMPARE_WHIR_DEGREE must be 4 or 5" >&2
        exit 2
        ;;
esac

if [[ ! "$REPETITIONS" =~ ^[1-9][0-9]*$ ]]; then
    echo "F2Z_BENCH_REPS must be a positive decimal integer" >&2
    exit 2
fi
if [[ ! "$THREADS" =~ ^[1-9][0-9]*$ ]]; then
    echo "RAYON_NUM_THREADS must be a positive decimal integer" >&2
    exit 2
fi

RUN_STAMP="$(date -u +%Y-%m-%dT%H-%M-%SZ)"
RUN_DIR="$REPO_ROOT/PerfRuns/${RUN_STAMP}-baby-bear-f2z-vs-whir"
if [[ -e "$RUN_DIR" ]]; then
    echo "refusing to overwrite existing run directory: $RUN_DIR" >&2
    exit 2
fi

# Capture repository state before creating the run directory so the run itself
# cannot make a clean checkout appear dirty.
DETECTED_GIT_REV="$(git -C "$REPO_ROOT" rev-parse HEAD)"
if [[ -n "$(git -C "$REPO_ROOT" status --porcelain --untracked-files=all)" ]]; then
    DETECTED_GIT_DIRTY=true
else
    DETECTED_GIT_DIRTY=false
fi
DETECTED_CPU="$(system_profiler SPHardwareDataType 2>/dev/null | awk -F ': ' '/Chip:/{print $2; exit}' || true)"
if [[ -z "$DETECTED_CPU" ]]; then
    DETECTED_CPU="$(sysctl -n machdep.cpu.brand_string 2>/dev/null || uname -m)"
fi

COMPARE_GIT_REV="${F2Z_PCS_COMPARE_GIT_REV:-$DETECTED_GIT_REV}"
COMPARE_GIT_DIRTY="${F2Z_PCS_COMPARE_GIT_DIRTY:-$DETECTED_GIT_DIRTY}"
COMPARE_CPU="${F2Z_PCS_COMPARE_CPU:-$DETECTED_CPU}"
COMPARE_BUILD_PROFILE="${F2Z_PCS_COMPARE_BUILD_PROFILE:-bench}"
COMPARE_CAMPAIGN_ID="${F2Z_PCS_COMPARE_CAMPAIGN_ID:-$RUN_STAMP}"

DEFAULT_TRACE="$RUN_DIR/traces/baby-bear-pcs-compare.jsonl"
TRACE_PATH="${F2Z_PCS_COMPARE_TRACE_PATH:-$DEFAULT_TRACE}"
case "$TRACE_PATH" in
    /*) ;;
    *) TRACE_PATH="$REPO_ROOT/$TRACE_PATH" ;;
esac
if [[ -e "$TRACE_PATH" ]]; then
    echo "refusing to overwrite existing trace: $TRACE_PATH" >&2
    exit 2
fi

DEFAULT_CAMPAIGN="$RUN_DIR/metadata/campaign.json"
CAMPAIGN_PATH="${F2Z_PCS_COMPARE_CAMPAIGN_PATH:-$DEFAULT_CAMPAIGN}"
case "$CAMPAIGN_PATH" in
    /*) ;;
    *) CAMPAIGN_PATH="$REPO_ROOT/$CAMPAIGN_PATH" ;;
esac
if [[ -e "$CAMPAIGN_PATH" ]]; then
    echo "refusing to overwrite existing campaign manifest: $CAMPAIGN_PATH" >&2
    exit 2
fi
if [[ "$TRACE_PATH" == "$CAMPAIGN_PATH" ]]; then
    echo "trace and campaign manifest paths must be different" >&2
    exit 2
fi

RAW_LOG="$RUN_DIR/logs/cargo-bench.log"
CANONICAL_REPORT_DIR="$RUN_DIR/reports/canonical"
COMPARISON_REPORT_DIR="$RUN_DIR/reports/comparison"
mkdir -p -- "$RUN_DIR/logs" "$(dirname -- "$TRACE_PATH")" "$(dirname -- "$CAMPAIGN_PATH")"

echo "Run directory: $RUN_DIR"
echo "Shapes: $SHAPES"
echo "Measured repetitions: $REPETITIONS (plus one warmup per runnable cell)"
echo "Backends: $BACKENDS"
echo "WHIR challenge extension degree: $WHIR_DEGREE"
echo "Rayon threads: $THREADS"
echo "Root seed: $ROOT_SEED"
echo "RUSTFLAGS: $NATIVE_RUSTFLAGS"
echo "Commit: $COMPARE_GIT_REV (dirty=$COMPARE_GIT_DIRTY)"
echo "Campaign ID: $COMPARE_CAMPAIGN_ID"
echo "CPU: $COMPARE_CPU"
echo "Trace: $TRACE_PATH"
echo "Campaign manifest: $CAMPAIGN_PATH"

(
    cd -- "$REPO_ROOT"
    RUSTFLAGS="$NATIVE_RUSTFLAGS" \
    RAYON_NUM_THREADS="$THREADS" \
    F2Z_BENCH_SHAPES="$SHAPES" \
    F2Z_BENCH_REPS="$REPETITIONS" \
    F2Z_BENCH_SEED="$ROOT_SEED" \
    F2Z_PCS_COMPARE_BACKENDS="$BACKENDS" \
    F2Z_PCS_COMPARE_CAMPAIGN_ID="$COMPARE_CAMPAIGN_ID" \
    F2Z_PCS_COMPARE_TRACE_PATH="$TRACE_PATH" \
    F2Z_PCS_COMPARE_CAMPAIGN_PATH="$CAMPAIGN_PATH" \
    F2Z_PCS_COMPARE_GIT_REV="$COMPARE_GIT_REV" \
    F2Z_PCS_COMPARE_GIT_DIRTY="$COMPARE_GIT_DIRTY" \
    F2Z_PCS_COMPARE_CPU="$COMPARE_CPU" \
    F2Z_PCS_COMPARE_BUILD_PROFILE="$COMPARE_BUILD_PROFILE" \
        cargo bench --bench baby_bear_pcs_compare --features "$BENCH_FEATURES"
) 2>&1 | tee "$RAW_LOG"

if [[ ! -e "$TRACE_PATH" ]]; then
    echo "benchmark completed without producing a trace file: $TRACE_PATH" >&2
    exit 2
fi
if [[ ! -s "$CAMPAIGN_PATH" ]]; then
    echo "benchmark completed without producing a non-empty campaign manifest: $CAMPAIGN_PATH" >&2
    exit 2
fi

if [[ -s "$TRACE_PATH" ]]; then
    python3 "$PROFILER" validate "$TRACE_PATH"
    python3 "$PROFILER" report "$TRACE_PATH" --out-dir "$CANONICAL_REPORT_DIR"
else
    echo "No runnable cells produced intervals; skipping canonical trace report."
fi
python3 "$SCRIPT_DIR/baby_bear_pcs_compare_report.py" \
    "$TRACE_PATH" \
    --campaign "$CAMPAIGN_PATH" \
    --out-dir "$COMPARISON_REPORT_DIR"

echo "Raw benchmark log: $RAW_LOG"
echo "Validated trace: $TRACE_PATH"
echo "Campaign manifest: $CAMPAIGN_PATH"
if [[ -s "$TRACE_PATH" ]]; then
    echo "Canonical interval report: $CANONICAL_REPORT_DIR/intervals.html"
fi
echo "Comparison report: $COMPARISON_REPORT_DIR/comparison.html"
