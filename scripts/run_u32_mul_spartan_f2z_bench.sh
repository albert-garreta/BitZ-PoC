#!/usr/bin/env bash
set -euo pipefail

# Run the canonical u32 × u32 → u64 Spartan/F2Z sweep. One uninstrumented
# latency executable and, when requested, one peak-allocator executable are
# built, then invoked in fresh processes for every (word width, size, pass)
# tuple.
#
# Usage:
#   scripts/run_u32_mul_spartan_f2z_bench.sh [summary.csv]
#
# The sibling artifacts are `<stem>_raw.csv` and `<stem>.log`. Existing files
# are never overwritten.

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"

EXPONENTS="${F2Z_BENCH_SHAPES:-15 16 17 18 19 20 21 22 23 24 25}"
WORD_BITS="${F2Z_MUL_WORD_BITS:-1}"
REPETITIONS="${F2Z_BENCH_REPS:-5}"
FEATURES="${F2Z_BENCH_FEATURES:-unchecked},span-metrics"
THREADS="${RAYON_NUM_THREADS:-10}"
STRATEGY="delayed-barrett"
MEASURE_MEMORY="${F2Z_BENCH_MEASURE_MEMORY:-1}"
ROOT_SEED="${F2Z_BENCH_SEED:-0x5533326d756c0064}"
# Runner-only knobs must not leak into the benchmark binary: its strict
# environment validator rejects names the binary itself does not consume.
unset F2Z_BENCH_FEATURES F2Z_BENCH_MEASURE_MEMORY
RUN_TIMESTAMP="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
RUN_DATE="${RUN_TIMESTAMP%%T*}"
RUN_STAMP="$(date -u +%Y%m%d-%H%M%S)"

