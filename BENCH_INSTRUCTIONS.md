# Benchmark campaigns

This file is the single source of the benchmark instructions. `README.md`
carries a verbatim copy between its two `bench-campaigns` HTML-comment
markers, refreshed by `python3 scripts/sync_readme_campaigns.py` and checked
by `python3 -m unittest discover -s scripts -p test_readme_sync.py`; the
anonymised artifact workspace splices its own filtered copy (without the
Zinc+ campaign) into its README the same way. Edit here, then sync.

Run the following commands from the repository root in **Bash**. They execute
benchmarks and verify the generated proofs. Use a fresh `RUN_DIR` for each
campaign and run the workloads sequentially on an otherwise idle machine:
`scripts/bench_gate.py` serialises campaigns with a machine-wide lock, waits
for 88% CPU idle held for 120 s before starting, and aborts a campaign whose
swap growth passes its guard. Every scheme, including the competitors, is
built at fat LTO with one codegen unit and `-C target-cpu=native`; the
runners resolve the executables they measure from Cargo's
`--message-format=json` output, and out-of-crate workers carry their own
release profile with the same settings (verify a worker's effective flags in
its `--build-info` before trusting its numbers).

## Setup

```bash
set -euo pipefail

rustup toolchain install 1.98.1
rustup toolchain install nightly-2026-07-01
[ -f scripts/materialize_vendors.py ] && python3 scripts/materialize_vendors.py --check
bash scripts/install_trace_processor.sh

export RUSTFLAGS="-C target-cpu=native"
export PERFETTO_TRACE_PROCESSOR="$PWD/.tools/perfetto/trace_processor_shell"
unset BITZ_LIG_PROFILE CARGO_ENCODED_RUSTFLAGS CARGO_TARGET_DIR

mkdir -p bench_results
export RUN_DIR="$(mktemp -d "$PWD/bench_results/all-benchmarks-$(date +%Y%m%d-%H%M%S)-XXXXXX")"
# The artifact workspace vendors Limber; a development checkout points this at
# the Limber checkout prepared by scripts/prepare_matched_limber.py.
LIMBER_DIR="${LIMBER_DIR:-$PWD/vendor/limber}"
gate() { python3 scripts/bench_gate.py run --label "$1" --swap-grow-gb "${2:-12}" -- "${@:3}"; }
echo "Results: $RUN_DIR"
```

## 1. SHA-256 + ECDSA over secp256k1: BitZ (matched circuit), Binius64, Binius64-Ligerito

The head-to-head. BitZ runs the circuit matched to Binius64's stock verifier
schedule (`sha256-chain-secp256k1/bitz-binius64-matched/v1`); Binius64 and
the BitZ-opener rows run the pinned fork's upstream `ecdsa::bitcoin_verify`
(`sha256-chain-secp256k1/binius64-bitcoin-verify/v1`). Every row records its
`curve` and `circuit_profile`, and the runner refuses any other pairing. See
`docs/sha256-ecdsa-comparison.md` for what "matched" means and what differs.

```bash
gate sha-ecdsa-secp256k1 12 python3 scripts/run_sha256_ecdsa_compare.py \
  --curve secp256k1 \
  --output "$RUN_DIR/sha256-ecdsa-secp256k1" \
  --methods bitz-split binius64 binius64-ligerito \
  --exponents 4 5 6 7 \
  --targets 100 \
  --threads 1 10 \
  --reps 5 \
  --bitz-profiles custom:1:4 custom:3:4 \
  --binius-rates 1 3 \
  --timing perfetto \
  2>&1 | tee "$RUN_DIR/sha256-ecdsa-secp256k1.log"
```

## 2. SHA-256 + ECDSA over P-256: BitZ alone

The paper's P-256 circuit (`sha256-chain-p256/bitz-lean-port/v1`). The pinned
Binius64 fork has no P-256 verifier of its own (its P-256 gadget was written
for this comparison), so no Binius row exists on this curve and the runner
refuses to record one.

```bash
gate sha-ecdsa-p256 12 python3 scripts/run_sha256_ecdsa_compare.py \
  --curve p256 \
  --output "$RUN_DIR/sha256-ecdsa-p256" \
  --methods bitz-split \
  --exponents 4 5 6 7 \
  --targets 100 \
  --threads 1 10 \
  --reps 5 \
  --bitz-profiles custom:1:4 custom:3:4 \
  --timing perfetto \
  2>&1 | tee "$RUN_DIR/sha256-ecdsa-p256.log"
```

