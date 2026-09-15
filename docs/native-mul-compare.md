# Independent multiplication modulo 2^32

The comparison proves N=2^L independent rows with unsigned 32-bit x, y, z
and z = x*y mod 2^32. There are no links between successive rows. Select
`u32-mod32` (default); `u32` is an alias. BabyBear has been removed from this
comparison. The optional `plonky3-whir` backend uses the same mod32 AIR as FRI.

## The suite (2026-09-13)

The paper's integer-multiplication campaigns follow one fixed suite: F2Z
(\ftwoz-SNARK) at rates 1/2 and 1/8 (`F2Z_LIG_PROFILE=custom:1:4` /
`custom:3:4`), Binius64 at rates 1/2 and 1/8, Binius64 with the F2Z opener at
rates 1/2 and 1/8 under the ROUND-BY-ROUND accounting
(`F2Z_BINIUS_LIGERITO_ACCOUNTING=rbr`; the exporter rejects union-bound
opener runs), Plonky3-FRI at rate 1/2 (query count solved per size for a
proven round-by-round 100-bit report), and Limber at the pinned 100-bit
Brakedown column-open target (`F2Z_LIMBER_BDLAMBDA`, default 100). Odd
exponents only (15, 17, 19, 21, 23, within each backend's limits), each cell
at BOTH 1 and 10 threads (two campaigns per configuration, joined by the
exporter). Cells that page are not run, except the Binius64-family rows,
which run while paging unless the slowdown is unreasonable. Table naming: no
`\cite` after scheme names; the F2Z rows are `\ftwoz-SNARK`, never
"\ftwoz\ (this work)".

Zinc+ joins the u32 table at rate 1/4 (see below); it is measured outside
this runner, by its own bench in a pinned zinc-plus checkout.

## Zinc+ (u32 only)

f2z-pcs and zinc-plus pin incompatible `crypto-bigint` releases (`=0.7.5`
against `=0.7.0-rc.9`), so Zinc+ cannot be linked into
`benches/mul_e2e_compare.rs`. Instead it is measured by
`protocol/benches/f2z_u32_mod32.rs` inside a private clone of zinc-plus at
`origin/main-beta` (609c18c), and converted into a run directory afterwards:

```sh
# In the clone: one process per (size, threads); 1 thread uses the
# non-parallel build, 10 threads the `parallel` one, as in the MultiSwap row.
CARGO_TARGET_DIR=/tmp/zinc-u32-st cargo bench --no-run --offline \
    --bench f2z_u32_mod32 --features "simd unchecked"
EXPONENT=15 REPS=5 SEED=<the suite corpus seed> <binary>

# In f2z-pcs: campaign directory -> run directory the exporter reads.
python3 scripts/zinc_plus_summary.py bench_results/zinc-plus-u32-<date> \
    --clone <clone> --revision 609c18c --toolchain "$(rustc --version)" \
    --machine-from PerfRuns/suite-u32-f2z-r2-t1 --out PerfRuns/zinc-plus-u32
```

The statement is one integer constraint per multiplication,
`x·y = z + 2^32·w`, over 8 int columns holding the 16-bit limbs of x, y, z
and w, every column range-checked by a `Word { width: 16 }` GKR-LogUp
lookup. Range-checking `w` is what makes it sound: without it
`w = (x·y − z)·2^-32 mod q` satisfies the constraint for any `z`. With all
limbs in range, `|x·y − z − 2^32·w| < 2^65 < q`, so the equation holds over
the integers and `z = x·y mod 2^32` exactly.

The bench derives its operands from `native-mul/mod32/inputs/v1` with the
campaign's seed and recomputes the table's row digest from the limbs it
proved, so its `corpus_digest` equals the other schemes' at every size; the
converter refuses a campaign where the two digests disagree.

Geometry and types, both established by CHECKED runs (the `unchecked`
feature off), not by argument:

- Rows are 8192 columns wide. The IPRS NTT over F65537 needs
  `row_len · inverse_rate < 65537` (2^14 at rate 1/4), and the narrow int
  code — whose base layer and first radix-8 stage run over `i64`/`i128` —
  is exact for 16-bit cells only up to the depth-3 code at 8192.
- The int lane uses the PLAIN IPRS code, not the narrow one. With more than
  one Zip+ row the verifier's `encode_wide` of an alpha-combined row
  (128-bit alphas over 16-bit cells) overflows the narrow lanes; the plain
  code encodes over the combination ring instead.
- The combination ring is `Int<6>` (384 bits). `Int<4>` overflows in the
  prover's row combination and `Int<5>` in the verifier's `encode_wide`.

Security at the suite's 100-bit target: 150 column openings at rate 1/4
(`num_column_openings(4, 100)`), no grinding, a 128-bit projecting prime
drawn from the transcript, and the LogUp range-check term
`127 − log2(8·2^L + 2^16)` ≥ 100 bits for `L ≤ 23`. The opening runs the
generic multi-row path, as decided; it is not the fast single-row path.

## Run

Use Rust 1.98.1. Every backend, Limber included, runs inside the comparison
binary, so one process builds the shared corpus and derives each backend's
witness from it. Limber is proved through the `limber` crate at the pinned
revision in `Cargo.toml`; no sibling checkout and no `LIMBER_REPO` are needed.

`binius64-ligerito` is Binius64's own circuit and PIOP (the same wires and
constraint reductions as `binius64`, including the IntMul reduction's logup*
pushforward oracle) with every oracle committed and opened by F2Z's opener
instead of ring switching + BaseFold: Round 0 (the out-of-domain sample)
right after each commitment, ring switching, and a Johnson-regime Ligerito
opening with fold and query grinding, at the campaign's Binius rate
(`F2Z_BINIUS_LOG_INV_RATE`, default 1 = rate 1/2; 3 = rate 1/8 gives the
paper's second opener row, table key `binius64-ligerito@<rate>`). Its 100-bit
gate is applied under `F2Z_BINIUS_LIGERITO_ACCOUNTING`: `union` (default) is a
whole-protocol union bound over every error term, `rbr` is the round-by-round
minimum (every term on its own at 100 bits, the figure the `f2z` row reports;
table family `binius64-ligerito-rbr`). Either way the opener's per-round target
is solved to the smallest value that clears the gate (105 under the union
bound, 100 round-by-round), and both figures are recorded in every row
(`union_bound_bits`, `round_by_round_bits`); the `binius64` row's 100 bits is
Binius64's query-phase target only. See `src/binius_ligerito/` and
`src/binary_pcs.rs`.

```sh
# Five-backend smoke: L=15, one warmup, five measured proofs, isolated RSS.
bash scripts/run_native_mul_compare.sh

# One suite campaign: odd sizes, 10 threads, F2Z at rate 1/2. Repeat with
# RAYON_NUM_THREADS=1 for the 1-thread group, and with
# F2Z_LIG_PROFILE=custom:3:4 for the rate-1/8 F2Z rows.
F2Z_MUL_COMPARE_BACKENDS=f2z F2Z_BENCH_SHAPES="15 17 19 21 23" \
F2Z_BENCH_REPS=5 RAYON_NUM_THREADS=10 \
bash scripts/run_native_mul_compare.sh

# The Binius64 rows at one rate (1: rate 1/2, 3: rate 1/8), and the F2Z-opener
# rows under the suite's round-by-round accounting.
F2Z_MUL_COMPARE_BACKENDS=binius64 F2Z_BINIUS_LOG_INV_RATE=1 \
F2Z_BENCH_SHAPES="15 17 19 21 23" F2Z_BENCH_REPS=5 RAYON_NUM_THREADS=10 \
bash scripts/run_native_mul_compare.sh
F2Z_MUL_COMPARE_BACKENDS=binius64-ligerito F2Z_BINIUS_LOG_INV_RATE=1 \
F2Z_BINIUS_LIGERITO_ACCOUNTING=rbr \
F2Z_BENCH_SHAPES="15 17 19 21 23" F2Z_BENCH_REPS=5 RAYON_NUM_THREADS=10 \
bash scripts/run_native_mul_compare.sh

# Inspect commands without starting Cargo.
bash scripts/run_native_mul_compare.sh --dry-run
```

`F2Z_MUL_COMPARE_BACKENDS` accepts `f2z binius64 binius64-ligerito plonky3-fri plonky3-whir limber`.
The default is `f2z binius64 binius64-ligerito plonky3-fri limber`. Select WHIR explicitly
to tune it for each run and size; see [WHIR tuning and replay](native-whir-tuning.md).
`F2Z_MUL_COMPARE_OUTPUT_DIR` selects a new, non-existing output directory;
default output is a timestamped directory in `PerfRuns/`.
The complete comparison supports exponents 15..24. Without F2Z, exponents
start at 4. FRI accepts through exponent 29, the Goldilocks FFT domain limit
with log blowup 3. Limber retains its existing exponent-24 runner limit.
Other native cases use address-space and backend domain bounds rather than
a machine-specific RAM cap. Choose sizes that fit the machine being measured.

Limber proves the same wrapping row as the authors' own `int_mult` example:
one modular row `x * y = z_lo (mod 2^w)` per gate, whose quotient is the high
half of the exact `2w`-bit product. Every committed value stays below `2^w`,
so the IntEval limb range check the Mod-PCS already runs is the operand range
check and the program needs no bit columns. The commitment is Brakedown
(`T256DynPrimeBdEngine`), which the crate documents as its comparison
instantiation against code-commitment systems. The adapter is pinned to the
authors' own accounting: with `F2Z_LIMBER_BDLAMBDA=114` (the crate's native
Brakedown default) at `2^15` it reproduces their recorded proof size of
`3417232` bytes exactly, with the same derived IntEval and Brakedown
parameters. The suite instead pins the Brakedown column-open target at 100
bits (`F2Z_LIMBER_BDLAMBDA=100`), the uniform comparison target; the crate's
IntEval (128), challenge (117) and 2^-114 fingerprint terms are constants
and stay above 100. Unlike `int_mult` the gates are independent, matching
the corpus and every other backend here.