DEFAULT_SUMMARY="$REPO_ROOT/bench_results/u32_mul_spartan_f2z_${RUN_STAMP}_summary.csv"
SUMMARY_CSV="${1:-$DEFAULT_SUMMARY}"
case "$SUMMARY_CSV" in
    /*) ;;
    *) SUMMARY_CSV="$REPO_ROOT/$SUMMARY_CSV" ;;
esac
case "$SUMMARY_CSV" in
    *.csv) ;;
    *) echo "summary output must end in .csv" >&2; exit 2 ;;
esac

ARTIFACT_STEM="${SUMMARY_CSV%.csv}"
case "$ARTIFACT_STEM" in
    *_summary) ARTIFACT_STEM="${ARTIFACT_STEM%_summary}" ;;
esac
RAW_CSV="${ARTIFACT_STEM}_raw.csv"
RAW_LOG="${ARTIFACT_STEM}.log"
OUTPUT_DIR="$(dirname -- "$SUMMARY_CSV")"

for output in "$SUMMARY_CSV" "$RAW_CSV" "$RAW_LOG"; do
    if [[ -e "$output" ]]; then
        echo "refusing to overwrite existing artifact: $output" >&2
        exit 2
    fi
done
mkdir -p -- "$OUTPUT_DIR"

COMMIT="$(git -C "$REPO_ROOT" rev-parse HEAD)"
if [[ -n "$(git -C "$REPO_ROOT" status --porcelain --untracked-files=all)" ]]; then
    GIT_DIRTY=true
else
    GIT_DIRTY=false
fi

BUILD_DIR="$(mktemp -d -t f2z-u32-mul-build.XXXXXX)"
trap 'rm -rf -- "$BUILD_DIR"' EXIT

build_benchmark() {
    local label="$1"
    local features="$2"
    local build_log="$BUILD_DIR/$label.log"
    echo "Building $label u32_mul binary with features: $features" >&2
    (
        cd -- "$REPO_ROOT"
        cargo bench --bench u32_mul --features "$features" --no-run 2>&1
    ) | tee "$build_log" >&2

    local binary
    binary="$(sed -n 's/^  Executable .* (\(.*u32_mul-[^)]*\))$/\1/p' "$build_log" | tail -n 1)"
    if [[ -z "$binary" ]]; then
        echo "could not locate the $label u32_mul benchmark executable" >&2
        exit 2
    fi
    case "$binary" in
        /*) ;;
        *) binary="$REPO_ROOT/$binary" ;;
    esac
    if [[ ! -x "$binary" ]]; then
        echo "benchmark executable is not runnable: $binary" >&2
        exit 2
    fi
    echo "$binary"
}

# The latency executable has no allocator wrapper at all. Peak-heap accounting
# lives in a separately compiled executable so its atomics cannot bias the
# latency comparison.
LATENCY_BENCH_BINARY="$(build_benchmark latency "$FEATURES")"
LATENCY_BINARY_SHA256="$(shasum -a 256 "$LATENCY_BENCH_BINARY" | awk '{print $1}')"
MEMORY_BENCH_BINARY=""
MEMORY_BINARY_SHA256=""
if [[ "$MEASURE_MEMORY" == "1" ]]; then
    MEMORY_FEATURES="$FEATURES,bench-peak-memory"
    MEMORY_BENCH_BINARY="$(build_benchmark memory "$MEMORY_FEATURES")"
    MEMORY_BINARY_SHA256="$(shasum -a 256 "$MEMORY_BENCH_BINARY" | awk '{print $1}')"
elif [[ "$MEASURE_MEMORY" != "0" ]]; then
    echo "F2Z_BENCH_MEASURE_MEMORY must be 0 or 1" >&2
    exit 2
fi
HARDWARE="$(system_profiler SPHardwareDataType 2>/dev/null | awk -F ': ' '/Chip:/{print $2; exit}' || true)"
if [[ -z "$HARDWARE" ]]; then
    HARDWARE="$(sysctl -n machdep.cpu.brand_string 2>/dev/null || uname -m)"
fi
OPERATING_SYSTEM="$(uname -srm)"

echo "Exponents: $EXPONENTS"
echo "F2Z word bits: $WORD_BITS"
echo "Strategy: $STRATEGY (canonical)"
echo "Measured repetitions: $REPETITIONS (plus one warmup per process)"
echo "Rayon threads: $THREADS"
echo "Root seed: $ROOT_SEED"
echo "Outer sumcheck univariate skip K: 3 (canonical)"
echo "Commit: $COMMIT (dirty=$GIT_DIRTY)"
echo "Latency binary SHA-256: $LATENCY_BINARY_SHA256"
if [[ -n "$MEMORY_BINARY_SHA256" ]]; then
    echo "Memory binary SHA-256: $MEMORY_BINARY_SHA256"
fi
echo "Hardware: $HARDWARE"
echo "Operating system: $OPERATING_SYSTEM"
echo "Allocator instrumentation: absent from latency binary; enabled only in memory binary"
echo "Raw log: $RAW_LOG"

run_case() {
    local binary="$1"
    local pass="$2"
    local word_bits="$3"
    local exponent="$4"
    local order="$5"
    echo "RUN pass=$pass word_bits=$word_bits exponent=$exponent strategy=$STRATEGY order=$order" | tee -a "$RAW_LOG"
    OBLONG_PROFILE=1 \
    RAYON_NUM_THREADS="$THREADS" \
    F2Z_MUL_WORD_BITS="$word_bits" \
    F2Z_BENCH_SHAPES="$exponent" \
    F2Z_BENCH_REPS="$REPETITIONS" \
    F2Z_BENCH_PASS="$pass" \
    F2Z_BENCH_ORDER="$order" \
    F2Z_BENCH_SEED="$ROOT_SEED" \
        "$binary" 2>&1 | tee -a "$RAW_LOG"
}

for word_bits in $WORD_BITS; do
    if [[ "$word_bits" != "1" && "$word_bits" != "8" ]]; then
        echo "F2Z_MUL_WORD_BITS entries must be 1 or 8; got $word_bits" >&2
        exit 2
    fi
    for exponent in $EXPONENTS; do
        run_case "$LATENCY_BENCH_BINARY" latency "$word_bits" "$exponent" 1
    done
done

if [[ "$MEASURE_MEMORY" == "1" ]]; then
    for word_bits in $WORD_BITS; do
        for exponent in $EXPONENTS; do
            run_case "$MEMORY_BENCH_BINARY" memory "$word_bits" "$exponent" 1
        done
    done
fi

python3 "$SCRIPT_DIR/u32_mul_bench_report.py" \
    "$RAW_LOG" \
    --raw-csv "$RAW_CSV" \
    --summary-csv "$SUMMARY_CSV" \
    --benchmark-date "$RUN_DATE" \
    --run-timestamp "$RUN_TIMESTAMP" \
    --commit "$COMMIT" \
    --git-dirty "$GIT_DIRTY" \
    --binary-sha256 "$LATENCY_BINARY_SHA256" \
    --memory-binary-sha256 "$MEMORY_BINARY_SHA256" \
    --hardware "$HARDWARE" \
    --operating-system "$OPERATING_SYSTEM" \
    --cargo-features "$FEATURES" \
    --root-seed "$ROOT_SEED" \
    --threads "$THREADS" \
    --measured-runs "$REPETITIONS" \
    --expected-word-bits "$WORD_BITS" \
    --expected-exponents "$EXPONENTS" \
    --measure-memory "$MEASURE_MEMORY"

echo "Saved raw samples: $RAW_CSV"
echo "Saved medians: $SUMMARY_CSV"