## 3. SHA-256 chains: BitZ, Binius64, Binius64-Ligerito

```bash
gate sha256-chain 12 python3 scripts/run_sha256_chain_compare.py \
  --methods bitz binius64 binius64-ligerito \
  --exponents 7 8 9 10 11 12 13 14 15 16 \
  --threads 1 10 \
  --reps 5 \
  --bitz-profiles custom:1:4 custom:3:4 \
  --binius-rates 1 3 \
  --output "$RUN_DIR/sha256-chain" \
  2>&1 | tee "$RUN_DIR/sha256-chain.log"
```

## 4. Multiplication comparisons: BitZ, Binius64, Binius64-Ligerito, Plonky3-FRI, Limber

The launcher builds once, validates every selection with the Rust case
planner, runs one worker process per case, records a separate single-proof
peak-RSS trial (`--memory rss`), and generates combined reports under
`<output>/reports` (schema `mul-bench/v2`). Plonky3-FRI proves u32-mod32
through its wrapping AIR and u64/u128 through a full-product AIR over 16-bit
limbs; Plonky3-WHIR remains a u32-mod32 direct experiment. Run one gated
invocation per workload; the launcher gates each campaign itself, so omit
`--no-gate` unless an outer gate already holds the lock. Cells whose prover
exceeds the machine's memory are excluded, not measured while paging:
`scripts/mul_memory_probe.py` runs one such cell alone under a small
swap-growth guard and appends a record with the verdict, the observed peak
and the installed memory to a JSONL file that the table generator reads (see
"Tables and figures"), for example

```bash
python3 scripts/mul_memory_probe.py --workload u128 --backend binius64 --log-n 21 \
  --threads 10 --log-inv-rate 1 --record "$RUN_DIR/mul-exclusions.jsonl" \
  --output "$RUN_DIR/multiplication-probes/u128-binius64-rate2-t10-n21"
```

```bash
for workload in u32-mod32 u64 u128; do
  python3 scripts/run_multiplication_benchmarks.py compare \
    --output "$RUN_DIR/multiplication-$workload" -- \
    proof --workload "$workload" --backends bitz,binius64,binius64-ligerito,plonky3-fri,limber \
    --log-n 15,17,19 --threads 1,10 --reps 5 --memory rss --skip-unsupported \
    --binius-ligerito-accounting rbr \
    2>&1 | tee "$RUN_DIR/multiplication-$workload.log"
done
```

Sizes above `2^19` are a per-backend decision on a 24 GiB machine: BitZ,
Binius64 at rate 1/2 and Plonky3-FRI u32 reach `2^21`; BitZ reaches `2^23`
for u32 and u64; the others page first. Add `--log-n 21` (or `23`) runs for
the backends that fit and record the rest as exclusions.

## 5. Multiplication: the Zinc+ rows

