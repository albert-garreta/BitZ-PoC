#!/usr/bin/env bash
set -euo pipefail

# Run the BabyBear multiplication Spartan/F2Z sweep with a paired immediate-versus-
# delayed design. One uninstrumented latency executable and, when requested,
# one peak-allocator executable are built, then invoked in fresh processes for
# every (size, strategy, pass) tuple.
#
# Usage:
#   scripts/run_baby_bear_mul_spartan_f2z_bench.sh [summary.csv]
#
# The sibling artifacts are `<stem>_raw.csv`, `<stem>_paired.csv`, and
# `<stem>.log`. Existing files are never overwritten.

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"

EXPONENTS="${F2Z_BABY_BEAR_MUL_EXPONENTS:-15 16 17 18 19 20 21 22 23 24 25}"
EXPONENTS="${EXPONENTS//,/ }"
REPETITIONS="${F2Z_BENCH_REPS:-5}"
FEATURES="${F2Z_BENCH_FEATURES:-unchecked}"
FEATURES="${FEATURES//,/ }"
THREADS="${RAYON_NUM_THREADS:-10}"
STRATEGIES="${F2Z_BENCH_STRATEGIES:-immediate delayed-barrett delayed-crypto-bigint}"
MEMORY_STRATEGIES="${F2Z_BENCH_MEMORY_STRATEGIES:-$STRATEGIES}"
STRATEGIES="${STRATEGIES//,/ }"
MEMORY_STRATEGIES="${MEMORY_STRATEGIES//,/ }"
MEASURE_MEMORY="${F2Z_BENCH_MEASURE_MEMORY:-1}"
ROOT_SEED="${F2Z_BABY_BEAR_MUL_SEED:-0x42424d554c000064}"
RUN_TIMESTAMP="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
RUN_DATE="${RUN_TIMESTAMP%%T*}"
RUN_STAMP="$(date -u +%Y%m%d-%H%M%S)"

