# Independent multiplication modulo 2^32

The comparison proves N=2^L independent rows with unsigned 32-bit x, y, z
and z = x*y mod 2^32. There are no links between successive rows. Select
`u32-mod32` (default); `u32` is an alias. BabyBear and the old integrated
Limber multiplication adapter have been removed from this comparison.
The optional `plonky3-whir` backend uses the same mod32 AIR as FRI;
other benchmarks retain their Limber dependencies.

## Run

Use Rust 1.97.1. The sibling `../limber-impl` checkout must contain the new
independent Brakedown `examples/int_mult.rs` on your fork's `f2z-benching`
branch. Set `LIMBER_REPO` to use another checkout. The upstream chain/Hyrax
example is a different workload and the runner rejects its output.

```sh
# Four-backend smoke: L=15, one warmup, five measured proofs, isolated RSS.
bash scripts/run_native_mul_compare.sh

# Five-sample sweep over L=15..20.
F2Z_BENCH_SHAPES="15 16 17 18 19 20" F2Z_BENCH_REPS=5 \
bash scripts/run_native_mul_compare.sh

# Inspect commands without starting Cargo.
bash scripts/run_native_mul_compare.sh --dry-run
```

`F2Z_MUL_COMPARE_BACKENDS` accepts `f2z binius64 plonky3-fri plonky3-whir limber`.
The default remains `f2z binius64 plonky3-fri limber`. Select WHIR explicitly
to tune it for each run and size; see [WHIR tuning and replay](native-whir-tuning.md).
`F2Z_MUL_COMPARE_OUTPUT_DIR` selects a new, non-existing output directory;
default output is a timestamped directory in `PerfRuns/`.
The complete comparison supports exponents 15..24. Without F2Z, exponents
start at 4. FRI accepts through exponent 29, the Goldilocks FFT domain limit
with log blowup 3. Limber retains its existing exponent-24 runner limit.
Other native cases use address-space and backend domain bounds rather than
a machine-specific RAM cap. Choose sizes that fit the machine being measured.

Limber runs this exact command in its own repository:

```sh
RUSTFLAGS="-C target-cpu=native" RAYON_NUM_THREADS=8 \
cargo +1.97.1 run --release --example int_mult -- --bits 32 --log-gates 15
```

Only the exponent varies. `F2Z_BENCH_REPS` defaults to 5 in both repositories.
One invocation does setup, one warmup and all measured trials. The runner
also invokes the example with `F2Z_MUL_MEMORY_ONLY=1` for a fresh, single-proof
memory measurement. That invocation has no warmup. The command remains the
same; Cargo compilation is outside measured intervals and RSS.

Both jobs enforce `RUSTFLAGS=-C target-cpu=native`. `RAYON_NUM_THREADS`
selects a positive thread count, defaulting to eight, and is recorded in
provenance. Use the same count across backends in a campaign.
The runner clears `CARGO_ENCODED_RUSTFLAGS`, `DUMP`, `CHAIN_BITS`, `BDLAMBDA`,
`BDSPEC`, `BDROWLEN`, `BDDIRECT`, `BDSPLIT`, and ambient memory-only mode.
For mod32 it also clears `F2Z_BINIUS_LOG_INV_RATE`. Effective configurations,
Rust toolchain, build profile, revisions, dirty state, source hashes, lockfile
hashes and machine information are saved. Repository state must remain stable
while a campaign runs. Results made from uncommitted changes record both the
base revision and the hash of the actual source tree; the base alone is not a
reproducible revision of those edits.

## Relations and native security policies

| Backend | Arithmetic relation and cost per operation | Native security configuration |
|---|---|---|
| F2Z | One integer R1CS constraint x*y=P, four 32-bit committed limbs representing x,y,z,w with P=z+2^32*w | Explicit Lambda100, Johnson `custom:3:4`, required Round-0 OOD |
| Binius64 | Bound x,y to 32 bits, native IMUL, mask and equate low 32-bit output; one IMUL plus four word-level ANDs | Explicit 100-bit FRI query target, rate 1/2 |
| Limber-Brakedown | One independent integer-mod row, 3N live witness values padded to 4N, N private quotients | T256DynPrimeBdEngine; `derive_no_limb_split(32,9,L+2)`; native approximately 114-bit policy |
| Plonky3-FRI | Two limb equations; 137 columns and 139 constraints per row | Goldilocks, degree-five extension, Poseidon2/MMCS, rate 1/8, 100 queries, binary folding, final polynomial length one, zero PoW |
| Plonky3-WHIR (optional) | The same 137-column, 139-constraint mod32 AIR | Multilinear zerocheck/sumcheck PIOP; Goldilocks; per-run WHIR tuning with evaluated Johnson accounting of at least 100 bits |

