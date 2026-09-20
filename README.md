
# BitZ 🫜 --- README for normal humans

The Rust crate and CLI are named `bitz`. Benchmark commands use `bitz`,
`bitz-split`, or `bitz-all`, with `--bitz-profiles` and `BITZ_*` environment
variables. Existing checkout paths and upstream repository names still work.
Hash domains, proof codec headers, and result schemas also use the BitZ
namespace. Regenerate proofs and benchmark fixtures made with the old namespace;
start a new results directory for renamed campaigns.

Production 100-bit BitZ paths now default to **Ligerito Johnson `custom:1:4`
(rate 1/2, initial_k 4) with early OOD**. Use `BITZ_LIG_PROFILE=udrg:1:4` (or the applicable CLI
`--profile udrg:1:4`) for matched-geometry UDR. This selector applies only
to BitZ/Ligerito; it does not change Binius64, Plonky3-FRI/WHIR, or Limber.
Higher-security profiles retain their previous defaults. See the
[entrypoint inventory, supported shapes, result versions, and validation commands](docs/ligerito-coverage.md).
Paired benchmark measurements are deferred.


The core BitZ PCS proves

```
MLE[w](r) = y ∈ F_q
```

for w a vector of bits.

Run it with:


```sh
RUSTFLAGS="-C target-cpu=native" cargo run --release --features unchecked -- 24
RUSTFLAGS="-C target-cpu=native" cargo run --release --features unchecked -- \
    28 --threads 1 --reps 5 --profile custom:1:4
```

`n` is log(|w|)

This repo also contains BitZ-SNARK, a SNARK for proving R1CS constraints over the integers, using BitZ as its PCS.

## Hybrid modular multiplication + chained SHA-256

This non-ZK experiment targets 100-bit composition security. It proves
`x * y = z + 2^32 * w`, with all four values constrained to u32, together
with a SHA-256 compression chain starting from the standard initial state.

From the repository root, build and run the full hybrid sweep across all
six workload sizes:

```bash
RUSTFLAGS="-C target-cpu=native" RAYON_NUM_THREADS=8 \
  cargo bench --bench hybrid_u32_sha256 --features hybrid -- \
  --sweep --mode hybrid
```

The sweep uses six pairs of `(multiplication log, compression log)`:
`15:7,16:8,17:9,18:10,19:11,20:12`. Each pair has equal packed witness
sizes for the multiplication and SHA branches. The largest pair is
**1,048,576 modular multiplications and 4,096 chained compressions**.
`--sweep` selects all six sizes. By default, each size runs one discarded warmup followed by five measured
proofs, giving **30 measured proofs total**. `--iterations N` controls
measured repetitions per size. `--shapes MUL_LOG:SHA_LOG,...` selects other
pairs (multiplication logs 9–22, SHA logs 1–16); `--shapes 9:9,...,14:14`
proves equal operation counts N = M, an SHA-dominated workload (see
`docs/hybrid-u32-sha256-rates.md`).
Every sample proves and verifies; setup is measured separately.

If using the existing executable built in `target/hybrid-build/`, skip
Cargo and run the full sweep directly:

```bash
RAYON_NUM_THREADS=8 target/hybrid-build/release/hybrid-u32-sha256 \
  --sweep --mode hybrid
```

The executable accepts the same flags. Use `--mode all` to compare
hybrid, separate BitZ/Binius
proofs, and all-Binius proofs. Use only `--features hybrid` for this
experiment; the `unchecked` feature is rejected by hybrid setup.

Stdout labels each backend, workload size, sample, prover/verifier time,
proof size, peak RSS and verification result. Hybrid samples also report
**PIOP** time (multiplication/Spartan and SHA reductions) and **IOP / PCS
opening** time (multiplication BitZ/GKR, joint sumcheck, and shared ring
switching/Ligerito), with individual component timings. The other modes
currently report total prover time.

Each sweep saves `summary.csv`, per-workload CSVs/logs and run settings in
a fresh directory under `benches/results/hybrid-u32-sha256/` (ignored by Git).
See the [protocol and benchmark guide](docs/hybrid-u32-sha256-protocol.md)
for custom sizes, timing definitions, proof files and security accounting.


## Reproducing the paper's benchmarks

Run all commands below from the repository root. Dependency checkouts use
sibling directories such as `../limber-impl`.