Zinc+ cannot be linked into this crate (it pins `crypto-bigint = 0.7.0-rc.9`
against `vendor/field`'s 0.7.5), so its rows come from an external worker
built inside a pinned zinc-plus checkout. The runner exports the exact BitZ
corpora with `examples/mul_corpus_export`, builds a single-threaded and a
`parallel` worker (features `simd,unchecked[,parallel]`, rate 1/4, 100 bits;
the first build also runs once in CHECKED mode at the smallest size), runs
one process per case under `/usr/bin/time -l`, and imports the logs into a
`mul-bench/v2` campaign whose rows the table treats like every other
backend's. Zinc+ reports no security accounting of its own; the imported
configuration records the inverse rate, column openings, projecting-prime
width, grinding bits and the LogUp range-check term per case.

```bash
cargo +1.98.1 build --release --locked --example mul_corpus_export
gate zinc-plus 12 python3 scripts/run_zinc_plus_campaign.py \
  --output "$RUN_DIR/zinc-plus" \
  --workdir "$RUN_DIR/zinc-plus-checkout" \
  --revision 878fbd8292472dcb13b25e2c9c0209406b5fb671 \
  --corpus-exporter target/release/examples/mul_corpus_export \
  --workloads u32-mod32 u64 u128 --exponents 15 17 19 --threads 1 10 --reps 5 \
  2>&1 | tee "$RUN_DIR/zinc-plus.log"
```

## 6. BitZ full-product u32 × u32 → u64, with component breakdown

```bash
cargo +1.98.1 run --release --locked --bin bitz \
  --features unchecked,span-metrics -- \
  --mul-sweep 15-22 \
  --threads 10 \
  --reps 5 \
  --profile custom:1:4 \
  --cooldown 20 \
  --latex "$RUN_DIR/u32-full-product.tex" \
  2>&1 | tee "$RUN_DIR/u32-full-product.log"
```

## 7. MultiSwap: BitZ, Limber-Hyrax, Limber-Brakedown

This uses the Limber checkout in `LIMBER_DIR`. `MSCFG=paper` is a workload
name and does not require a manuscript directory. `--draft` runs proofs and
the local comparison checks while marking canonical trace validation as
pending.

```bash
gate multiswap 10 python3 scripts/run_matched_multiswap_campaign.py \
  --draft \
  --limber-root "$LIMBER_DIR" \
  --security-bits 114 \
  --batch-counts 1,2,4,8,16 \
  --all-threads 10 \
  --warmups 1 \
  --samples 10 \
  --rustflags="-C target-cpu=native" \
  --output-dir "$RUN_DIR/multiswap" \
  2>&1 | tee "$RUN_DIR/multiswap.log"
```

## 8. SHA-256 layout parameter sweep over s and t

The `sha256_product_layout` benchmark holds the workload at `2^14` SHA-256
compressions and sweeps the layout split with `s + t = 29`. By default, it runs
`t = 7..27` (`s = 29 - t`), with one warmup and 21 measured samples per split.
The `t = 28` case is skipped because its projected peak memory exceeds 60 GiB.
This is a controlled fixed-prime layout experiment.

```bash
RUSTFLAGS="-C target-cpu=native" \
RAYON_NUM_THREADS=10 \
cargo +1.98.1 bench --locked \
  --bench sha256_product_layout \
  --features unchecked,span-metrics,bench-internals
```

To select particular splits and change the sample count, prepend
`BITZ_SHA_PRODUCT_TS="13 17" BITZ_BENCH_REPS=5` to the command. This selects
`(t, s) = (13, 16)` and `(17, 12)`, with five measured samples per split.
Add `--no-run` to the Cargo command to compile without executing the sweep.

## 9. Hybrid SHA-256 chain + multiplication modulo 2^32

This is the paper's **"Modular multiplications and bit operations"** experiment
(table label `tab:hybrid-sha256-mul`). It proves `N` relations
`x*y = z + 2^32*w`, with four u32 limbs, together with `M = N/256` chained SHA-256
compressions. The two branches have equal packed witness sizes; their witness
values are independent. Shape `15:7`, for example, means `2^15` multiplications
and `2^7` compressions.

| CLI mode | Multiplication and SHA proof |
|---|---|
| `hybrid` | BitZ multiplication PIOP + Binius64 SHA PIOP, with one shared BitZ opening |
| `all-binius` | Both relations in Binius64, using BaseFold/FRI (paper: Binius UDR) |
| `binius-ligerito` | Both relations in Binius64, using the BitZ/Ligerito opener (paper: Binius Johnson) |

The sweep below runs all three modes at rates 1/2 and 1/8, with 1 and 10
threads, one warmup and five measured iterations per shape. The BitZ hybrid
checks a 100-bit whole-protocol union bound; the Binius/Ligerito mode uses
100-bit round-by-round accounting. The BaseFold query target is explicitly
set to 100 below, overriding the hybrid CLI's default of 112.

Build the benchmark once and obtain its executable path from Cargo's artifact
record, then run each configuration in a separate sweep. On macOS, the sampler
also records per-case peak RSS and swap-outs for the table's memory column.

```bash
cargo +1.98.1 bench --locked --no-run \
  --bench hybrid_u32_sha256 --features hybrid --message-format=json \
  > "$RUN_DIR/hybrid-build.jsonl"

HYBRID_BIN="$(python3 - "$RUN_DIR/hybrid-build.jsonl" <<'PY'
import json
import sys
with open(sys.argv[1]) as stream:
    artifacts = [json.loads(line) for line in stream]
executables = [entry["executable"] for entry in artifacts
               if entry.get("reason") == "compiler-artifact"
               and entry.get("target", {}).get("name") == "hybrid_u32_sha256"
               and entry.get("executable")]
if len(executables) != 1:
    raise SystemExit("expected exactly one hybrid benchmark executable")
print(executables[0])
PY
)"

HYBRID_SHAPES="15:7,16:8,17:9,18:10,19:11,20:12"
HYBRID_ROOT="$RUN_DIR/hybrid-witness"
mkdir -p "$HYBRID_ROOT"

hybrid_sweep() {
  local mode="$1" rate="$2" threads="$3"
  local dir="$HYBRID_ROOT/$mode-rate$rate-t$threads"
  local tsv="$dir-peak-rss-and-swap.tsv"
  local command=("$HYBRID_BIN" --sweep --mode "$mode"
    --shapes "$HYBRID_SHAPES" --iterations 5 --results-dir "$dir")
  if [[ "$mode" == hybrid ]]; then
    command+=(--profile "custom:$rate:4")
  fi
  if [[ "$(uname -s)" == Darwin ]]; then
    command=(python3 scripts/rss_sampler.py --output "$tsv" -- "${command[@]}")
  fi
  env RAYON_NUM_THREADS="$threads" \
    BITZ_HYBRID_BINIUS_LOG_INV_RATE="$rate" \
    BITZ_HYBRID_BINIUS_SECURITY_BITS=100 \
    BITZ_BINIUS_LOG_INV_RATE="$rate" \
    BITZ_BINIUS_LIGERITO_ACCOUNTING=rbr \
    "${command[@]}" 2>&1 | tee "$dir.log"
  if [[ -f "$tsv" ]]; then
    mv "$tsv" "$dir/peak-rss-and-swap.tsv"
  fi
}

for threads in 1 10; do
  for rate in 1 3; do
    for mode in hybrid all-binius binius-ligerito; do
      hybrid_sweep "$mode" "$rate" "$threads"
    done
  done
done

echo "Completed. Results: $RUN_DIR"
```

For the optional **equal-operation-count** experiment (`N = M`), set the
following variables, then repeat the three nested `for` loops above:

```bash
HYBRID_SHAPES="9:9,10:10,11:11,12:12,13:13,14:14"
HYBRID_ROOT="$RUN_DIR/hybrid-counts"
mkdir -p "$HYBRID_ROOT"
```

This is a separate workload from the paper's equal-witness table. Each sweep's
result directory must not already exist. It will contain `summary.csv`,
`run.txt`, and per-case CSV/log files.

## Tables and figures

Every table is generated from recorded campaign output; none is typed by
hand, and each carries in its header comments the generator, the run
directories, the machine, and the per-row medians needed to regenerate it.

```bash
# SHA-256 + ECDSA: one table per curve (mixing curves is refused).
python3 scripts/sha256_ecdsa_table.py "$RUN_DIR/sha256-ecdsa-secp256k1" \
  --out "$RUN_DIR/sha256-ecdsa-secp256k1-table.tex"
python3 scripts/sha256_ecdsa_table.py "$RUN_DIR/sha256-ecdsa-p256" \
  --out "$RUN_DIR/sha256-ecdsa-p256-table.tex"

# Multiplication: one table per workload from every campaign that measured it
# (the launcher's directories and the imported Zinc+ campaign). Excluded cells
# come from the memory probe's JSONL records (or a JSON list) with their
# reason, observed peak and machine memory; the file may be empty.
touch "$RUN_DIR/mul-exclusions.jsonl"
for workload in u32-mod32 u64 u128; do
  python3 scripts/mul_table.py "$RUN_DIR/multiplication-$workload" "$RUN_DIR/zinc-plus/campaign" \
    --workload "$workload" --exclusions "$RUN_DIR/mul-exclusions.jsonl" \
    --out "$RUN_DIR/native-mul-$workload-table.tex"
done

# Hybrid witness sweeps.
hybrid_rows=()
for threads in 1 10; do
  for rate in 1 3; do
    for mode in hybrid all-binius binius-ligerito; do
      hybrid_rows+=(--row "$mode@$rate:$threads=$RUN_DIR/hybrid-witness/$mode-rate$rate-t$threads")
    done
  done
done
python3 scripts/hybrid_table.py --variant witness "${hybrid_rows[@]}" \
  --output "$RUN_DIR/hybrid-witness.tex"
```

For equal-count results, use `--variant counts` and the `hybrid-counts`
directories instead. Generation of a table does not rerun the proofs. The
bundled `scripts/zk_trace.py` (artifact workspace) provides trace report
tooling; tables default to `outputs/tables/` and figures to `outputs/figures/`
there.
