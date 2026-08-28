#!/usr/bin/env bash
set -euo pipefail

# Run the u32 × u32 → u64 Spartan/F2Z sweep with a paired immediate-versus-
# delayed design. One uninstrumented latency executable and, when requested,
# one peak-allocator executable are built, then invoked in fresh processes for
# every (word width, size, strategy, pass) tuple.
#
# Usage:
#   scripts/run_u32_mul_spartan_f2z_bench.sh [summary.csv]
#
# The sibling artifacts are `<stem>_raw.csv`, `<stem>_paired.csv`, and
# `<stem>.log`. Existing files are never overwritten.

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"

EXPONENTS="${F2Z_MUL_EXPONENTS:-15 16 17 18 19 20 21 22 23 24 25}"
WORD_BITS="${F2Z_MUL_WORD_BITS:-1}"
REPETITIONS="${F2Z_BENCH_REPS:-5}"
FEATURES="${F2Z_BENCH_FEATURES:-unchecked}"
THREADS="${RAYON_NUM_THREADS:-10}"
STRATEGIES="${F2Z_BENCH_STRATEGIES:-immediate delayed-barrett delayed-crypto-bigint}"
MEMORY_STRATEGIES="${F2Z_BENCH_MEMORY_STRATEGIES:-$STRATEGIES}"
MEASURE_MEMORY="${F2Z_BENCH_MEASURE_MEMORY:-1}"
ROOT_SEED="${F2Z_MUL_SEED:-0x5533326d756c0064}"
OUTER_SKIP="${F2Z_SPARTAN_OUTER_SKIP:-0}"
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
PAIRED_CSV="${ARTIFACT_STEM}_paired.csv"
RAW_LOG="${ARTIFACT_STEM}.log"
OUTPUT_DIR="$(dirname -- "$SUMMARY_CSV")"

for output in "$SUMMARY_CSV" "$RAW_CSV" "$PAIRED_CSV" "$RAW_LOG"; do
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
echo "Strategies: $STRATEGIES"
echo "Measured repetitions: $REPETITIONS (plus one warmup per process)"
echo "Rayon threads: $THREADS"
echo "Root seed: $ROOT_SEED"
echo "Outer sumcheck univariate skip K: $OUTER_SKIP"
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
    local strategy="$5"
    local order="$6"
    echo "RUN pass=$pass word_bits=$word_bits exponent=$exponent strategy=$strategy order=$order" | tee -a "$RAW_LOG"
    OBLONG_PROFILE=1 \
    RAYON_NUM_THREADS="$THREADS" \
    F2Z_MUL_WORD_BITS="$word_bits" \
    F2Z_MUL_EXPONENTS="$exponent" \
    F2Z_BENCH_REPS="$REPETITIONS" \
    F2Z_BENCH_PASS="$pass" \
    F2Z_BENCH_ORDER="$order" \
    F2Z_SPARTAN_REDUCTION="$strategy" \
    F2Z_SPARTAN_OUTER_SKIP="$OUTER_SKIP" \
        "$binary" 2>&1 | tee -a "$RAW_LOG"
}

# Pairwise-counterbalance the primary Immediate/Barrett comparison: across
# benchmark shapes each strategy runs first equally often when the shape count
# is even. Reference
# reduction is scheduled after that pair because it is a correctness oracle,
# not the optimized path subject to the latency gate.
exponent_index=0
for word_bits in $WORD_BITS; do
    if [[ "$word_bits" != "1" && "$word_bits" != "8" ]]; then
        echo "F2Z_MUL_WORD_BITS entries must be 1 or 8; got $word_bits" >&2
        exit 2
    fi
    for exponent in $EXPONENTS; do
        read -r -a strategy_array <<< "$STRATEGIES"
        strategy_count="${#strategy_array[@]}"
        if (( strategy_count == 0 )); then
            echo "F2Z_BENCH_STRATEGIES must name at least one strategy" >&2
            exit 2
        fi
        has_immediate=0
        has_barrett=0
        for strategy in "${strategy_array[@]}"; do
            [[ "$strategy" == "immediate" ]] && has_immediate=1
            [[ "$strategy" == "delayed-barrett" ]] && has_barrett=1
        done
        ordered_strategies=()
        if ((has_immediate == 1 && has_barrett == 1)); then
            if ((exponent_index % 2 == 0)); then
                ordered_strategies+=(immediate delayed-barrett)
            else
                ordered_strategies+=(delayed-barrett immediate)
            fi
            for strategy in "${strategy_array[@]}"; do
                if [[ "$strategy" != "immediate" && "$strategy" != "delayed-barrett" ]]; then
                    ordered_strategies+=("$strategy")
                fi
            done
        else
            ordered_strategies=("${strategy_array[@]}")
        fi
        position=1
        for strategy in "${ordered_strategies[@]}"; do
            run_case "$LATENCY_BENCH_BINARY" latency "$word_bits" "$exponent" "$strategy" "$position"
            position=$((position + 1))
        done
        exponent_index=$((exponent_index + 1))
    done
done

if [[ "$MEASURE_MEMORY" == "1" ]]; then
    for word_bits in $WORD_BITS; do
        for exponent in $EXPONENTS; do
            order=1
            for strategy in $MEMORY_STRATEGIES; do
                run_case "$MEMORY_BENCH_BINARY" memory "$word_bits" "$exponent" "$strategy" "$order"
                order=$((order + 1))
            done
        done
    done
fi

python3 "$SCRIPT_DIR/u32_mul_bench_report.py" \
    "$RAW_LOG" \
    --raw-csv "$RAW_CSV" \
    --summary-csv "$SUMMARY_CSV" \
    --paired-csv "$PAIRED_CSV" \
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
    --strategies "$STRATEGIES" \
    --memory-strategies "$MEMORY_STRATEGIES" \
    --measure-memory "$MEASURE_MEMORY"

echo "Saved raw samples: $RAW_CSV"
echo "Saved medians: $SUMMARY_CSV"
echo "Saved paired comparison: $PAIRED_CSV"