Plonky3 splits each operand and output into base B=2^16 limbs and enforces

```
x0*y0 = z0 + B*c0
x0*y1 + x1*y0 + c0 = z1 + B*c1
```

All operand/result limbs and c0 are bounded to 16 bits; c1 is bounded to
17 bits. Thus integer equations cannot acquire Goldilocks wraparound
aliases. There are 129 Boolean checks, eight recompositions and two
arithmetic equations. FRI's pinned library AIR-derived security
report must reach 100 bits before proving and on the returned proof. WHIR
uses its separate AIR/WHIR security model and records the selected schedule.
Both generate identical trace rows and verify their complete proofs.

F2Z reports geometry, all Ligerito levels, query and folding grinding,
Round-0 grinding and security-accounting terms. Its reported accounting is
round-by-round economic security. Binius reports its query-phase target and
actual compiled counts. Limber preserves native parameter validation,
IntEval target 128, challenge target 117, and Brakedown target 114. These are
documented native policies, not a derived uniform complete-protocol bound.

## Shared input and witness audit

Initialize BLAKE3 and update, in order:

1. ASCII bytes `native-mul/mod32/inputs/v1` (no terminator).
2. Seed as little-endian u64, default `0x5533_3250_4353_0064`.
3. Exponent as little-endian u32.

Read successive eight-byte chunks from the XOF. Decode the first and last
four bytes as little-endian u32 x and y. Use their wrapping product for z.
`F2Z_BENCH_SEED` overrides the seed in decimal or hexadecimal.

The canonical digest is BLAKE3 of `native-mul/mod32/rows/v1`, then N as
little-endian u64, then each x,y,z as little-endian u32. Audit native witness
materialization and compare these canonical rows. Quotients and native carry
representations are private and may differ between backends.

Golden digests (tested independently in both repositories):

- L=4: `90f3ca71e3e95a08eb8960a5daea013132ea10b2f69784b2b96604683d417f0d`.
- L=15: `a006d0e2143cfce1ec9dc60dd92be801f48a126b8076d4704cb698e7fd4ac9da`.

At L=4 the first four pairs are `(1210304475,2365989708)`,
`(1744110415,2023825938)`, `(587259919,3206144740)`, and
`(686439776,1601866294)`.

## Measurement and output contract

Public setup occurs once per backend and size. Every trial regenerates its
native witness, proves, and verifies. The first trial is an in-process warmup
and is excluded from medians. Five measured trials are the default.

- `setup_ms`: public setup, measured separately.
- `witness_ms`: native witness generation. Binius includes packing performed
  inside its prover, so this metric overlaps its prover metric.
- `online_prover_ms`: commitment-inclusive proving.
- `witness_to_proof_ms`: directly measured witness-to-complete-proof interval;
  do not construct it by adding potentially overlapping metrics.
- `post_proof_ms`: proof serialization and accounting after the PCS proof is ready
  (native backends), excluded from `witness_to_proof_ms`.
- `verify_ms`: complete native verification.
- `peak_rss_bytes`: fresh-process high-water RSS across corpus generation,
  setup, witness generation, commitment, proof, verification and size accounting.
  Linux uses VmHWM; macOS uses getrusage bytes. Disable only with
  `F2Z_MUL_COMPARE_MEMORY=0`, which leaves memory absent.
- `proof_bytes`: complete transmitted payload including initial commitments.
  Binius uses native transcript bytes; Plonky3 uses the complete postcard
  proof encoding. F2Z includes the root, canonical opening and analytically
  counted fixed-width PIOP elements/nonces. Limber includes canonical input
  commitments, canonical eval argument and an analytically counted sumcheck
  payload `(3*L + 2*(L+2) + 6)*16` bytes. Encoding conventions are recorded.

