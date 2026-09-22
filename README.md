# BitZ

BitZ is a hash-based polynomial commitment scheme (PCS) for witnesses over
arbitrary rings. It commits to a vector f ∈ Sⁿ over any finitely generated ring S
(a finite field, ℤ, ℤ/2³², a cyclotomic ring, …) and proves linear claims about
it, such as multilinear-extension (MLE) evaluations:

    ⟨f, v⟩ = μ  in S,    or, through a ring homomorphism ψ: S → R,    ⟨ψ(f), v⟩ = μ  in R.

A typical instance is S = ℤ, R = F_q, and ψ reduction modulo q. The name is
"Bit-ℤ": the ring is switched to bits and then to a binary field.

## How it works

1. **Bitification.** The claim is rewritten as a claim about the bit-decomposition
   of f. Decomposition is linear, so an inner product on f becomes an inner product
   on its bits with a different weight vector.
2. **Commitment.** The bits are committed as elements of the binary field GF(2¹²⁸),
   with 128 bits packed into each field element, using a packed hash-based scheme:
   ring-switching plus a recursive Ligerito opener (the Flock code in `vendor/flock-mod`).
3. **Lift to ℤ.** The prover sends μ′ ∈ ℤ, the value of the lifted inner product.
4. **Proof in the exponent.** For a generator g of GF(2¹²⁸)*, the claim becomes
   g^⟨lift(u), bits(f)⟩ = g^μ′. That is a product Πᵢ ((g^{uᵢ} − 1)·bitᵢ + 1) over
   the committed bits, proved as a grand product by a GKR-style forest specialised
   to low-entropy inputs. There is no wrong-field arithmetic in this step.

Rings that are not prime fields, and values too large for the order of g, are
handled by lifting the claim to ℤ[X₁, …, X_k] and projecting it onto a random
prime field before step 3.

What this buys:

- **Pay per bit.** Cost depends essentially only on the bit-size of the witness, not on
  the ring, so ℤ, ℤ/2³² and prime fields cost about the same per bit.
- **Composability.** BitZ can serve as the PCS of a proof system over a prime field, ℤ,
  a polynomial ring or a lattice ring, and it is hash-based.
- **Free range checks.** Commitments to integers take a bit-size parameter B, so a prover
  cannot commit to larger integers, and many range checks disappear.

## What is in this repository

- **The PCS and CLI (`src/`, binary `bitz`).** The core PCS proves `MLE[w](r) = y ∈ F_q`
  for a bit vector w. Opened with a Flock-backed ring-switch and recursive Ligerito.
- **BitZ-SNARK.** A SNARK for R1CS over the integers using the fingerprinting paradigm: commit
  over ℤ, the verifier samples a random prime q, the R1CS is projected to F_q, a PIOP
  reduces it to an MLE claim, and BitZ proves that claim. A hybrid variant proves
  constraints over ℤ and binary fields together, with virtualized F₂ addition (XOR)
  between integer witnesses.
- **Benchmark campaigns.** Seven campaigns compare BitZ with Binius64, Limber and
  Plonky3 (FRI and WHIR) on SHA-256 with P-256 ECDSA, SHA-256 chains, integer
  multiplication (u32, u64, u128), MultiSwap, and a hybrid of SHA-256 with modular
  multiplication. They target 100 bits of security in non-ZK mode and are documented below.
- **The manuscript.** *BitZ: proofs and commitments in arbitrary rings through binary
  fields* lives in `paper/`.

Quick start, proving an MLE evaluation of a bit vector of length 2ⁿ:

```sh
RUSTFLAGS="-C target-cpu=native" cargo run --release --features unchecked -- 24
RUSTFLAGS="-C target-cpu=native" cargo run --release --features unchecked -- \
    28 --threads 1 --reps 5 --profile custom:1:4
```

## Dependencies

Cargo fetches everything it needs; there are no submodules and no vendoring step.

- **In-tree:** `vendor/field` (field arithmetic) and `vendor/flock-mod` (the Flock library,
  derived from `succinctlabs/flock`). Each has a `VENDORED.md`.