read -r -a requested_exponents <<< "$EXPONENTS"
if (( ${#requested_exponents[@]} == 0 )); then
    echo "F2Z_BABY_BEAR_MUL_EXPONENTS must name at least one exponent" >&2
    exit 2
fi
validated_exponents=()
for exponent in "${requested_exponents[@]}"; do
    if [[ ! "$exponent" =~ ^[0-9]+$ ]]; then
        echo "invalid BabyBear benchmark exponent: $exponent (expected an integer >= 15)" >&2
        exit 2
    fi
    exponent_value=$((10#$exponent))
    if ((exponent_value < 15)); then
        echo "invalid BabyBear benchmark exponent: $exponent (expected an integer >= 15)" >&2
        exit 2
    fi
    if (( ${#validated_exponents[@]} != 0 )); then
        for seen in "${validated_exponents[@]}"; do
            if [[ "$exponent_value" == "$seen" ]]; then
                echo "duplicate BabyBear benchmark exponent: $exponent" >&2
                exit 2
            fi
        done
    fi
    validated_exponents+=("$exponent_value")
done
EXPONENTS="${validated_exponents[*]}"
if [[ ! "$REPETITIONS" =~ ^[1-9][0-9]*$ ]]; then
    echo "F2Z_BENCH_REPS must be a positive decimal integer" >&2
    exit 2
fi
if [[ ! "$THREADS" =~ ^[1-9][0-9]*$ ]]; then
    echo "RAYON_NUM_THREADS must be a positive decimal integer" >&2
    exit 2
fi

read -r -a requested_features <<< "$FEATURES"
if (( ${#requested_features[@]} == 0 )); then
    echo "F2Z_BENCH_FEATURES must name at least one non-reserved feature" >&2
    exit 2
fi
validated_features=()
for feature in "${requested_features[@]}"; do
    case "$feature" in
        bench-peak-memory|*/bench-peak-memory)
            echo "F2Z_BENCH_FEATURES must not enable reserved bench-peak-memory; the runner enables it only in the memory binary" >&2
            exit 2
            ;;
    esac
    if (( ${#validated_features[@]} != 0 )); then
        for seen in "${validated_features[@]}"; do
            if [[ "$feature" == "$seen" ]]; then
                echo "duplicate requested Cargo feature: $feature" >&2
                exit 2
            fi
        done
    fi
    validated_features+=("$feature")
done
FEATURES="${validated_features[*]}"
if [[ " $FEATURES " != *" span-metrics "* ]]; then
    FEATURES="$FEATURES span-metrics"
fi

read -r -a requested_strategies <<< "$STRATEGIES"
if (( ${#requested_strategies[@]} == 0 )); then
    echo "F2Z_BENCH_STRATEGIES must name at least one strategy" >&2
    exit 2
fi
validated_strategies=()
for strategy in "${requested_strategies[@]}"; do
    case "$strategy" in
        immediate|delayed-barrett|delayed-crypto-bigint) ;;
        *) echo "unsupported benchmark strategy: $strategy" >&2; exit 2 ;;
    esac
    if (( ${#validated_strategies[@]} != 0 )); then
        for seen in "${validated_strategies[@]}"; do
            if [[ "$strategy" == "$seen" ]]; then
                echo "duplicate benchmark strategy: $strategy" >&2
                exit 2
            fi
        done
    fi
    validated_strategies+=("$strategy")
done
STRATEGIES="${validated_strategies[*]}"

read -r -a requested_memory_strategies <<< "$MEMORY_STRATEGIES"
validated_memory_strategies=()
if (( ${#requested_memory_strategies[@]} != 0 )); then
    for strategy in "${requested_memory_strategies[@]}"; do
        found=0
        for allowed in "${validated_strategies[@]}"; do
            [[ "$strategy" == "$allowed" ]] && found=1
        done
        if ((found == 0)); then
            echo "memory strategy is not in the latency sweep: $strategy" >&2
            exit 2
        fi
        if (( ${#validated_memory_strategies[@]} != 0 )); then
            for seen in "${validated_memory_strategies[@]}"; do
                if [[ "$strategy" == "$seen" ]]; then
                    echo "duplicate memory strategy: $strategy" >&2
                    exit 2
                fi
            done
        fi
        validated_memory_strategies+=("$strategy")
    done
fi
MEMORY_STRATEGIES="${validated_memory_strategies[*]-}"
case "$MEASURE_MEMORY" in
    0) ;;
    1)
        if (( ${#validated_memory_strategies[@]} == 0 )); then
            echo "memory measurement requires at least one memory strategy" >&2
            exit 2
        fi
        ;;
    *) echo "F2Z_BENCH_MEASURE_MEMORY must be 0 or 1" >&2; exit 2 ;;
esac

DEFAULT_SUMMARY="$REPO_ROOT/bench_results/baby_bear_mul_spartan_f2z_${RUN_STAMP}_summary.csv"
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

# Snapshot repository metadata before creating default artifacts under the
# worktree; otherwise the raw log itself would make a clean checkout dirty.
COMMIT="$(git -C "$REPO_ROOT" rev-parse HEAD)"
if [[ -n "$(git -C "$REPO_ROOT" status --porcelain --untracked-files=all)" ]]; then
    GIT_DIRTY=true
else
    GIT_DIRTY=false
fi

BUILD_DIR="$(mktemp -d -t f2z-baby-bear-mul-build.XXXXXX)"
trap 'rm -rf -- "$BUILD_DIR"' EXIT

CARGO_METADATA_JSON="$BUILD_DIR/cargo-metadata.json"
if ! (
    cd -- "$REPO_ROOT"
    cargo metadata --offline --locked --no-deps --format-version 1 \
        --manifest-path "$REPO_ROOT/Cargo.toml" > "$CARGO_METADATA_JSON"
); then
    echo "could not read local Cargo feature metadata" >&2
    exit 2
fi

resolve_cargo_features() {
    local requested="$1"
    python3 - "$CARGO_METADATA_JSON" "$REPO_ROOT/Cargo.toml" "$requested" <<'PY'
import json
import re
import sys
from pathlib import Path

metadata_path, manifest_path, requested_text = sys.argv[1:]
with open(metadata_path, encoding="utf-8") as source:
    metadata = json.load(source)

manifest = Path(manifest_path).resolve()
packages = [
    package
    for package in metadata.get("packages", [])
    if Path(package["manifest_path"]).resolve() == manifest
]
if len(packages) != 1:
    raise SystemExit(
        f"expected one Cargo package for {manifest}, found {len(packages)}"
    )

feature_graph = packages[0].get("features", {})
requested = [part for part in re.split(r"[\s,]+", requested_text) if part]
pending = ["default", *requested]
enabled = set()
while pending:
    feature = pending.pop()
    if feature in enabled:
        continue
    if feature not in feature_graph:
        raise SystemExit(f"unknown root-package Cargo feature: {feature}")
    enabled.add(feature)
    for activation in feature_graph[feature]:
        if activation in feature_graph:
            pending.append(activation)

print(",".join(sorted(enabled)))
PY
}

LATENCY_RESOLVED_FEATURES="$(resolve_cargo_features "$FEATURES")"
case ",$LATENCY_RESOLVED_FEATURES," in
    *,bench-peak-memory,*)
        echo "requested Cargo feature closure enables reserved bench-peak-memory in the latency binary" >&2
        exit 2
        ;;
esac

MEMORY_FEATURES=""
MEMORY_RESOLVED_FEATURES=""
if [[ "$MEASURE_MEMORY" == "1" ]]; then
    MEMORY_FEATURES="$FEATURES bench-peak-memory"
    MEMORY_RESOLVED_FEATURES="$(resolve_cargo_features "$MEMORY_FEATURES")"
fi

detect_physical_memory_bytes() {
    local value=""
    local profile_memory=""
    local mem_kib=""
    local pages=""
    local page_size=""

    if command -v sysctl >/dev/null 2>&1; then
        value="$(sysctl -n hw.memsize 2>/dev/null || true)"
    fi
    if [[ ! "$value" =~ ^[1-9][0-9]*$ ]] && [[ -n "$SYSTEM_HARDWARE_PROFILE" ]]; then
        profile_memory="$(printf '%s\n' "$SYSTEM_HARDWARE_PROFILE" | awk -F ': ' '/^[[:space:]]*Memory:/{print $2; exit}')"
        if [[ -n "$profile_memory" ]]; then
            value="$(python3 - "$profile_memory" <<'PY' || true
import re
import sys
from decimal import Decimal

match = re.fullmatch(
    r"\s*([0-9]+(?:\.[0-9]+)?)\s*([KMGTPE])(?:I?B)?\s*",
    sys.argv[1],
    flags=re.IGNORECASE,
)
if match is None:
    raise SystemExit(1)
power = "KMGTPE".index(match.group(2).upper()) + 1
value = Decimal(match.group(1)) * (1 << (10 * power))
if value != value.to_integral_value():
    raise SystemExit(1)
print(int(value))
PY
)"
        fi
    fi
    if [[ ! "$value" =~ ^[1-9][0-9]*$ ]] && [[ -r /proc/meminfo ]]; then
        mem_kib="$(awk '/^MemTotal:/{print $2; exit}' /proc/meminfo)"
        if [[ "$mem_kib" =~ ^[1-9][0-9]*$ ]]; then
            value="$(awk -v kib="$mem_kib" 'BEGIN { printf "%.0f", kib * 1024 }')"
        fi
    fi
    if [[ ! "$value" =~ ^[1-9][0-9]*$ ]] && command -v getconf >/dev/null 2>&1; then
        pages="$(getconf _PHYS_PAGES 2>/dev/null || true)"
        page_size="$(getconf PAGE_SIZE 2>/dev/null || true)"
        if [[ "$pages" =~ ^[1-9][0-9]*$ ]] && [[ "$page_size" =~ ^[1-9][0-9]*$ ]]; then
            value="$(awk -v pages="$pages" -v page_size="$page_size" 'BEGIN { printf "%.0f", pages * page_size }')"
        fi
    fi
    printf '%s\n' "$value"
}

SYSTEM_HARDWARE_PROFILE=""
if command -v system_profiler >/dev/null 2>&1; then
    SYSTEM_HARDWARE_PROFILE="$(system_profiler SPHardwareDataType 2>/dev/null || true)"
fi
PHYSICAL_MEMORY_BYTES="$(detect_physical_memory_bytes)"
if [[ ! "$PHYSICAL_MEMORY_BYTES" =~ ^[1-9][0-9]*$ ]]; then
    echo "could not determine positive host physical-memory bytes" >&2
    exit 2
fi
PHYSICAL_MEMORY_GIB="$(python3 - "$PHYSICAL_MEMORY_BYTES" <<'PY'
import sys

value = int(sys.argv[1])
if not 0 < value <= (1 << 64) - 1:
    raise SystemExit("physical-memory byte count is not a positive u64")
print(f"{value / (1 << 30):.6f}")
PY
)"
if [[ " $EXPONENTS " == *" 25 "* ]]; then
    if ! python3 - "$PHYSICAL_MEMORY_BYTES" <<'PY'
import sys

raise SystemExit(0 if int(sys.argv[1]) >= 128 * (1 << 30) else 1)
PY
    then
        echo "refusing exponent 25: detected $PHYSICAL_MEMORY_BYTES physical-memory bytes ($PHYSICAL_MEMORY_GIB GiB), but at least 128 GiB is required" >&2
        exit 2
    fi
fi

mkdir -p -- "$OUTPUT_DIR"

# Atomically reserve the raw log before any build starts. The preflight check
# above gives a useful error in the common case; noclobber closes the race
# between that check and creation by requesting O_EXCL.
if ! (set -o noclobber; : > "$RAW_LOG") 2>/dev/null; then
    echo "refusing to overwrite existing artifact: $RAW_LOG" >&2
    exit 2
fi

build_benchmark() {
    local label="$1"
    local features="$2"
    local build_log="$BUILD_DIR/$label.log"
    echo "Building $label baby_bear_mul binary with features: $features" >&2
    (
        cd -- "$REPO_ROOT"
        cargo bench --bench baby_bear_mul --features "$features" --no-run 2>&1
    ) | tee "$build_log" >&2

    local binary
    binary="$(sed -n 's/^  Executable .* (\(.*baby_bear_mul-[^)]*\))$/\1/p' "$build_log" | tail -n 1)"
    if [[ -z "$binary" ]]; then
        echo "could not locate the $label baby_bear_mul benchmark executable" >&2
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
    MEMORY_BENCH_BINARY="$(build_benchmark memory "$MEMORY_FEATURES")"
    MEMORY_BINARY_SHA256="$(shasum -a 256 "$MEMORY_BENCH_BINARY" | awk '{print $1}')"
elif [[ "$MEASURE_MEMORY" != "0" ]]; then
    echo "F2Z_BENCH_MEASURE_MEMORY must be 0 or 1" >&2
    exit 2
fi
HARDWARE="$(printf '%s\n' "$SYSTEM_HARDWARE_PROFILE" | awk -F ': ' '/Chip:/{print $2; exit}' || true)"
if [[ -z "$HARDWARE" ]]; then
    HARDWARE="$(sysctl -n machdep.cpu.brand_string 2>/dev/null || uname -m)"
fi
OPERATING_SYSTEM="$(uname -srm)"

{
    echo "Exponents: $EXPONENTS"
    echo "Strategies: $STRATEGIES"
    echo "Measured repetitions: $REPETITIONS (plus one warmup per process)"
    echo "Rayon threads: $THREADS"
    echo "Root seed: $ROOT_SEED"
    echo "Commit: $COMMIT (dirty=$GIT_DIRTY)"
    echo "Requested Cargo features: $FEATURES"
    echo "Resolved latency Cargo features: $LATENCY_RESOLVED_FEATURES"
    if [[ -n "$MEMORY_RESOLVED_FEATURES" ]]; then
        echo "Resolved memory Cargo features: $MEMORY_RESOLVED_FEATURES"
    else
        echo "Resolved memory Cargo features: (memory binary not built)"
    fi
    echo "Latency binary SHA-256: $LATENCY_BINARY_SHA256"
    if [[ -n "$MEMORY_BINARY_SHA256" ]]; then
        echo "Memory binary SHA-256: $MEMORY_BINARY_SHA256"
    fi
    echo "Hardware: $HARDWARE"
    echo "Operating system: $OPERATING_SYSTEM"
    echo "Physical memory: $PHYSICAL_MEMORY_BYTES bytes ($PHYSICAL_MEMORY_GIB GiB)"
    if [[ -n "$MEMORY_BINARY_SHA256" ]]; then
        echo "Allocator instrumentation: absent from latency binary; enabled only in memory binary"
    else
        echo "Allocator instrumentation: absent from latency binary; memory binary not built"
    fi
    echo "Raw log: $RAW_LOG"
    if [[ " $EXPONENTS " == *" 25 "* ]]; then
        echo "Capacity note: exponent 25 requires at least 128 GiB physical memory; detected $PHYSICAL_MEMORY_GIB GiB."
    fi
} | tee -a "$RAW_LOG"

run_case() {
    local binary="$1"
    local pass="$2"
    local exponent="$3"
    local strategy="$4"
    local order="$5"
    echo "RUN pass=$pass exponent=$exponent strategy=$strategy order=$order" | tee -a "$RAW_LOG"
    OBLONG_PROFILE=1 \
    RAYON_NUM_THREADS="$THREADS" \
    F2Z_BABY_BEAR_MUL_EXPONENTS="$exponent" \
    F2Z_BENCH_REPS="$REPETITIONS" \
    F2Z_BENCH_PASS="$pass" \
    F2Z_BENCH_ORDER="$order" \
    F2Z_BABY_BEAR_MUL_SEED="$ROOT_SEED" \
    F2Z_SPARTAN_REDUCTION="$strategy" \
        "$binary" 2>&1 | tee -a "$RAW_LOG"
}

# Pairwise-counterbalance the Immediate/Barrett comparison by alternating
# which strategy runs first at each exponent. Crypto-Bigint is scheduled after
# that pair as the reference reduction strategy.
exponent_index=0
for exponent in $EXPONENTS; do
    has_immediate=0
    has_barrett=0
    has_crypto_bigint=0
    for strategy in "${validated_strategies[@]}"; do
        [[ "$strategy" == "immediate" ]] && has_immediate=1
        [[ "$strategy" == "delayed-barrett" ]] && has_barrett=1
        [[ "$strategy" == "delayed-crypto-bigint" ]] && has_crypto_bigint=1
    done
    ordered_strategies=()
    if ((has_immediate == 1 && has_barrett == 1)); then
        if ((exponent_index % 2 == 0)); then
            ordered_strategies+=(immediate delayed-barrett)
        else
            ordered_strategies+=(delayed-barrett immediate)
        fi
    else
        ((has_immediate == 1)) && ordered_strategies+=(immediate)
        ((has_barrett == 1)) && ordered_strategies+=(delayed-barrett)
    fi
    # Crypto-Bigint is a reference backend and always runs after the primary
    # Immediate/Barrett pair, regardless of caller-provided list order.
    ((has_crypto_bigint == 1)) && ordered_strategies+=(delayed-crypto-bigint)
    position=1
    for strategy in "${ordered_strategies[@]}"; do
        run_case "$LATENCY_BENCH_BINARY" latency "$exponent" "$strategy" "$position"
        position=$((position + 1))
    done
    exponent_index=$((exponent_index + 1))
done

if [[ "$MEASURE_MEMORY" == "1" ]]; then
    memory_exponent_index=0
    for exponent in $EXPONENTS; do
        has_immediate=0
        has_barrett=0
        has_crypto_bigint=0
        for strategy in "${validated_memory_strategies[@]}"; do
            [[ "$strategy" == "immediate" ]] && has_immediate=1
            [[ "$strategy" == "delayed-barrett" ]] && has_barrett=1
            [[ "$strategy" == "delayed-crypto-bigint" ]] && has_crypto_bigint=1
        done
        ordered_memory_strategies=()
        if ((has_immediate == 1 && has_barrett == 1)); then
            if ((memory_exponent_index % 2 == 0)); then
                ordered_memory_strategies+=(immediate delayed-barrett)
            else
                ordered_memory_strategies+=(delayed-barrett immediate)
            fi
        else
            ((has_immediate == 1)) && ordered_memory_strategies+=(immediate)
            ((has_barrett == 1)) && ordered_memory_strategies+=(delayed-barrett)
        fi
        ((has_crypto_bigint == 1)) && ordered_memory_strategies+=(delayed-crypto-bigint)
        order=1
        for strategy in "${ordered_memory_strategies[@]}"; do
            run_case "$MEMORY_BENCH_BINARY" memory "$exponent" "$strategy" "$order"
            order=$((order + 1))
        done
        memory_exponent_index=$((memory_exponent_index + 1))
    done
fi

python3 "$SCRIPT_DIR/baby_bear_mul_bench_report.py" \
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
    --cargo-features-requested "$FEATURES" \
    --latency-cargo-features-resolved "$LATENCY_RESOLVED_FEATURES" \
    --memory-cargo-features-resolved "$MEMORY_RESOLVED_FEATURES" \
    --physical-memory-bytes "$PHYSICAL_MEMORY_BYTES" \
    --physical-memory-gib "$PHYSICAL_MEMORY_GIB" \
    --root-seed "$ROOT_SEED" \
    --threads "$THREADS" \
    --measured-runs "$REPETITIONS" \
    --expected-exponents "$EXPONENTS" \
    --strategies "$STRATEGIES" \
    --memory-strategies "$MEMORY_STRATEGIES" \
    --measure-memory "$MEASURE_MEMORY"

echo "Saved raw samples: $RAW_CSV"
echo "Saved medians: $SUMMARY_CSV"
echo "Saved paired comparison: $PAIRED_CSV"
