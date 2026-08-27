#!/usr/bin/env bash
set -euo pipefail

# Controlled inner-sumcheck sweep. The outer sumcheck is fixed to delayed
# Barrett. Six policies vary native witness folding and field coefficient
# accumulation, in cyclically counterbalanced order for exponents 15..23.

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"

EXPONENTS="${F2Z_MUL_EXPONENTS:-15 16 17 18 19 20 21 22 23}"
REPETITIONS="${F2Z_BENCH_REPS:-5}"
FEATURES="${F2Z_BENCH_FEATURES:-unchecked,bench-internals}"
THREADS="${RAYON_NUM_THREADS:-10}"
ROOT_SEED="${F2Z_MUL_SEED:-0x5533326d756c0064}"
RUN_TIMESTAMP="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
RUN_STAMP="$(date -u +%Y%m%d-%H%M%S)"

DEFAULT_SUMMARY="$REPO_ROOT/bench_results/u32_mul_inner_policy_${RUN_STAMP}_summary.csv"
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
OVERALL_CSV="${ARTIFACT_STEM}_overall.csv"
RAW_LOG="${ARTIFACT_STEM}.log"

for output in "$SUMMARY_CSV" "$RAW_CSV" "$PAIRED_CSV" "$OVERALL_CSV" "$RAW_LOG"; do
    if [[ -e "$output" ]]; then
        echo "refusing to overwrite existing artifact: $output" >&2
        exit 2
    fi
done
mkdir -p -- "$(dirname -- "$SUMMARY_CSV")"

COMMIT="$(git -C "$REPO_ROOT" rev-parse HEAD)"
if [[ -n "$(git -C "$REPO_ROOT" status --porcelain --untracked-files=all)" ]]; then
    GIT_DIRTY=true
else
    GIT_DIRTY=false
fi

BUILD_DIR="$(mktemp -d -t f2z-u32-inner-policy.XXXXXX)"
trap 'rm -rf -- "$BUILD_DIR"' EXIT
BUILD_LOG="$BUILD_DIR/build.log"

echo "Building one uninstrumented u32_mul_inner_policy binary" >&2
(
    cd -- "$REPO_ROOT"
    cargo bench --bench u32_mul_inner_policy --features "$FEATURES" --no-run 2>&1
) | tee "$BUILD_LOG" >&2

BENCH_BINARY="$(sed -n 's/^  Executable .* (\(.*u32_mul_inner_policy-[^)]*\))$/\1/p' "$BUILD_LOG" | tail -n 1)"
if [[ -z "$BENCH_BINARY" ]]; then
    echo "could not locate the u32_mul_inner_policy benchmark executable" >&2
    exit 2
fi
case "$BENCH_BINARY" in
    /*) ;;
    *) BENCH_BINARY="$REPO_ROOT/$BENCH_BINARY" ;;
esac
BINARY_SHA256="$(shasum -a 256 "$BENCH_BINARY" | awk '{print $1}')"
HARDWARE="$(system_profiler SPHardwareDataType 2>/dev/null | awk -F ': ' '/Chip:/{print $2; exit}' || true)"
if [[ -z "$HARDWARE" ]]; then
    HARDWARE="$(sysctl -n machdep.cpu.brand_string 2>/dev/null || uname -m)"
fi
OPERATING_SYSTEM="$(uname -srm)"

POLICY_FOLDS=(delayed immediate delayed immediate delayed immediate)
POLICY_FIELDS=(delayed delayed immediate immediate threshold:4096 threshold:4096)
POLICY_COUNT="${#POLICY_FOLDS[@]}"

echo "Exponents: $EXPONENTS" | tee "$RAW_LOG"
echo "Measured repetitions: $REPETITIONS (plus one warmup per cell)" | tee -a "$RAW_LOG"
echo "Rayon threads: $THREADS" | tee -a "$RAW_LOG"
echo "Root seed: $ROOT_SEED" | tee -a "$RAW_LOG"
echo "Commit: $COMMIT (dirty=$GIT_DIRTY)" | tee -a "$RAW_LOG"
echo "Binary SHA-256: $BINARY_SHA256" | tee -a "$RAW_LOG"

exponent_index=0
for exponent in $EXPONENTS; do
    position=1
    for ((offset = 0; offset < POLICY_COUNT; offset++)); do
        policy_index=$(((exponent_index + offset) % POLICY_COUNT))
        native_fold="${POLICY_FOLDS[$policy_index]}"
        field_accumulation="${POLICY_FIELDS[$policy_index]}"
        echo "RUN exponent=$exponent native_fold=$native_fold field_accumulation=$field_accumulation order=$position" | tee -a "$RAW_LOG"
        OBLONG_PROFILE=1 \
        RAYON_NUM_THREADS="$THREADS" \
        F2Z_MUL_EXPONENTS="$exponent" \
        F2Z_BENCH_REPS="$REPETITIONS" \
        F2Z_BENCH_ORDER="$position" \
        F2Z_MUL_SEED="$ROOT_SEED" \
        F2Z_INNER_NATIVE_FOLD="$native_fold" \
        F2Z_INNER_FIELD_ACCUM="$field_accumulation" \
            "$BENCH_BINARY" 2>&1 | tee -a "$RAW_LOG"
        position=$((position + 1))
    done
    exponent_index=$((exponent_index + 1))
done

python3 "$SCRIPT_DIR/u32_mul_inner_policy_report.py" \
    "$RAW_LOG" \
    --raw-csv "$RAW_CSV" \
    --summary-csv "$SUMMARY_CSV" \
    --paired-csv "$PAIRED_CSV" \
    --overall-csv "$OVERALL_CSV" \
    --metadata "run_timestamp=$RUN_TIMESTAMP" \
    --metadata "commit=$COMMIT" \
    --metadata "git_dirty=$GIT_DIRTY" \
    --metadata "binary_sha256=$BINARY_SHA256" \
    --metadata "hardware=$HARDWARE" \
    --metadata "operating_system=$OPERATING_SYSTEM" \
    --metadata "cargo_features=$FEATURES" \
    --metadata "threads=$THREADS" \
    --metadata "root_seed=$ROOT_SEED"

echo "Saved raw samples: $RAW_CSV"
echo "Saved medians: $SUMMARY_CSV"
echo "Saved paired comparisons: $PAIRED_CSV"
echo "Saved overall ranking: $OVERALL_CSV"
