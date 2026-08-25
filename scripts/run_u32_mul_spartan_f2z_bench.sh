#!/usr/bin/env bash
set -euo pipefail

# Run the production u32 × u32 → u64 Spartan/F2Z sweep and convert the
# benchmark's stable RESULT records into one row per circuit size.
#
# Usage:
#   scripts/run_u32_mul_spartan_f2z_bench.sh [output.csv]
#
# Useful overrides:
#   F2Z_MUL_EXPONENTS="15 16 17" F2Z_BENCH_REPS=5 \
#     scripts/run_u32_mul_spartan_f2z_bench.sh results.csv

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"

EXPONENTS="${F2Z_MUL_EXPONENTS:-15 16 17 18 19 20 21 22 23 24 25}"
REPETITIONS="${F2Z_BENCH_REPS:-5}"
FEATURES="${F2Z_BENCH_FEATURES:-unchecked}"
RUN_TIMESTAMP="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
RUN_DATE="${RUN_TIMESTAMP%%T*}"

DEFAULT_NAME="u32_mul_spartan_f2z_$(date -u +%Y%m%d-%H%M%S).csv"
OUTPUT_CSV="${1:-$REPO_ROOT/bench_results/$DEFAULT_NAME}"
case "$OUTPUT_CSV" in
    /*) ;;
    *) OUTPUT_CSV="$REPO_ROOT/$OUTPUT_CSV" ;;
esac

OUTPUT_DIR="$(dirname -- "$OUTPUT_CSV")"
RAW_LOG="${OUTPUT_CSV%.csv}.log"
mkdir -p -- "$OUTPUT_DIR"

echo "Running exponents: $EXPONENTS"
echo "Measured repetitions: $REPETITIONS (plus one warmup)"
echo "CSV: $OUTPUT_CSV"
echo "Raw log: $RAW_LOG"

(
    cd -- "$REPO_ROOT"
    OBLONG_PROFILE=1 \
    F2Z_MUL_EXPONENTS="$EXPONENTS" \
    F2Z_BENCH_REPS="$REPETITIONS" \
        cargo bench --bench u32_mul --features "$FEATURES"
) | tee "$RAW_LOG"

awk \
    -v output="$OUTPUT_CSV" \
    -v run_date="$RUN_DATE" \
    -v run_timestamp="$RUN_TIMESTAMP" \
    -v repetitions="$REPETITIONS" '
BEGIN {
    print "benchmark_date,run_timestamp_utc,exponent,multiplications,r1cs_rows,multiplications_per_row,r1cs_columns,r1cs_nnz,integer_witness_values,compact_witness_bits,compact_witness_bytes,spartan_prove_ms,bitify_prove_ms,f2z_prove_ms,combined_prove_ms,spartan_verify_ms,bitify_verify_ms,f2z_verify_ms,combined_verify_ms,spartan_proof_payload_bytes,f2z_proof_bytes,peak_heap_mib,measured_runs,warmup_runs,threads,field_modulus,ligerito_hash,ligerito_inverse_rate,ligerito_initial_k,shape_seed,notes" > output
}

/^rayon threads:/ {
    threads = $3
}

/^u32_mul gates=/ {
    shape_seed = ""
    for (i = 1; i <= NF; i++) {
        if ($i ~ /^seed=/) {
            split($i, seed_parts, "=")
            shape_seed = seed_parts[2]
        }
    }
}

/RESULT exponent=/ {
    for (key in field) {
        delete field[key]
    }
    for (i = 1; i <= NF; i++) {
        if ($i ~ /^[a-z0-9_]+=/) {
            split($i, pair, "=")
            field[pair[1]] = pair[2]
        }
    }

    multiplications = field["multiplications"] + 0
    if (threads == "") {
        threads = 1
    }

    printf "%s,%s,%d,%.0f,%.0f,1,%.0f,%.0f,%.0f,%.0f,%.0f,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%d,1,%s,2^100-15,SHA-256,1/2,4,%s,\n", \
        run_date, \
        run_timestamp, \
        field["exponent"], \
        multiplications, \
        multiplications, \
        4 * multiplications, \
        3 * multiplications, \
        field["integer_witness_values"], \
        field["compact_witness_bits"], \
        field["compact_witness_bytes"], \
        field["spartan_prove_ms"], \
        field["bitify_prove_ms"], \
        field["f2z_prove_ms"], \
        field["prove_ms"], \
        field["spartan_verify_ms"], \
        field["bitify_verify_ms"], \
        field["f2z_verify_ms"], \
        field["verify_ms"], \
        field["spartan_bytes"], \
        field["f2z_bytes"], \
        field["peak_mib"], \
        repetitions, \
        threads, \
        shape_seed >> output
    rows++
}

END {
    if (rows == 0) {
        print "No RESULT rows found in benchmark output" > "/dev/stderr"
        exit 2
    }
}
' "$RAW_LOG"

echo "Saved $OUTPUT_CSV"