Both jobs enforce `RUSTFLAGS=-C target-cpu=native`. `RAYON_NUM_THREADS`
selects a positive thread count, defaulting to ten, and is recorded in
provenance. Use the same count across backends in a campaign; the suite runs
every campaign twice, at 1 and at 10 threads.
The runner clears `CARGO_ENCODED_RUSTFLAGS`, `DUMP`, `CHAIN_BITS`,
`BDSPEC`, `BDROWLEN`, `BDDIRECT`, `BDSPLIT`, and ambient memory-only mode.
Limber's Brakedown column-open target is pinned through the recorded
`F2Z_LIMBER_BDLAMBDA` knob (default 100, the suite's uniform target; 114
reproduces the authors' native policy); an ambient `BDLAMBDA` is rejected so
the campaign always records the value that ran.
`F2Z_BINIUS_LOG_INV_RATE` is not inherited either: an explicit value selects the
Binius rate for the whole campaign (Binius64's BaseFold and the F2Z opener of
`binius64-ligerito` alike), is recorded in `campaign.json`, and must match the
rate the compiled circuit or opener reports. For mod32 it selects the default
rate 1/2 (`1`) or the paper's second row, rate 1/8 (`3`). Effective configurations,
Rust toolchain, build profile, revisions, dirty state, source hashes, lockfile
hashes and machine information are saved. Repository state must remain stable
while a campaign runs. Results made from uncommitted changes record both the
base revision and the hash of the actual source tree; the base alone is not a
reproducible revision of those edits.

## Relations and native security policies

| Backend | Arithmetic relation and cost per operation | Native security configuration |
|---|---|---|
| F2Z | One integer R1CS constraint x*y=P, four 32-bit committed limbs representing x,y,z,w with P=z+2^32*w | Explicit Lambda100, Johnson `custom:1:4` (rate 1/2) or `custom:3:4` (rate 1/8), required Round-0 OOD |
| Binius64 | Bound x,y to 32 bits, native IMUL, mask and equate low 32-bit output; one IMUL plus four word-level ANDs | Explicit 100-bit FRI query target, rates 1/2 and 1/8 |
| Binius64 + F2Z opener | The same circuit and PIOP; every oracle committed and opened by ring switching + Johnson Ligerito with Round 0 | Rates 1/2 and 1/8; round-by-round accounting (`rbr`), every error term gated at 100 bits on its own |
| Limber-Brakedown | One independent wrapping integer-mod row `x*y = z_lo (mod 2^w)`, 3N live witness values padded to 4N, N private quotients (the high halves) | T256DynPrimeBdEngine; `derive_no_limb_split(w,9,L+2)` for `w <= 64`, `derive(128,32,9,L+2)` for `u128`; Brakedown column-open target pinned at 100 bits (`F2Z_LIMBER_BDLAMBDA`); IntEval 128 / challenge 117 / 2^-114 fingerprint unchanged |
| Plonky3-FRI | Two limb equations; 137 columns and 139 constraints per row | Goldilocks, degree-five extension, Poseidon2/MMCS, rate 1/2, the smallest query count whose proven round-by-round report clears 100 bits per size, binary folding, final polynomial length one, zero PoW |
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
report must reach 100 bits before proving and on the returned proof; at
rate 1/2 the query count is solved per size as the smallest clearing that
report (the solved count is recorded in the configuration). WHIR
uses its separate AIR/WHIR security model and records the selected schedule.
Both generate identical trace rows and verify their complete proofs.

F2Z reports geometry, all Ligerito levels, query and folding grinding,
Round-0 grinding and security-accounting terms. Its reported accounting is
round-by-round economic security. Binius reports its query-phase target and
actual compiled counts. Limber preserves native parameter validation with
IntEval target 128 and challenge target 117; its Brakedown column-open
target comes from the pinned `F2Z_LIMBER_BDLAMBDA` (100 in the suite, 114
native). These are documented per-scheme policies, not a derived uniform
complete-protocol bound.

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
  payload `(3*L + 2*(L+2) + 6)*16` bytes, reported as `piop_payload_bytes` in
  its recorded configuration. Encoding conventions are recorded.

Native phase diagnostics remain available. The runner rejects missing,
unverified, mismatched or incomplete records, wrong trial counts, wrong
backends and changed configurations. All gate counts and corpus digests
must match before the campaign is marked complete; the in-process witness
audit checks Limber's recovered rows against the shared corpus digest, like
every other backend.

The run root contains `campaign.json`, unified `summary.json`, `samples.jsonl`
and `metrics.csv`. Traces and logs are under `native/`. Table generation uses the
root results, including their recorded machine rather than the export host:

```sh
python3 scripts/native_mul_table.py PerfRuns/<run-directory> \
  --workload u32-mod32 --out paper/native-mul-table.tex
```

The table follows the SHA+ECDSA format: one row group per size, sub-grouped
by thread count (1 then 10), one row per scheme, bold best per (size,
threads) group and column. Feed the 1-thread and 10-thread campaign
directories together; the exporter joins them by (scheme, size, threads),
requires identical median proof bytes between the thread counts of one
(scheme, size), and rejects union-bound `binius64-ligerito` runs (the suite
renders the round-by-round rows only). `--exponents 15,17,19,21,23` selects
the suite's odd sizes.
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
cargo +1.98.1 test --release --test native_mul_compare \
  --features bench-internals,native-mul-compare
```

Validation covers zero and maximum values, overflow, incorrect low results,
bad carries, bounds, independent indexing, Goldilocks aliases, shared golden
vectors, compiled Binius counts, Johnson/OOD selection, actual FRI/Brakedown
selection, FRI security over supported sizes, WHIR proof rejection for incorrect
results/carries and limb bounds, real proofs and tampering.
Runner tests cover warmup exclusion, repetitions, environment normalization,
structured failures, selectable thread counts, WHIR eligibility/configuration
identity and incompatible result rejection.

The historical validation build used BitZ `b79b869` on `independent-u32-multiplication`
after its rebase and Limber `861f10a6a4d705d92a9faf13a8f860d8ba057ca0` on
`f2z-benching`, with the working-tree changes recorded in the smoke manifest.
Pinned Plonky3: `62f49209aec15ab060c83afbaf9eeb74d8c0c411`.
Its Binius64 revision was `2b27daea4a893fab930259cc7ad59d0a37c2ef95` plus
vendored prover/verifier patches. Exact tested source and Cargo.lock SHA256
values are recorded per result in `provenance`; use those with the recorded
base revisions to identify that build.

Current Binius64 uses the [consolidated fork](binius64-consolidation.md), pinned
at `bc73510ed63bf47eec25d4d10ade84f1c1fe2790` without local Binius patches.
The new compiler emits one AND and three zero constraints per modular u32 row;
negative tests still independently check operand bounds and product correctness.

## Deferred work

- Spartan2 integration.
- Zinc+ beyond u32, and its fast single-row opening path.
- Wider-workload security changes.
- The remaining all-benchmark Johnson/Round-0 OOD audit.
- SHA-chain and SHA+ECDSA comparisons, including smaller SHA sizes.
- Witness-generation improvements.
- Transcript equivalence between implementations.
- A uniform complete-protocol 100-bit bound across all backends.