Cross-system comparisons measure complete native proofs: witness generation,
commitment, constraint proving, PCS opening, and verification.

Install the native Perfetto trace processor locally, then set its path in each
shell used for benchmarks. Run these commands from the repository root:

```bash
bash scripts/install_trace_processor.sh
export PERFETTO_TRACE_PROCESSOR="$PWD/.tools/perfetto/trace_processor_shell"
export RUSTFLAGS="-C target-cpu=native"
```

The installer downloads Perfetto **v58.2** for macOS or Linux, verifies its
SHA-256 checksum, and reuses an existing matching installation. It requires
`curl` and `sha256sum` or `shasum`. The binary stays in this checkout's ignored
`.tools/perfetto/` directory; no system installation is needed.

### The benchmark campaigns

The campaign instructions live in [BENCH_INSTRUCTIONS.md](BENCH_INSTRUCTIONS.md) and are copied
below verbatim by `scripts/sync_readme_campaigns.py` (the artifact workspace's README carries the
same copy); edit the source file, then run the script.

<!-- bench-campaigns:begin (generated from BENCH_INSTRUCTIONS.md by scripts/sync_readme_campaigns.py; edit that file) -->
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

### Setup

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

### 1. SHA-256 + ECDSA over secp256k1: BitZ (matched circuit), Binius64, Binius64-Ligerito

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

### 2. SHA-256 + ECDSA over P-256: BitZ alone

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

### 3. SHA-256 chains: BitZ, Binius64, Binius64-Ligerito

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

### 4. Multiplication comparisons: BitZ, Binius64, Binius64-Ligerito, Plonky3-FRI, Limber

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

### 5. Multiplication: the Zinc+ rows

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

### 6. BitZ full-product u32 × u32 → u64, with component breakdown

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

### 7. MultiSwap: BitZ, Limber-Hyrax, Limber-Brakedown

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

### 8. SHA-256 layout parameter sweep over s and t

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

### 9. Hybrid SHA-256 chain + multiplication modulo 2^32

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

### Tables and figures

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
<!-- bench-campaigns:end -->

### Raw performance of BitZ PCS on the core LinBitsRings relation

```sh
RUSTFLAGS="-C target-cpu=native" cargo run --release --features unchecked -- \
    --sweep 20-30 --threads 8 --reps 5 --profile custom:1:4
```

### Comparison with fields-witch (Soukhanov's characteristic-2 field switch)