- **Git pins:** Limber, Plonky3 and Binius64 are the `wu-s-john` forks, each pinned to an
  exact commit in `Cargo.toml` (and `benchmarks/binius64/Cargo.toml`), plus the official
  `halo2curves` revision Limber needs. `Cargo.lock` records the resolved sources.
- **Requirements:** Python 3.11 or newer and `rustup`. The first build needs network
  access for the registry and the git pins. Git is also needed to fetch the pinned Limber
  checkout for MultiSwap (into `.tools/limber`).

The field reference microbenchmark and retired external comparison integrations
are omitted; the seven retained campaigns are documented below.

## Run all seven campaigns

From the repository root:

```sh
bash scripts/run_all_benchmarks.sh --dry-run
bash scripts/run_all_benchmarks.sh --smoke
bash scripts/run_all_benchmarks.sh
```

The wrapper runs the seven campaigns below sequentially. It installs the pinned Rust
toolchains and Perfetto if needed, and creates a fresh
results directory under `bench_results/`. Use `--output DIR` to choose another
new directory. `CARGO_TARGET_DIR` is preserved for build-cache reuse.

`BITZ_REVISION` and `BITZ_DIRTY` are optional metadata: the shared validator in
`benches/common/mod.rs` accepts them while still rejecting unknown `BITZ_*` knobs.
They are only fallbacks for when Git is unavailable at run time; otherwise results
record the revision, dirty flag and tracked diff straight from Git.

`--dry-run` only prints commands; it does not compile, verify dependencies, or
execute benchmarks. `--smoke` uses one size and one measured sample per campaign,
retaining the listed backends, rates, and thread counts. It also exercises the
equal-count hybrid table. Warmups and separate memory trials still run.

