# BitZ benchmark workspace

This working checkout contains BitZ and local source repositories for
its retained third-party benchmark dependencies. The root Git history is
preserved; packaging changes are ordinary working-tree edits on master.
The independent vendor histories are normalized. Cloning the root repository
alone does not copy the independent vendor repositories.

```text
BitZ/
  .git/                   Existing BitZ history (preserved)
  Cargo.toml
  Cargo.lock
  provenance.toml
  src/                    BitZ prover and command-line tools
  crates/                 Local circuit implementation
  benches/                Retained comparisons
  benchmarks/binius64/     Isolated SHA+P-256 worker
  scripts/                Campaigns, report generators, compile-only checks
  vendor/
    field/                Local field implementation, tracked by BitZ
    limber/.git/
    plonky3/.git/
    binius64/.git/
    flock-mod/.git/
```

Each independent vendor repository preserves its designated upstream history
and ends with one `bitzcodes <bitzcodes@fastmail.com>` snapshot commit.
`provenance.toml` records the actual parent and snapshot commits. Limber's
upstream metadata was normalized for selected contributors; its upstream
source code, commit structure, and other contributors' attribution are preserved.
An upstream-committed executable is removed from every historical tree.
Limber's `halo2curves` dependency is fetched by Cargo from its official upstream
Git repository at the exact revision recorded in `provenance.toml`. Cargo may
also download registry packages. The field reference microbenchmark and
comparison tests requiring separate Binius, Flock, or Poseidon2 reference
repositories are omitted; the retained benchmark campaigns use the main vendors.

Inspect code and history directly, for example:

```sh
git log --oneline
git -C vendor/limber log --oneline
git -C vendor/binius64 show HEAD
```

## Compile without running benchmarks

Install Rust `1.98.1` and `nightly-2026-07-01`, then run:

```sh
bash scripts/compile_export.sh
```

This builds all seven campaigns below and their affected Rust tests.
It uses native CPU code generation and locked dependencies. Registry packages
and the pinned upstream `halo2curves` source may be downloaded. No benchmark,
proof, test executable, or report generator is run by this script. Cargo's
normal build scripts and procedural macros run
as part of compilation. Build outputs and logs are ignored by Git.

To compile the hybrid SHA-256/multiplication benchmark without executing it:

```sh
RUSTFLAGS="-C target-cpu=native" cargo +1.98.1 bench --locked --no-run \
  --bench hybrid_u32_sha256 --features hybrid
```

## Benchmark campaigns

Run the following commands from the repository root in **Bash**. These commands
execute benchmarks and verify generated proofs. Use a fresh `RUN_DIR` for each
campaign and run the workloads sequentially on an otherwise idle machine.

### Setup

```bash
set -euo pipefail

rustup toolchain install 1.98.1
rustup toolchain install nightly-2026-07-01
bash scripts/install_trace_processor.sh

export RUSTFLAGS="-C target-cpu=native"
export PERFETTO_TRACE_PROCESSOR="$PWD/.tools/perfetto/trace_processor_shell"
unset BITZ_LIG_PROFILE CARGO_ENCODED_RUSTFLAGS CARGO_TARGET_DIR

mkdir -p bench_results
export RUN_DIR="$(mktemp -d "$PWD/bench_results/all-benchmarks-$(date +%Y%m%d-%H%M%S)-XXXXXX")"
LIMBER_DIR="$PWD/vendor/limber"
echo "Results: $RUN_DIR"
```

### 1. SHA-256 + P-256: BitZ, Binius64, Binius64-Ligerito

```bash
python3 scripts/run_sha256_ecdsa_compare.py \
  --output "$RUN_DIR/sha256-p256" \
  --methods bitz-split binius64 binius64-ligerito \
  --exponents 4 5 6 7 \
  --targets 100 \
  --threads 1 10 \
  --reps 5 \
  --bitz-profiles custom:1:4 custom:3:4 \
  --binius-rates 1 3 \
  --timing perfetto \
  2>&1 | tee "$RUN_DIR/sha256-p256.log"
```

### 2. SHA-256 chains: BitZ, Binius64, Binius64-Ligerito

```bash
python3 scripts/run_sha256_chain_compare.py \
  --methods bitz binius64 binius64-ligerito \
  --exponents 7 8 9 10 11 12 13 14 15 16 \
  --threads 1 10 \
  --reps 5 \
  --bitz-profiles custom:1:4 custom:3:4 \
  --binius-rates 1 3 \
  --output "$RUN_DIR/sha256-chain" \
  2>&1 | tee "$RUN_DIR/sha256-chain.log"
```

### 3. Multiplication comparisons

The runner selects the supported sizes for each backend and operand width.
Plonky3-FRI supports the u32 workload only. `--no-gate` disables the machine lock
and swap guard, so this command should be run without concurrent benchmarks.

```bash
python3 scripts/run_multiplication_benchmarks.py \
  --no-gate \
  --workloads u32 u64 u128 \
  --backends bitz binius64 binius64-ligerito plonky3-fri limber \
  --threads 1 10 \
  --reps 5 \
  --bitz-profiles custom:1:4 custom:3:4 \
  --binius-rates 1 3 \
  --output "$RUN_DIR/multiplication" \
  2>&1 | tee "$RUN_DIR/multiplication.log"
```

### 4. BitZ full-product u32 × u32 → u64, with component breakdown

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

### 5. MultiSwap: BitZ, Limber-Hyrax, Limber-Brakedown

This uses the local `vendor/limber` snapshot. `MSCFG=paper` is a workload name
and does not require a manuscript directory. `--draft` runs proofs and the
local comparison checks while marking canonical trace validation as pending.

```bash
python3 scripts/run_matched_multiswap_campaign.py \
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

### 6. Hybrid SHA-256 chain + multiplication modulo 2^32

This is the paper's **“Modular multiplications and bit operations”** experiment
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

### 7. SHA-256 layout parameter sweep over s and t

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

### Tables and figures

The bundled `scripts/zk_trace.py` provides trace report tooling. Tables default
to `outputs/tables/` and figures to `outputs/figures/`; the root `paper/` directory
is not needed. After collecting the hybrid witness sweeps above, its table can
be generated with:

```bash
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
directories instead. Generation of a table does not rerun the proofs.

Source attribution and licenses are retained alongside the incorporated code.
Historical upstream citations may identify their original contributors;
metadata normalization does not prevent recognizing previously published code.

## Reviewing the changes

Use `git diff` and `git status` at the root to review the working-tree changes.
Use `git -C vendor/<name> diff HEAD^ HEAD` to inspect a vendor's complete
customization commit. Existing root-tracked vendor files remain in the root
index for review; no root index or history rewrite was performed.

The root development history and local documentation are retained for review.
Do not treat the entire development checkout, including its root `.git` and
local configuration, as an anonymous distribution artifact.