[fields-witch](https://github.com/morgana-proofs/fields-witch) is Lev
Soukhanov's implementation of his "Char 2 fieldswitch" note: it commits
`2^k` entries of `F_{2^127}` (integers below `2^127`) densely over
`F_{2^128}` and proves their multilinear evaluation over `F_p`,
`p = 2^127 - 1`. The comparison is bit-matched: `2^k` entries of 127 bits
are the same 16 bytes per entry as BitZ at `n = k + 7` with `W = 1`.
`scripts/run_fields_witch_compare.py` derives fields-witch's per-round limb
schedules from its README rule (it reproduces the README's `2^20` schedule
exactly), runs every (scheme, size, threads) cell in a fresh process under
`/usr/bin/time -l` on a quiet box (CPU-idle gate), and writes
`PerfRuns/<stamp>-fields-witch-compare/{results.jsonl,summary.md,fields-witch-table.tex}`;
see `docs/fields-witch-compare.md` for the measured comparison.

```sh
git clone https://github.com/morgana-proofs/fields-witch ../fields-witch   # measured at 30cca8c
(cd ../fields-witch && CARGO_TARGET_DIR=target \
    RUSTFLAGS="-C target-cpu=native" cargo build --release --examples)
RUSTFLAGS="-C target-cpu=native" cargo build --release --features unchecked --bin bitz
python3 scripts/run_fields_witch_compare.py \
    --fw-bin ../fields-witch/target/release/examples/protocol_profile \
    --sizes 14,16,18,20,22 --threads 1,8 --reps 5 \
    --word-rows 20:32,20:64 --latex paper/fields-witch-table.tex
```

`--fw-bin` / `--bitz-bin` override the binaries (the BitZ default follows
`CARGO_TARGET_DIR`); `--bitz-profile udr:1:4` measures BitZ in fields-witch's
unique-decoding regime; `--render-latex <results.jsonl> --latex <path>`
regenerates the paper table from a finished run.

### Integer multiplication

Choose `bitz` for standalone BitZ experiments or `compare` for cross-system
comparisons. Launcher options go before `--`; Rust benchmark flags go after it.
Standalone proof timings exclude native witness generation; comparison proof
timings include it.

```sh
python3 scripts/run_multiplication_benchmarks.py bitz -- \
  proof --workload u32-full,u64,u128 --log-n 15..=20 --w 1,3,8 \
  --split=0,1 --threads 1,8 --reps 5 --skip-unsupported --dry-run

python3 scripts/run_multiplication_benchmarks.py compare --output results/compare -- \
  proof --workload u32-mod32,u64,u128 --backends all --log-n 15,17,19 \
  --threads 1,8 --skip-unsupported --memory rss

python3 scripts/run_multiplication_benchmarks.py bitz -- \
  witness --workload u32-full,u64,u128,baby-bear --log-n 10 --threads 1
```

The launcher builds one executable, runs under the machine lock and swap guard,
and writes shared reports under `<output>/reports`. It defaults to a fresh
`PerfRuns/<timestamp>-multiplication` directory. `--no-gate` disables the gate;
`--swap-grow-gb` changes its default 12 GiB limit. Build settings are preserved,
with native CPU compilation used when no Rust flags are supplied.

Launcher `--dry-run` before `--` prints commands without building or writing
files. Benchmark `--dry-run` after `--` builds and validates the expanded cases
without proving. Forwarded `--help` shows the selected benchmark's flags.
Both targets support `proof`, `witness`, and `pcs`; `bitz` additionally supports
`piop`, `outer`, and `bounds`. See the
[multiplication benchmark guide](docs/native-mul-compare.md) for configuration,
measurement boundaries, memory passes, and the `mul-bench/v2` result format.

### RSA MultiSwap — matched 114-bit comparison

Compare **BitZ/Ligerito, Limber-Hyrax, and Limber-Brakedown** on Limber's
Table 1 fixture. One circuit copy contains **4 exponentiations with 352-bit
exponents modulo a 2048-bit RSA modulus**, with 6,209 live integer constraint
rows. The RSA chains execute; hash and Poseidon operations contribute modeled
costs. The fixture has no application public inputs (`count=0`, `values=[]`)
and does not prove a complete public accumulator transition.

The campaign fixes `k=0` and proves **1, 2, 4, 8, or 16 complete circuit
copies in one proof**: 4–64 RSA exponentiations. It checks matching canonical
statements and witness data across backends. Each modeled security check must
reach **at least 114 bits**; the shared 128-bit prime fingerprint retains its
roughly 114-bit bound. This accounting is per check/round, not a combined
whole-proof soundness bound or an RSA key-strength claim. Limber retains its
native 128-bit integer target and 117-bit integer challenge bound target.

Run these commands from the repository root. MultiSwap benchmarks the existing
Limber checkout supplied through `--limber-root` and records its revision for
provenance. If you need a checkout, the optional setup helper clones the Cargo
dependency revision into a destination that does not already exist:

```sh
python3 scripts/prepare_matched_limber.py ../limber-impl
```

This clones the dependency revision directly, with no patching or local commits.
Use `--limber-root ../limber-impl` for the sibling
checkout. The runner does not require its revision to match the Cargo dependency.
The setup and campaign scripts require Python 3.11 or newer.

The runner requires this repository's pinned Rust toolchain and Limber's
`nightly-2026-07-01`. It sets `MSCFG=paper` and each backend's security
parameters, overriding inherited workload/security settings. Run the full
sweep with **1 and 16 threads**, one warmup, and ten measured proofs per
configuration (**30 configurations**):

```sh
python3 scripts/run_matched_multiswap_campaign.py \
  --draft \
  --limber-root ../limber-impl \
  --security-bits 114 \
  --batch-counts 1,2,4,8,16 \
  --all-threads 16 \
  --warmups 1 \
  --samples 10 \
  --rustflags="-C target-cpu=native"
```

`--draft` runs proof verification and repository comparison checks, but marks
the results as **pending canonical validation**. The external
`zk-proof-profiler/scripts/zk_trace.py` validator is not bundled here. For
canonical execution, replace `--draft` with `--profiler` followed by the
actual path to that file. A placeholder path will fail preflight.

Add `--dry-run` to preview the commands without compiling or running proofs.
For a smoke run, change to `--batch-counts 1 --samples 1` (six configurations).
Results appear under `bench_results/<campaign>/reports/combined/` as
`summary.json`, `metrics.csv`, and `intervals.html`. They report witness,
commitment-plus-proving, combined prover, and verification times, along with
proof sizes including commitments and process peak memory. Compilation and
setup are excluded from headline proving times; analytical proof-size
estimates are marked.

See the [campaign guide](docs/matched-multiswap-campaign.md) for the statement,
security accounting, toolchain setup, and validation requirements. The
[historical comparison rows](#historical-multiswap-comparison-rows-limber-zinc)
below predate this matched campaign.


### SHA-256 
```sh
CARGO_TARGET_DIR=target RUSTFLAGS="-C target-cpu=native" \
RAYON_NUM_THREADS=8 \
BITZ_SHA_COMPARE_EXPONENTS="4 5 6 7 9 10 11 12" \
BITZ_SHA_COMPARE_REPS=5 \
BITZ_SHA_COMPARE_BACKENDS="bitz binius64" \
BITZ_SHA_COMPARE_OUTPUT_DIR="PerfRuns/$(date -u +%Y-%m-%dT%H-%M-%SZ)-sha256-compare" \
  cargo bench --bench sha256_e2e_compare --features bench-internals,native-sha256-compare
```

# Scratch

## Historical MultiSwap comparison rows (Limber, Zinc+)

These timings are historical one-copy measurements with mixed security
settings, not results from the matched 114-bit campaign above. The historical
BitZ setting can still be selected explicitly (use `RAYON_NUM_THREADS=8` for
the 8-thread column):

```sh
BITZ_BENCH_LAMBDA=114 BITZ_BENCH_SHAPES=0 BITZ_MULTISWAP_BATCH_COUNT=1 \
  BITZ_BENCH_REPS=5 RAYON_NUM_THREADS=1 RUSTFLAGS="-C target-cpu=native" \
  cargo bench --bench multiswap --features unchecked
```

Same box (Apple M5, 4P+6E cores, 24 GB), `-C target-cpu=native`, medians of 5, 1 thread / 8 rayon threads; prover time includes commitment, excludes witness generation (< 0.15 s everywhere). LaTeX table: `paper/multiswap-table.tex`.

| System (commit) | Prove 1 thr | Prove 8 thr | Verify 1 thr | Verify 8 thr | Proof |
|---|---|---|---|---|---|
| BitZ-SNARK (historical checkout), 114 bits | 243 ms | 97 ms | 9.0 ms | 11.7 ms | 269 KB |
| Zinc+ main-beta (`878fbd8`), 16-bit limbs + range checks, 114 bits (14 grinding bits) | 1973 ms | 569 ms | 18.2 ms | 12.5 ms | 1272 KB (841 KiB zstd) |
| Zinc+ main-beta (`878fbd8`), fat-cell mock, no range checks, 100 bits | 781 ms | 236 ms | 54.0 ms | 23.5 ms | 1415 KB |
| Limber-Brakedown (`b003684`) | 1063 ms | 542 ms | 39.6 ms | 40.0 ms | 5769 KB |
| Limber-Hyrax (`b003684`) | 1116 ms | 368 ms | 34.1 ms | 20.3 ms | 175 KB |

Historical BitZ opener: Ligerito in the unique-decoding regime, rate 1/8, fold arity 4, fold grinding, validated at the 114-bit target (CLI profile `udrg:3:4:114`).

Limber — [albert-garreta/limber-impl](https://github.com/albert-garreta/limber-impl) `b003684` (fork of lucasxia01/limber-impl `853c6c4`; Rust ≥ 1.97). `MSCFG=paper` is required: the default `full` is a newer 2^14-row circuit, not the Table 1 statement. `RAYON_NUM_THREADS=8` for the 8-thread columns.

```sh
MSCFG=paper RAYON_NUM_THREADS=1 RUSTFLAGS="-C target-cpu=native" cargo bench --bench multiswap_modp           # Hyrax (Criterion; prove/ = commit+prove)
MSCFG=paper PSIZE=1 RAYON_NUM_THREADS=1 RUSTFLAGS="-C target-cpu=native" cargo bench --bench multiswap_modp   # Hyrax proof size
MSCFG=paper BDPCS=1 RAYON_NUM_THREADS=1 RUSTFLAGS="-C target-cpu=native" cargo bench --bench multiswap_modp   # Brakedown (one run; repeat 5×)
```

Zinc+ — [NethermindEth/zinc-plus](https://github.com/NethermindEth/zinc-plus) branch `main-beta` `878fbd8`; `Cargo.lock` is untracked, pin `crypto-primitives` to `2cf39db886a76dc3e961cbb9c86fb5ab042381ef` (`cargo update -p crypto-primitives --precise …`). For the 8-thread columns add the `parallel` feature and `RAYON_NUM_THREADS=8`. Methodology: `docs/limber-2026-1635-sound-row.md` there.

```sh
LIMB16=1 NVARS=13 REPS=5 RUSTFLAGS="-C target-cpu=native" cargo bench --features "simd unchecked iprs-rate-1-8 sec-114" --bench limber_multiswap   # range-checked, 114 bits
FOLD=1 WIDE16=1 NVARS=11 REPS=5 RUSTFLAGS="-C target-cpu=native" cargo bench --features "simd unchecked iprs-rate-1-8" --bench limber_multiswap   # fat-cell mock, 100 bits
```

## Integer R1CS with F_2 virtualization

Every relation below runs the one protocol runner in
`src/piop/spartan/protocol/` (`docs/unified-protocol.md`); the per-relation
`prove_*`/`verify_*` entry points are thin wrappers over it, and
`tests/transcript_state_pins.rs` pins every transcript.

Pick the security parameter with `BITZ_BENCH_LAMBDA`. Unless noted otherwise,
the benches below honour it. The fixed-prime `(t,s)` sweep uses its own
λ=100 profile:

| `BITZ_BENCH_LAMBDA` | profile | meaning |
|---|---|---|
| `100` | `Lambda100` | no grinding anywhere; every term this crate controls ≥ 100 bits |
| `128` | `Lambda128` | every controllable term ≥ 128 bits (two grinding bits per forest round, one at the ring switch; the GF(2^128) floor at ~126.4 still binds and is reported) |
| `112` | `Limber112` | historical matched MultiSwap target; Ligerito target 112, eight reduction grinding bits for the selected batch sweep — MultiSwap only |
| `114` | `Limber114` | matched campaign and standalone MultiSwap default; Ligerito target 114, ten reduction grinding bits for the selected batch sweep — MultiSwap only |
| `sha128-reference-schedule` | `Sha128ReferenceSchedule` | the historical SHA-256 128-bit schedule, kept for comparison |

The profile names are accepted too (`BITZ_BENCH_LAMBDA=lambda128`). Unset,
SHA-256 and u32×u32 run at λ=100, MultiSwap at 114, and the BabyBear and
`lambda_sweep` benches run every profile they know (two and three rows per
shape). A profile the bench's relation cannot instantiate — `112`/`114` outside
MultiSwap, or `100`/`128` on MultiSwap — aborts up front with the
admissible list. Each `RESULT` line carries `profile=<name>` next to
`lambda=<bits>`. `BITZ_BENCH_QUIET=1` mutes the benches' advisory
`warning:` lines (e.g. the `pcs` bench's note that it ignores
`BITZ_BENCH_LAMBDA`); errors still abort.



### SHA-256 independent compressions — λ=100 bits of security (`BITZ_BENCH_LAMBDA=128` for the 128-bit profile)
```sh
BITZ_BENCH_LAMBDA=100 BITZ_BENCH_SHAPES=14 BITZ_BENCH_REPS=3 RUSTFLAGS="-C target-cpu=native" \
  cargo bench --bench sha256_compressions --features unchecked
```

Size the same benchmark by the packed assignment domain (`MnumRows=2^n`)
instead of a power-of-two compression count with:

```sh
BITZ_BENCH_LAMBDA=100 BITZ_SHA_MNUMROWS_LOG2S="24 25" BITZ_BENCH_REPS=3 RUSTFLAGS="-C target-cpu=native" \
  cargo bench --bench sha256_compressions --features unchecked
```

This uses `floor((2^n - 1) / 20456)` compressions: one shared constant,
20,456 adjacent assignment cells per compression, and one trailing zero
suffix only.

`BITZ_SHA_OPENING_T=<t>` (compression-count shapes only) gives the opening
an explicit BitZ split of `2^t` rows × `2^(vars − t)` columns. The default
opens the balanced `Id_{2^r} ⊗ M` block layout: `r` instance bits join the
15 local bits on the row axis, so one row block covers `2^r` compressions
and only the remaining `k − r` instance bits index columns. `r` is tuned so
`t = 15 + r` lands on the instantiation's `t = ceil(0.6 · n)` balance point
(clamped to `r ∈ [0, k]`). That keeps the read-off vector — the `2^s`
~125-bit integers sent in the clear — at `2^(k−r)` integers instead of
growing with the 2^15 local cells: the earlier `t = min(k, 13)` pin sent
327 KB at 2^12–2^13 and doubled per step from 2^14 on. Crossing the
one-forest cap (`127 − t − 1 < q_bits`) is the cost: the opening runs one
merged forest per weight chunk, and the forests are the whole surcharge
(2^14: 361 KB at 1.45 s, against 930 KB at 0.57 s for the old pin). `BITZ_SHA_OPENING_LAYOUT=inner` (the default) takes the
inner-sumcheck path for the split; `=product` keeps the direct product
opening and transposes the product tensor instead (instance-major: the 15
local bits plus the low instance bits form the rows, the high instance bits
the columns; `t ≥ 15`), which is the cheap way to get the split. The bench
prints the layout, forest count and read-off width per shape.

### SHA-256 `(t,s)` product-layout sweep — fixed 98-bit prime, λ=100

This sweep fixes `2^14` independent SHA-256 compressions and `t+s=29`:
the product assignment has `2^t` rows and `2^s` columns. Run this command
from the repository root.

Full default sweep, `t=7..27`, with one warmup and 21 measured samples per
split:

```sh
RAYON_NUM_THREADS=8 BITZ_BENCH_LAMBDA=100 BITZ_BENCH_REPS=21 \
BITZ_SHA_RESULT_PATH="benchmark-results/sha256_product_layout_$(date +%Y%m%d_%H%M%S).txt" \
  cargo bench --features "bench-internals bench-peak-memory" --bench sha256_product_layout
```

`BITZ_SHA_PRODUCT_TS` selects individual `t` values; leave it unset for the
full default sweep. Leave the other SHA shape/layout overrides unset
(`BITZ_BENCH_SHAPES`, `BITZ_SHA_LOG2S`, `BITZ_SHA_MNUMROWS_LOG2S`,
`BITZ_SHA_OPENING_T`, and `BITZ_SHA_OPENING_LAYOUT`).

Each timestamped result file contains verified `SAMPLE` records and a
`RESULT` summary per split. Both include proof size (`proof_bytes`, with
PIOP/opening components) and peak live heap (`peak_heap_bytes` and
`peak_heap_mib`). The heap window covers witness generation, commitment,
and proving, including allocations already live at the start; it excludes
verification and proof serialization. Summary timings are medians, and the
summary heap peak is the maximum over measured runs, excluding the warmup.
This measures Rust heap allocations, not process RSS. Allocator tracking
also instruments the timed runs; omit `bench-peak-memory` for timing-only
runs, which report memory as `na`.

The default sweep also records `t=1..6`
as unsupported because packing requires at least 128 rows, and `t=28` as
skipped because its projected peak exceeds 60 GiB. Explicit selections
accept `t=7..28`; `t=29` is not supported.

### SHA-256 chain — `2^k` CHAINED compressions (a `64·2^k`-byte Merkle–Damgård chain), λ=100:
```sh
BITZ_BENCH_LAMBDA=100 BITZ_BENCH_SHAPES=14 BITZ_BENCH_REPS=3 RUSTFLAGS="-C target-cpu=native" \
  cargo bench --bench sha256_chain --features unchecked
```

The chained counterpart of `sha256_compressions`: `H_{i+1} = Compress(H_i,
M_i)` from the standard initial state, proved as ONE relation whose
intermediate chaining values are witness. Same circuit and 184 linear
constraints per compression, same runtime-prime protocol and direct product
opening; the difference is the F₂ map. Instance `i` commits only its block
and hint bits (6,888 instead of 7,144 — its 256 chaining-state bits are
read straight from instance `i − 1`'s committed output cells through the
chained virtual map, instance 0's from the initial-state constants), and
every instance carries 256 *terminal* rows that are the last instance's
output bits and structurally zero elsewhere. The public statement is the
block sequence plus the digest, bound by the same uniform public-I/O
batching (block slots at every instance; terminal slots equal to the
digest at the last instance, zero before it). The prover-side cost of the
chaining is confined to the ring switch's batching passes, which gain one
rotated-instance term over 264 columns per compression; the forest, the
PIOP and the proof bytes are the independent batch's.
`prepare_sha256_chain_batch_with_profile_and_initial_state` prepares a
chain from any public initial chaining value (a continuation).

Compare the same raw chain with **BitZ, Binius64, and Binius64-Ligerito**:

```bash
python3 scripts/run_sha256_chain_compare.py \
  --methods bitz binius64 binius64-ligerito \
  --exponents 7 8 9 10 11 12 13 14 15 16 \
  --threads 1 10 --reps 5 \
  --bitz-profiles custom:1:4 custom:3:4 --binius-rates 1 3 \
  --output bench_results/sha256-chain-comparison
```

This runs 120 configurations sequentially, with one excluded warmup per
configuration. Every backend uses identical deterministic public blocks and
final chaining state, starts from the standard IV, and verifies every proof.
No padding or ECDSA is added. `--dry-run` previews the matrix without building
or writing files. The runner uses the local Perfetto processor and runs directly
without `bench_gate` or an external Python profiler.

The new output directory contains `summary.csv` (median timings and whole-worker
peak RSS), `samples.csv`, raw result JSON with phase timings and security details,
logs, fixtures, and source/build provenance. Prove time includes commitment;
E2E also includes witness generation, while setup and verification are separate.
Peak RSS includes fixture construction, setup and warmup, and excludes compilation.
BitZ proof size combines analytical PIOP payload bytes with serialized PCS and
commitment bytes; both Binius variants report serialized proofs including
commitments. The 100-bit BitZ economic target, BaseFold query target, and
Binius-Ligerito per-round algebraic target have distinct accounting, recorded
in each result. Timed-out or invalid cases retain their logs and fail the campaign.

### Padded SHA-256 message with P-256 ECDSA

The `ecdsa` feature adds one padded message hash followed by one signature
verification, using a shared BitZ source commitment. SHA's linear rows bypass
the outer sumcheck and join the ECDSA matrix claims in one shared inner
sumcheck. For `N=2^i` total compressions, the message has `64*(N-1)` bytes.
The [integration and benchmark guide](docs/sha256-ecdsa.md) covers the API,
the all-rows comparison mode, and the 100/128 economic security settings.
This benchmark takes its shape and security setting as command-line arguments.

The [signed-chain comparison](docs/sha256-ecdsa-comparison.md) measures BitZ
Split, BitZ AllRows, and non-ZK Spartan MC on identical fixtures. It supports
configurable Spartan chunking, separate setup/witness/prove/verify timers,
decoded-proof verification, complete proof sizes, and resumable campaigns.

Run **BitZ Split and Spartan** for `2^4` through `2^11` total SHA-256
compressions, including padding, with one ECDSA verification per proof.
Cargo fetches missing dependencies, including the published Spartan revision
pinned by this repository. Add `--offline` when all dependencies are cached.

```sh
python3 scripts/run_sha256_ecdsa_compare.py \
  --output bench_results/bitz-split-spartan-i4-i11 \
  --methods bitz-split spartan-mc \
  --spartan-splits 0:4 0:5 0:6 0:7 0:8 0:9 0:10 0:11 \
  --targets 100 \
  --threads 1 32 \
  --reps 3
```

`--spartan-splits r:c` controls **Spartan's chunking only**: `2^r`
compressions per instance and `2^c` instances, for `2^(r+c)` total
compressions. For example, `0:11` means 2,048 instances of one compression,
while `4:7` means 128 instances of 16 compressions in the same chain.
BitZ uses only the total exponent `i=r+c`; the runner deduplicates BitZ runs
across Spartan chunkings with the same total and other settings.
The method name `bitz-split` instead refers to handling SHA's linear constraints
separately from the ECDSA outer sumcheck.

Use `--methods bitz-split` or `--methods spartan-mc` to run either method alone.
`--targets 100` selects BitZ's economic security target; Spartan retains its
nominal 128-bit group setting. The runner builds the release benchmark and
verifies every proof. It writes commit time, witness generation time, PIOP time,
IOP/PCS time, verifier time, proof size, and peak memory to `summary.csv`
(medians) and `samples.csv` (all trials) in the output directory. Peak memory
is the whole worker process maximum, including setup, warmup, and verification.

The [current comparison results](docs/sha256-ecdsa-shared-kernels-results.md)
use Spartan2's shared NeutronNova and sumcheck kernels through the non-ZK
adapter. They include paired BitZ/Spartan tables for 1 and 32 threads, with
end-to-end totals from witness generation through verification.

#### BitZ versus ZKPassport non-ZK UltraHonk

Compare BitZ Split with the ZKPassport-derived Noir circuit using native
Barretenberg 5.0.0 UltraHonk, with zero knowledge disabled. Both prove the
same SHA-256 compression chain followed by one P-256 ECDSA verification,
using identical low-s fixtures.

Use the sibling `../zk-passport-circuits` checkout on the fork's
[`bitz-benching` branch](https://github.com/wu-s-john/zk-passport-circuits/tree/bitz-benching).
Run from this repository's root on Linux x86_64. The bootstrap builds the
Rust runner and prepares the pinned native tools:

```sh
python3 benchmarks/zkpassport/build.py --test
```

Run **8, 32, and 256 compressions with 16 threads**:

```sh
python3 scripts/run_sha256_ecdsa_compare.py \
  --methods bitz-split zkpassport-honk \
  --exponents 3 5 8 \
  --threads 16 \
  --targets 100 128 \
  --reps 3 \
  --output bench_results/sha256-ecdsa-zkpassport-16t
```

`--exponents 3 5 8` selects `2^3`, `2^5`, and `2^8` total compressions,
including the mandatory SHA padding block. Each configuration runs one
warmup and three measured proofs, all verified. BitZ runs both economic
security targets; UltraHonk runs once per size using its BN254 KZG security
model. This gives **9 configurations and 36 verified proofs**, including
warmups. To run UltraHonk alone, use `--methods zkpassport-honk`.

The output directory contains `summary.csv`, `samples.csv`, raw logs,
shared fixtures, and build/circuit provenance. Setup and SRS downloads are
excluded from proving times. UltraHonk reports aggregate proving time;
unavailable internal phase timings remain null. Rerunning the same command
resumes completed work; choose a new output directory after code changes
or to collect fresh measurements.

See the [native ZKPassport benchmark guide](benchmarks/zkpassport/README.md)
for measurement boundaries, offline operation, and the retained upstream
Noir constraint-coverage diagnostic.

### Multiplication and BabyBear

```sh
cargo bench --bench mul_bitz --features span-metrics,bench-internals -- \
  proof --workload baby-bear --log-n 15,20 --bitz-profile 100,128 --threads 8
```

All multiplication benchmarks and reports use the [two-target interface](docs/native-mul-compare.md).

### SHA security-profile sweep: Lambda100 / Sha128ReferenceSchedule / Lambda128 (set `BITZ_BENCH_LAMBDA` for one of them):
```sh
BITZ_BENCH_SHAPES=12 BITZ_BENCH_REPS=3 RUSTFLAGS="-C target-cpu=native" \
  cargo bench --bench lambda_sweep --features unchecked
```

### PCS-only (t:s:W triples; no IOP security profile, so `BITZ_BENCH_LAMBDA` does not apply; profiling stays opt-in here — add OBLONG_PROFILE=1 for the phase line):
```sh
BITZ_BENCH_SHAPES="17:11:1" BITZ_BENCH_REPS=5 RUSTFLAGS="-C target-cpu=native" \
  cargo bench --bench pcs --features unchecked,span-metrics,bench-peak-memory
```

## Dependencies

- https://github.com/albert-garreta/flock-mod
- https://github.com/worldfnd/f2z-benchmark
- [Albert: I'm not sure what this is. Leaving it here just in case] **`crypto-primitives`** — vendored at `vendor/crypto-primitives` (NethermindEth, Apache-2.0; see `vendor/crypto-primitives/VENDORED.md` for
  the pinned revision and the crypto-bigint 0.7.5 / rand 0.10 port).