One outer benchmark lock protects the complete workflow; before the first
campaign, the gate waits for at least 88% CPU idle held for 120 s. Multiplication
receives `--no-gate` internally to avoid taking that lock twice. The default
swap-growth guard is 34 GiB, configurable with `--swap-grow-gb`. Failures retain
their logs and stop the workflow without claiming completion. The wrapper waits for
idle only once; see [Measurement conditions](#measurement-conditions-of-the-published-numbers)
for how the published numbers were gated.

## Compile without running benchmarks

Install Rust `1.98.1` and `nightly-2026-07-01`, then run:

```sh
bash scripts/compile_export.sh
```

This builds all seven campaigns below and their affected Rust tests.
It uses native CPU code generation and locked dependencies. Registry packages
and the git-pinned dependencies (Limber, Plonky3, Binius64, `halo2curves`) may be
downloaded, and the pinned Limber commit is fetched into `.tools/limber`. No benchmark,
proof, test executable, or report generator is run by this script. Cargo's
normal build scripts and procedural macros run
as part of compilation. Build outputs and logs are ignored by Git.

To compile the hybrid SHA-256/multiplication benchmark without executing it:

```sh
RUSTFLAGS="-C target-cpu=native" cargo +1.98.1 bench --locked --no-run \
  --bench hybrid_u32_sha256 --features hybrid
```

## Measurement conditions of the published numbers

The paper's numbers were measured on an Apple M5 (24 GB) under the conditions
below. Departing from them moves the numbers by more than most of the effects the
tables report.

- **Idle gate.** Every timed campaign started only after `scripts/bench_gate.py`
  saw at least 88% CPU idle held for 120 s, sampled every 20 s (`--min-idle`,
  `--hold-seconds`, `--poll-seconds`). Run back-to-back on a warm machine, an
  unchanged binary measured a 21% slower prover and a 47% slower verifier (SHA-256,
  `2^14` compressions, 10 threads). `--hold-seconds 60` is acceptable; do not drop
  the wait.
- **Campaign granularity.** One gated invocation per workload, backend, rate and
  thread count, with the sizes running inside it; the SHA-256 tables were gated per
  rate and thread group. The multiplication launcher (campaign 3, without
  `--no-gate`) gates each of its campaigns itself. To reproduce a SHA-256 table
  group, run it separately under the gate, for example:

  ```bash
  python3 scripts/bench_gate.py run --label sha256-p256-rate2-t10 -- \
    python3 scripts/run_sha256_ecdsa_compare.py \
    --output "$RUN_DIR/sha256-p256-rate2-t10" \
    --methods bitz-split binius64 binius64-ligerito --exponents 4 5 6 7 \
    --targets 100 --threads 10 --reps 5 --bitz-profiles custom:1:4 \
    --binius-rates 1 --timing perfetto
  ```

- **GKR forest schedule.** On Apple Silicon, `src/merged_forest/schedule.rs`
  overrides the shared L/8 rule for the measured single-claim shapes (see
  `docs/gkr-full-product-regression.md`). Each multiplication result records the
  resolved schedule under `effective.gkr_schedules`; read it there rather than
  assuming one. Explicit `--gkr-schedule` requests are never substituted.
- **Builds.** `cargo +1.98.1`, fat LTO and one codegen unit (the release and bench
  profiles), and `RUSTFLAGS="-C target-cpu=native"` for every scheme, including the
  competitors. Resolve benchmark executables from Cargo's `--message-format=json`
  output, as the runners do, never by listing an existing `target/` directory. A
  stale binary measures old code, and can also reject shapes that the current
  source accepts.
- **Fixed inputs.** The u32 corpus seed is `0x5533_3250_4353_0064`
  (6139306037344403556). Each multiplication result records its per-exponent
  corpus digest (`effective.corpus_digest`); rows measured at another seed are not
  comparable. Transcript domain strings determine the proof bytes, so changing them
  changes the proof-size columns.

Not reproducible from this artifact:

- the Fields-Witch comparison (rates 1/2 and 1/8); its runner is not included;
- the Zinc+ rows; the external Zinc+ comparison is omitted.

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

The launcher builds once, validates every selection with the Rust case planner,
runs sequentially, and generates combined reports in `<output>/reports`. Without
a `bitz` or `compare` target, it uses the retained per-backend size limits:
180 configurations across u32-mod32, u64, and u128, with 1 and 10 threads, both
BitZ/Binius rates, one warmup, five samples, and a separate RSS trial. Plonky3-FRI
runs only u32-mod32. Limber stops at exponent 19; other defaults range through
21 or 23 depending on workload and backend.

```bash
python3 scripts/run_multiplication_benchmarks.py \
  --output "$RUN_DIR/multiplication" \
  2>&1 | tee "$RUN_DIR/multiplication.log"
```

Use `--dry-run` to preview this matrix or `--exponents 15 --reps 1 --threads 1`
for a smaller run. Direct experiments, including WHIR, remain available with
`compare -- proof --backends all --log-n 15 --threads 1` or `bitz -- ...`.
For direct experiments, launcher flags precede `--` and Rust options follow it;
benchmark `--dry-run` after the separator compiles and validates case selection.
The reporter consumes `mul-bench/v2` results; historical formats are unsupported.

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

This fetches the Limber commit pinned in `Cargo.toml` into `.tools/limber` on first
use. `MSCFG=paper` is a workload name and does not require a manuscript directory. `--draft` runs proofs and the
local comparison checks while marking canonical trace validation as pending.

Matched reports use the same minimal-byte v1 circuit digest and batch statement
contract for BitZ and Limber. BitZ's fixed-width v2 proof digest is kept separate;
the benchmark computes comparison digests from the actual public matrices and
moduli. This fixes the `canonical digest mismatch` caused by reporting the v2
hash as v1. Rerun affected campaigns into a fresh directory to regenerate traces.

```bash
python3 scripts/run_matched_multiswap_campaign.py \
  --draft \
  --security-bits 114 \
  --batch-counts 1,2,4,8,16 \
  --all-threads 10 \
  --warmups 1 \
  --samples 10 \
  --rustflags="-C target-cpu=native" \
  --output-dir "$RUN_DIR/multiswap" \
  2>&1 | tee "$RUN_DIR/multiswap.log"
```

### 6. SHA-256 layout parameter sweep over s and t

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

### 7. Hybrid SHA-256 chain + multiplication modulo 2^32

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

## Tests

```sh
python3 -m unittest discover -s scripts -p 'test_*.py' -v
```

Licenses for the in-tree crates are kept beside their source, and third-party
licenses stay in the forks pinned in `Cargo.toml`.