Native phase diagnostics remain available; Limber reports the common totals
without inventing separate internal phases. Limber emits structured
`LIMBER_MUL_RESULT` / `LIMBER_MUL_MEMORY` JSON. The runner rejects missing,
unverified, mismatched or incomplete records, wrong trial counts, wrong
backends and changed configurations. All four gate counts and corpus digests
must match before the campaign is marked complete.

The run root contains `campaign.json`, unified `summary.json`, `samples.jsonl`
and `metrics.csv`. Native traces/logs are under `native/`; Limber logs and
normalized summaries are under `limber-int-mult/`. Table generation uses the
root results, including their recorded machine rather than the export host:

```sh
python3 scripts/native_mul_table.py PerfRuns/<run-directory> \
  --workload u32-mod32 --out paper/native-mul-table.tex
```

Multiple directories extend the size sweep. Overlapping rows or
`--proof-sizes-from` imports must match workload version, protocol/config,
source/build fingerprint, corpus, machine and measurement policy. Historical
chain, Hyrax, WHIR and full-product results are not relabeled or imported.
The exporter retains size selection, memory-bound exclusions, rate labels
and proof-size columns.

## Wider workloads

The existing full-product u64 and u128 relations, corpus generation and
F2Z/Binius implementations are preserved. Run them separately:

```sh
F2Z_MUL_COMPARE_WORKLOADS=u64 F2Z_MUL_COMPARE_BACKENDS="f2z binius64" \
F2Z_BENCH_SHAPES="15 16 17" bash scripts/run_native_mul_compare.sh

F2Z_MUL_COMPARE_WORKLOADS=u128 F2Z_MUL_COMPARE_BACKENDS=binius64 \
F2Z_BINIUS_LOG_INV_RATE=3 F2Z_BENCH_SHAPES="15 16 17" \
bash scripts/run_native_mul_compare.sh
```

F2Z requires exponents of at least 15. Upper limits follow address-space and
backend domain bounds; they do not assume a particular RAM capacity. The wider
Binius rate override remains available and is represented separately in tables.

## Validation and tested sources

```sh
python3 -B -m unittest discover -s scripts -p test_native_mul_runner.py -v
RUSTFLAGS="-C target-cpu=native" RAYON_NUM_THREADS=8 \
cargo +1.97.1 test --release --test native_mul_compare \
  --features bench-internals,native-mul-compare
# In the Limber checkout:
RUSTFLAGS="-C target-cpu=native" RAYON_NUM_THREADS=8 \
cargo +1.97.1 test --release --example int_mult
```

Validation covers zero and maximum values, overflow, incorrect low results,
bad carries, bounds, independent indexing, Goldilocks aliases, shared golden
vectors, compiled Binius counts, Johnson/OOD selection, actual FRI/Brakedown
selection, FRI security over supported sizes, WHIR proof rejection for incorrect
results/carries and limb bounds, real proofs and tampering.
Runner tests cover warmup exclusion, repetitions, environment normalization,
structured failures, selectable thread counts, WHIR eligibility/configuration
identity and incompatible result rejection.

The implementation builds on BitZ `b79b869` on `independent-u32-multiplication`
after its rebase and Limber `861f10a6a4d705d92a9faf13a8f860d8ba057ca0` on
`f2z-benching`, with the working-tree changes recorded in the smoke manifest.
Pinned Plonky3: `62f49209aec15ab060c83afbaf9eeb74d8c0c411`.
Pinned Binius64: `2b27daea4a893fab930259cc7ad59d0a37c2ef95` plus the existing
vendored prover/verifier patches. Exact tested source and Cargo.lock SHA256
values are recorded per result in `provenance`; use those with the recorded
base revisions to identify this uncommitted implementation.

## Deferred work

- Spartan2 integration.
- Wider-workload security changes.
- The remaining all-benchmark Johnson/Round-0 OOD audit.
- SHA-chain and SHA+ECDSA comparisons, including smaller SHA sizes.
- Witness-generation improvements.
- Transcript equivalence between implementations.
- A uniform complete-protocol 100-bit bound across all backends.
