
# BitZ 🫜 --- README for normal humans

BitZ proves

```
MLE[w](r) = y ∈ F_q
```

for w a vector of bits.

Run it with:


```sh
RUSTFLAGS="-C target-cpu=native" cargo run --release --features unchecked -- 24
RUSTFLAGS="-C target-cpu=native" cargo run --release --features unchecked -- \
    28 --threads 1 --reps 5 --profile custom:3:4
```

`f2z <n> [<t> <s> [<W>]] [--threads N] [--reps R] [--profile P] [--word-bits W]`:

`n` is log(|w|)

## Integer R1CS with F_2 virtualization

Pick the security parameter with `F2Z_BENCH_LAMBDA`. Every bench below
honours it, so one bench can be run at exactly one λ:

| `F2Z_BENCH_LAMBDA` | profile | meaning |
|---|---|---|
| `100` | `Lambda100` | no grinding anywhere; every term this crate controls ≥ 100 bits |
| `128` | `Lambda128` | every controllable term ≥ 128 bits (two grinding bits per forest round, one at the ring switch; the GF(2^128) floor at ~126.4 still binds and is reported) |
| `114` | `Limber114` | the two-prime MultiSwap/Limber comparison target — MultiSwap only |
| `sha128-reference-schedule` | `Sha128ReferenceSchedule` | the historical SHA-256 128-bit schedule, kept for comparison |

The profile names are accepted too (`F2Z_BENCH_LAMBDA=lambda128`). Unset,
SHA-256 and u32×u32 run at λ=100, MultiSwap at 114, and the BabyBear and
`lambda_sweep` benches run every profile they know (two and three rows per
shape). A profile the bench's relation cannot instantiate — `114` outside
MultiSwap, or `100`/`128` on MultiSwap — aborts up front with the
admissible list. Each `RESULT` line carries `profile=<name>` next to
`lambda=<bits>`.

### MultiSwap — (2 modular exponentiations on a 2048 bit RSA modulus + Poseidon hashing), λ=114:
```sh
F2Z_BENCH_LAMBDA=114 RAYON_NUM_THREADS=1 RUSTFLAGS="-C target-cpu=native" \
  cargo bench --bench multiswap --features unchecked
```

### SHA-256 — λ=100 bits of security (`F2Z_BENCH_LAMBDA=128` for the 128-bit profile)
```sh
F2Z_BENCH_LAMBDA=100 F2Z_BENCH_SHAPES=14 F2Z_BENCH_REPS=3 RUSTFLAGS="-C target-cpu=native" \
  cargo bench --bench sha256_compressions --features unchecked
```

Size the same benchmark by the packed assignment domain (`MnumRows=2^n`)
instead of a power-of-two compression count with:

```sh
F2Z_BENCH_LAMBDA=100 F2Z_SHA_MNUMROWS_LOG2S="24 25" F2Z_BENCH_REPS=3 RUSTFLAGS="-C target-cpu=native" \
  cargo bench --bench sha256_compressions --features unchecked
```

This uses `floor((2^n - 1) / 20456)` compressions: one shared constant,
20,456 adjacent assignment cells per compression, and one trailing zero
suffix only.

`F2Z_SHA_OPENING_T=<t>` (compression-count shapes only) replaces the direct
product opening by the inner-sumcheck path with an explicit F2Z split of
`2^t` rows × `2^(vars − t)` columns for the opening. The default product
layout pins `t = min(k, 13)` (rows = instances, columns = the 2^15 local
cells), so its read-off vector — the `2^s` ~125-bit integers sent in the
clear — is 327 KB at 2^12–2^13 and doubles per step from 2^14 on. A larger
`t` shrinks that vector but crosses the one-forest cap (`127 − t − 1 <
q_bits`): the opening then runs one merged forest per weight chunk, i.e.
about twice the forest prover time. The bench prints the layout, forest
count and read-off width per shape.

### u32×u32 -> u64 — λ=100; exponents ≥ 15:
```sh
F2Z_BENCH_LAMBDA=100 F2Z_BENCH_SHAPES="15 20" F2Z_BENCH_REPS=5 RUSTFLAGS="-C target-cpu=native" \
  cargo bench --bench u32_mul --features unchecked
```

### Babybear mult — λ=100; exponents ≥ 15 (unset `F2Z_BENCH_LAMBDA` = a λ=100 and a λ=128 row per shape):
```sh
F2Z_BENCH_LAMBDA=100 F2Z_BENCH_SHAPES="15 20" F2Z_BENCH_REPS=5 RUSTFLAGS="-C target-cpu=native" \
  cargo bench --bench baby_bear_mul --features unchecked
```

### SHA security-profile sweep: Lambda100 / Sha128ReferenceSchedule / Lambda128 (set `F2Z_BENCH_LAMBDA` for one of them):
```sh
F2Z_BENCH_SHAPES=12 F2Z_BENCH_REPS=3 RUSTFLAGS="-C target-cpu=native" \
  cargo bench --bench lambda_sweep --features unchecked
```

### PCS-only (t:s:W triples; no IOP security profile, so `F2Z_BENCH_LAMBDA` does not apply; profiling stays opt-in here — add OBLONG_PROFILE=1 for the phase line):
```sh
F2Z_BENCH_SHAPES="17:11:1" F2Z_BENCH_REPS=5 RUSTFLAGS="-C target-cpu=native" \
  cargo bench --bench pcs --features unchecked
```

## Dependencies

- https://github.com/albert-garreta/flock-mod
- https://github.com/worldfnd/f2z-benchmark
- [Albert: I'm not sure what this is. Leaving it here just in case] **`crypto-primitives`** — vendored at `vendor/crypto-primitives` (NethermindEth, Apache-2.0; see `vendor/crypto-primitives/VENDORED.md` for
  the pinned revision and the crypto-bigint 0.7.5 / rand 0.10 port).


# AI SPAGHETTI README
F2Z is a polynomial commitment scheme for **integer data committed over a
cheap characteristic-2 code**: it proves

```
MLE[INT(D)](r) = y ∈ F_q
```

for a `2^t × 2^s` matrix `D` of `W`-bit cells whose **bits** are committed over
`F_2`. The integer row-fold is carried **in the exponent** of
`K = GF(2^128)` — `α^{v_c} = ∏_b α^{w_b·D[(b,c)]}`, each factor affine in one
committed bit — certified by a GKR grand-product forest that touches the
commitment only through `K`-linear queries; the column combination
`y = Σ_c e_c·v_c` is read off in the clear over `F_q`. The construction is due
to Lev Soukhanov (the char2-fieldswitch note, §9).

Why two fields: `K = GF(2^128)` binds each `v_c` in the exponent (no parity
collapse — `v_c` is sent and reused as an integer, never reduced mod 2), while
the evaluation field `F_q` (any characteristic ≠ 2 ring) carries the final
combination on the sent integers. Full-width `F_q` row-weights are **chunked**
(`L = ⌈q_bits/c_w⌉` sub-folds, each bound injectively below `ord α`) and
recombined in the clear, so the scheme is a genuine `F_q` MLE opening over an
unchanged F₂ commitment.

## Opener: ring-switch + recursive Ligerito

The **only** PCS opener is the flock-backed **ring-switch + recursive Ligerito**
pipeline. The committed bit-matrix is packed 128 bits per `GF(2^128)` element
and RS-encoded / Merkleized by `flock-core` (Succinct Labs' *Flock*). Each
mod-`q` limb chunk contributes a **merged product forest** (the `α^{v_c}`
binding) plus a de-black-boxing degree-2 **pre-sumcheck** that strips the
α-power weight factor, leaving a residual bit-MLE evaluation claim
`M̂(r*, ξ) = μ`. A **ring-switch** (Diamond–Posen / Flock App. B) turns that
into an inner-product claim on the packed polynomial, and the `L` chunk claims
are `η`-batched into **one** recursive Ligerito call whose closing residual is
evaluated succinctly by a tensor-algebra trick (`O(m·128²)`, no `2^m`-sized
table). Everything hot — commit, additive-NTT encode, Ligerito folding
+ Merkle — runs `flock-core`'s optimized code; the ring-switch and the GKR
forest run over the native `GF(2^128)` type. The earlier flat linear-code
opening has been removed.

## Usage

```rust,ignore
use f2z::pcs::{IntEvalParams, smallest_generator};
use f2z::ligerito::packed_vars;
use f2z::ligerito_flock::{
    IntEvalRsLigModQProof, LigConfig, commit_rs_flock_with, lig_configs,
    prove_mle_eval_mod_q_ligerito, verify_mle_eval_mod_q_ligerito,
};
use f2z::transcript::Blake3Transcript;

// Instance shape: 2^t × 2^s cells of `word_bits`-bit integers.
let p = IntEvalParams { t: 10, s: 6, word_bits: 1 };
let alpha = smallest_generator();                 // any generator of GF(2^128)^×
let (pc, vc) = lig_configs(
    packed_vars(&p),
    LigConfig::Adhoc { log_batch: 2, log_inv_rate: 2 },
)?;

// Commit the bits (root published before any challenge).
let hint = commit_rs_flock_with(&p, &data, pc.log_inv_rates[0], pc.initial_k);

// Prove MLE[INT(D)](r) = y ∈ F_q  (row_weights_q = eq(r₁,·) lifted into [0,q)).
let mut pt = Blake3Transcript::new();
let proof =
    prove_mle_eval_mod_q_ligerito(&mut pt, &hint, &p, &row_weights_q, q_bits, alpha, &pc);

// Serialize the complete proof (zinc parts + length-prefixed bincode LigeritoProof).
let bytes = proof.to_bytes();
let proof = IntEvalRsLigModQProof::from_bytes(&bytes)?;

// Verify against the commitment and the claimed y (R = any char-≠2 ring).
let mut vt = Blake3Transcript::new();
verify_mle_eval_mod_q_ligerito(
    &mut vt, &hint.commitment, &proof, &p,
    &row_weights_q, &col_weights, alpha, y, q_bits, &vc,
)?;
```

`ligerito_flock::tests::mle_eval_mod_q_ligerito_roundtrips` is the end-to-end
reference (a concrete 100-bit prime `F_q = 2^100 − 15`, in the 1-chunk `W=1` and
2-chunk `W=32` regimes); `examples/reference_measure.rs` prints prove/verify
timings and serialized proof sizes.

## Proof-stream serialization

`IntEvalRsLigModQProof::to_bytes` / `from_bytes` are the host proof codec. The
zinc-side parts (the per-chunk merged forests, chunk folds `u`, pre-sumchecks,
and ring-switch `s_v` messages) are written field by field via
[`proof_codec`](src/proof_codec.rs) — little-endian scalars, with the sumcheck
proofs reusing the crate's length-prefixed `Transcribable` encoding. flock's
serde `LigeritoProof` rides as a single **length-prefixed `bincode` 1.3 blob**
(bincode 1.3 being flock's own pinned encoder). The codec is canonical
(re-serialization is byte-identical) and rejects tampered bytes — either the
stream fails to decode, or the reconstructed proof fails verification (pinned by
`mod_q_ligerito_proof_serialization_roundtrips`).

The chunk folds `u` are length-prefixed at their **live** width, not `2^s`:
everything past the last non-zero fold is omitted and re-padded by the
verifier. Those entries are the all-zero columns of a padded witness, whose
roots are `α^0 = 1`, so the decoded `us` is bit-for-bit the prover's — this
is a shorter encoding of the same proof object, with the same soundness
surface (a prover could always have sent the zeros explicitly). Canonicity
is enforced in both directions: `from_bytes` rejects a transmitted trailing
zero with `CodecError::NonCanonical`, and bounds the declared fold count by
the bytes actually remaining before reserving. At φ = 0.55 this is −8.1 % of
the proof at n = 28 (182 116 → 167 364 B) and −12.2 % at n = 30
(241 132 → 211 628 B); full-width witnesses serialize exactly as before.
Pinned by `mod_q_ligerito_padded_witness_trims_us`.

## Dependencies

- **`flock-core`** — the ring-switch / additive-NTT / Ligerito hot
  paths (Succinct Labs' *Flock*, `MIT OR Apache-2.0`). `Cargo.toml` pins the
  published modified fork by revision:
  [`albert-garreta/flock-mod`](https://github.com/albert-garreta/flock-mod)
  (upstream plus the k=4 L0 interleaving, the Slim3 rate-1/8 profile, and the
  lookahead exports the precomputed-round-0 API needs), so a fresh clone
  builds with no sibling checkout. For local flock development, override the
  dependency with an uncommitted `path` edit or a `[patch]` entry.
  `flock-core` itself pins `bincode 1.3` and `serde 1`. Note flock's prover
  retains large scratch buffers across proves
  (`ligerito_flock::flock_scratch_clear` releases them).
- **`crypto-primitives`** — vendored at `vendor/crypto-primitives`
  (NethermindEth, Apache-2.0; see `vendor/crypto-primitives/VENDORED.md` for
  the pinned revision and the crypto-bigint 0.7.5 / rand 0.10 port).
- **`circuit`** — backend-independent SHA-256/F2Z circuit synthesis copied
  into `crates/circuit`, with its matrix-field support in `crates/field`.
  A fresh checkout therefore needs no sibling `f2z-benchmark` repository;
  provenance and the pinned upstream revision are recorded in
  `crates/circuit/VENDORED.md`.
- `crypto-bigint 0.7.5`, `crypto-primes`, `blake3`, `rayon`.

## Building and testing

```sh
RUSTFLAGS="-C target-cpu=native" cargo test --release
RUSTFLAGS="-C target-cpu=native" cargo run --release --example reference_measure
# both merged-forest schedules are pinned byte-identical to the eager forest:
RUSTFLAGS="-C target-cpu=native" F2_FOREST_SCHEDULE=l8 cargo test --release \
  merged_forest
# the copied circuit crate's full tests and benchmark builds:
RUSTFLAGS="-C target-cpu=native" cargo test --release --all-features --locked \
  --manifest-path crates/circuit/Cargo.toml
RUSTFLAGS="-C target-cpu=native" cargo bench --all-features --no-run --locked \
  --manifest-path crates/circuit/Cargo.toml
```

`-C target-cpu=native` is load-bearing on aarch64 (enables PMULL for the NEON
`GF(2^128)` pipeline and flock's NEON kernels). Features: `parallel` (default,
rayon), `unchecked` (release-style integer guards off). The suite covers the
merged-forest lazy-vs-eager byte-identity (both `F2_FOREST_SCHEDULE` schedules),
the mod-`q` Ligerito roundtrip with tamper / range / generator rejections, the
NEON-vs-scalar field equivalence (`neon_mul_matches_scalar_pipeline`), and the
serialization roundtrip + tampered-byte rejection.

For profilers (`sample`/`samply`/Instruments), `--profile profiling` builds
release codegen plus DWARF in its own target subdirectory, so alternating
profile/measure runs never invalidates the release cache.

### CLI runner (`f2z`)

`src/bin/f2z.rs` is a one-shot commit / prove / verify runner for a single
shape — the runnable sibling of `benches/pcs.rs` (it's the package's only
binary, so plain `cargo run` targets it):

```sh
RUSTFLAGS="-C target-cpu=native" cargo run --release --features unchecked -- 24
RUSTFLAGS="-C target-cpu=native" cargo run --release --features unchecked -- \
    28 17 11 --threads 1 --reps 5 --profile slim
```

`f2z <n> [<t> <s> [<W>]] [--threads N] [--reps R] [--profile P] [--word-bits W]`:

- `n` — cell-index MLE variables, `n = t + s` (`2^n · W` committed bits).
  Omitting `t s` uses the reference split `t ≈ 0.6n` (clamped to the
  packing constraint `t + log₂W ≥ 7`; a note is printed if the shape
  forces `L > 1` chunks). `W` as a fourth positional sets the cell width
  (power of two, default 1; equivalent to `--word-bits`) — e.g. the
  reference W=32 shape: `f2z 12 4 8 32`.
- `--family j2|j3|j4` — run the EXPERIMENTAL mod-q **RLC claim family**
  at this `n` instead of the single-claim opening (`j2` = the XOR triple,
  k=3 claims on `m₁, m₂, m₁⊕m₂`; `j3` = k=4 with the 3-way XOR; `j4` =
  k=5). W is fixed at 1 and the shape is the measured A/B layout (4 UAIR
  columns, x-tensor split `t' ≈ s` — comparable with the 2026-07-26/27
  RLC notes); `t s W` positionals do not apply; every rep is verified.
  E.g. `f2z 26 --family j2 --reps 5`. The full A/B against the
  virtual-XOR and independent baselines stays in `examples/rlc_ab.rs`
  (`F2Z_AB_N`/`F2Z_AB_REPS`, plus `F2Z_AB_SINGLES=1` and `F2Z_AB_J3=1`
  modes).
- `--taps vx|family|collapse|rotxor` — run the EXPERIMENTAL
  **structured-taps** paths at this `n` (32-bit words along the ENTRY
  axis of W=1 bit-vectors, `g = 5`; 2 UAIR columns; ALL claims at ONE
  shared point; needs n ≥ 14): `vx` = the j=2 k=6 ROT/SHIFT/word-offset
  instance through the batched tap-claims (extraction + translated-eq
  openings) path; `family` = the same instance through the clustered
  stream family; `collapse` = the instance's 13 deduped streams as 13
  SINGLE-TAP claims through the weight-transform collapse (≤ 4 inner
  claims); `rotxor` = 8 uniform-op-of-XOR-set claims — `ROT^c(a₁⊕a₂)`
  rotations, a word-offset of the pair, single-column rotations —
  through the same collapse (`TapPointClaim` now carries an XOR column
  set; 4 inner claims; measured 79.5/199.4 ms at n=22/24, ≈ 2–3× ONE
  claim for all 8). Every rep is verified. E.g. `f2z 24 --taps rotxor
  --reps 5`. The full A/B (with independent baselines, phase trees, seeds)
  stays in `examples/taps_ab.rs` (`F2Z_AB_N`/`F2Z_AB_REPS`/
  `F2Z_TAPS_SEED`, `F2Z_AB_COLLAPSE=1` for the collapse demo,
  `OBLONG_PROFILE=1` for phase trees).
- `--threads N` / `-j N` — rayon pool size (`1` = single-threaded;
  default all cores / `RAYON_NUM_THREADS`).
- `--reps R` — timing repetitions (medians reported; **every rep is
  verified**; default 3).
- `--profile` — Ligerito config, resolved exactly like the bench: `slim`
  (default; rate 1/4, k=4) / `slim3` (rate 1/8, k=4) / `fast` (rate 1/2,
  k=4) / `secure` (embedded profiles at `m = n ≥ 22`) or
  `custom:<log_inv_rate>:<initial_k>` (validator-gated Johnson geometry);
  below `m = 22` everything falls back to the ad-hoc test config
  (UNAUDITED).
- Integer guards are a **compile-time** feature: build with
  `--features unchecked` for quotable numbers — the header self-reports
  the active mode and warns otherwise.

Output is one self-describing header (resolved geometry, thread count,
guard mode) plus commit / prove / verify medians, peak heap (the same
live-heap high-water notion as the bench), and the proof-size split
(shown here for `--profile fast`):

```text
f2z: n=24 (t=15, s=9, W=1, m_p=17, chunks=1) | lig=fast@r1/2k4 | threads=10 | int guards: unchecked
commit:       2.27 ms   peak    12.10 MB
prove:       62.78 ms   peak    85.01 MB   (median of 3, verified)
verify:       2.43 ms
proof:       152.9 KiB  (forest-side 22.2 | s_v 2.0 | ligerito 125.4)
```

### Reference measurement

Single machine (Apple M4, `-C target-cpu=native`, median of 5), one genuine
`F_q = 2^100 − 15` MLE opening under the pass-fusion defaults
(`examples/reference_measure.rs` — a quick, non-idle-fronted run; the
protocol-grade tables live under "Reference numbers" below):

| shape | prove | verify | serialized proof |
|---|---|---|---|
| **n=16** (t=10, s=6, W=1, 1 chunk) | **4.4 ms** | **1.7 ms** | **43.2 KiB** |
| n=18 (t=12, s=6, W=1, 1 chunk) | 5.7 ms | 1.5 ms | 75.0 KiB |
| (t=4, s=8, W=32, 2 chunks) | 7.8 ms | 2.5 ms | 65.7 KiB |

The n=18 proof size (75.0 KiB) matches the source branch's ~71 KiB Ligerito
proof at n=18.

## Benchmarks

**Unified output schema.** The protocol benches (`multiswap`,
`sha256_compressions`, `u32_mul`, `pcs`, `lambda_sweep`) share one
accounting model and one machine-readable `RESULT schema=f2z/1 …` line —
see `docs/bench-schema.md`. The end-to-end prover includes bit-packing,
commitment, prime sampling + grinding, the PIOP, bitification, Step 5.0,
and the F2Z opening; witness generation and one-time preprocessing are
excluded and reported separately. Each bench prints per-step prover and
verifier breakdowns keyed to the paper's §2.1 steps, summing to their
totals with an explicit residual. Canonical env knobs are
`F2Z_BENCH_REPS` / `F2Z_BENCH_SHAPES` / `F2Z_BENCH_SEED` (old per-bench
names remain as deprecated aliases), and **any unknown `F2Z_*` variable
aborts the bench** with the known-knob list.

**Security profiles.** The IOP security level is a compile-time profile
(`src/piop/spartan/profile.rs`): `Lambda100` (the default — no grinding
anywhere), `Lambda128` (every term this crate controls ≥ 128 bits,
including two bits of forest/GKR grinding per round; the flock-internal
GF(2^128) floor at ~126.4 still binds and is reported as such),
`Limber114` (the MultiSwap/Limber comparison, pinned), and
`Sha128ReferenceSchedule` (the historical SHA parameter schedule, retained
only as an explicit comparison profile and pinned by `tests/transcript_pins.rs`). Every interval
width and grinding difficulty is *derived* from the target plus the shape
facts, and each instantiation carries a per-term soundness accounting
(`achieved bits` + the binding term), printed by the benches.
`F2Z_BENCH_LAMBDA=100|114|128|sha128-reference-schedule` selects the
profile a run measures at — every protocol bench honours it (the
profiles stay compile-time types; the knob picks which monomorphized body
runs), and a profile the bench's prime strategy cannot instantiate aborts
with the admissible list. The `lambda_sweep` bench proves one SHA witness
under all three SHA profiles — the 100-vs-128 prover-time/proof-size
tradeoff table — or under the one `F2Z_BENCH_LAMBDA` names.

`benches/pcs.rs` (plain `harness = false` binary, no criterion) reports, per
shape: commit / prove / verify wall-clock (medians), serialized proof size,
codec round-trip time, **peak heap** per phase — the live-heap high-water
("net outstanding bytes"), the same notion as flock's benches and zinc-plus's
`f2_int_ligerito_mem`, so the numbers compare directly across the three
repos — and a proof-size `split:` line (forest-side vs ring-switch `s_v` +
Ligerito). Under `OBLONG_PROFILE=1` it additionally prints a `phases:` line
(forest+presum vs ligerito open, from one profiled prove per shape; the
timed medians then carry ~µs-scale scope overhead — leave it off for
headline timing).

```sh
RUSTFLAGS="-C target-cpu=native" cargo bench --bench pcs --features unchecked
# specific shapes (t:s:W triples) and rep count:
F2Z_BENCH_SHAPES="10:6:1 14:8:1" F2Z_BENCH_REPS=5 \
  RUSTFLAGS="-C target-cpu=native" cargo bench --bench pcs --features unchecked
# a witness that is NOT a power of two: fill fraction φ ∈ (0, 1] — the
# trailing (1−φ) of the columns are left all zero, i.e. the padding a
# witness of N = φ·2^n cells carries (see `F2Z_COL_ELIDE` below):
F2Z_BENCH_FILL=0.55 F2Z_BENCH_SHAPES="17:11:1" F2Z_BENCH_REPS=3 \
  RUSTFLAGS="-C target-cpu=native" cargo bench --bench pcs --features unchecked
```

The shape header reports the fill as `live=<C>/<2^s>`, and the `proof:`
line carries an `fnv` fingerprint of the serialized bytes — the handle for
byte-identity A/Bs across prover knobs.

`scripts/bench_csv.sh` sweeps shapes × profiles one process at a time (the
measurement protocol) and writes one CSV row per run —
`scripts/bench_csv.sh -p "fast,slim,slim3" --phases` covers the reference
shapes; `--big` appends n=30–32 (memory-healthy box required), `-j 1`
single-threads, `-h` for all knobs. Output lands in `bench_results/`.

Prover knobs (every configuration produces byte-identical proofs):
`F2Z_EQF_FUSE=0` disables pass fusion (restores the eager two-pass fold);
`F2Z_LUT3=0` disables the deeper L/4 LUT prefixes; `F2Z_EQF_NOKERNEL=1`
forces the generic (non-NEON) round/fold kernels (diagnostic);
`F2Z_PAIR2_FACTORED=0` restores the precombined 16-case LUT round tables
(the factored default trades two extra wide multiplies per slot for 4×
less table footprint — measured −9–15 % prove at n = 28, see the dated
note under the reference numbers); `F2_FOREST_SCHEDULE=l8` opts into the
L/8 forest memory schedule; `F2Z_COL_ELIDE=0` disables **live-column
elision** — the default builds only the leading columns that carry data
and collapses the trailing all-zero ones (a zero-padded witness's padding,
since the column index is the high-order MLE index) into ONE synthetic
constant-1 group carrying their summed `eq` weight, which is exact in
char 2. A witness of `N = φ·2^n` cells therefore pays the forest for
`⌈N/2^{t+log₂W}⌉` columns instead of `2^s`: at φ = 0.55 measured
**−34.6 % prove / −35.0 % peak at n = 28** (497.6 → 325.6 ms,
1360.9 → 884.3 MB) and −50.8 % / −35.4 % at n = 30 (the n = 30 arm
overshoots the compute-proportional ~−40 % because the un-elided run at
5.4 GB is memory-pressured — quote −35–40 % as the compute win). The
elision granularity is one column, so the residual padding waste is under
`2^{t+log₂W}` cells (0.05 % of `2^28` at t = 17). Pinned byte-identical by
`col_elision_matches_full`.

`F2Z_LIG_PROFILE` (bench-only, changes the proof: `slim` default / `fast`
/ `secure` / `r8` = ad-hoc UDR rate-1/8 probe /
`custom:<log_inv_rate>:<initial_k>`) selects the Ligerito profile; the
shape header prints the resolved geometry (`lig=slim@r1/8k4`) — see the
RS rate study below.

Performance parity with upstream: the release profile carries the upstream
`lto = true` / `codegen-units = 1` (without them the vendored field kernels
lose cross-unit inlining — measured ~1.2–1.5× slower), and `--features
unchecked` mirrors the upstream bench convention (plain integer ops; the
default build keeps overflow guards). At a matched shape (n=22, t=13, s=9,
single mod-q claim, same box, interleaved runs) F2Z proves in **20.9 ms** vs
the upstream 2-col Base arm's **18.2–18.8 ms** — within ~1.1×, the residual
being harness and layout differences rather than the PCS.

### Reference numbers

Apple M4 (16 GB); `t ≈ 0.6n` splits, W=1, one mod-q claim; medians,
idle-fronted, one shape per process at n ≥ 24, 45–90 s cooldowns between
shapes; the **pass-fusion + deeper-LUT defaults** (`F2Z_EQF_FUSE` /
`F2Z_LUT3`, opt-out `=0`) and the **fast profile at its current (rate
1/2, k=4) generation** (n ≥ 22; n < 22 is the ad-hoc config). `schedule`
is the forest memory schedule (`F2_FOREST_SCHEDULE=l8` opt-in, required
at n ≥ 31 on 16 GB). The `forest` / `open` phase columns come from ONE
profiled prove per shape (`OBLONG_PROFILE=1`; forest = pow2 + forest GKR
+ fold-v + pre-sumcheck, open = ring-switch + B-combination + recursive
Ligerito; they sum to within ~2 % of the prove median except where
noted). Rows marked † are the earlier (rate 1/2, k=6) fast generation,
pending a re-measure on a memory-fresh box.

**Prover time** (`prove` = median; phases from the profiled prove):

| n | shape (t, s) | commit | forest+presum | ligerito open | prove | verify | prove peak | schedule |
|---|---|---|---|---|---|---|---|---|
| 16 | 10, 6 | 0.46 ms | 3.25 ms | 0.72 ms | 4.03 ms | 1.21 ms | 0.69 MB | L/4 |
| 18 | 12, 6 | 0.37 ms | 5.68 ms | 0.93 ms | 6.85 ms | 1.43 ms | 2.4 MB | L/4 |
| 20 | 13, 7 | 0.80 ms | 9.13 ms | 1.62 ms | 10.9 ms | 1.83 ms | 7.7 MB | L/4 |
| 22 | 14, 8 | 0.85 ms | 17.2 ms | 8.07 ms | 24.9 ms | 2.09 ms | 23.1 MB | L/4 |
| 24 | 15, 9 | 2.03 ms | 42.5 ms | 21.5 ms | 62.1 ms | 2.48 ms | 85.0 MB | L/4 |
| 26 | 16, 10 | 6.35 ms | 131 ms | 25.3 ms | 159 ms | 3.14 ms | 325 MB | L/4 |
| 28 | 17, 11 | 24.7 ms | 530 ms | 53.6 ms | 579 ms | 4.41 ms | 1.27 GB | L/4 |
| 30† | 18, 12 | 88.1 ms | 2.98 s | 139 ms | 3.15 s | 5.55 ms | 4.95 GB | L/4 |
| 31† | 19, 12 | 180 ms | 8.51 s | 314 ms | 8.92 s | 7.83 ms | 5.81 GB | l8 |
| 32† | 19, 13 | 390 ms | ~96 % | ~4 % | 29.8 s | 11.8 ms | 11.5 GB | l8 |

(n=32's profiled prove ran thermally shaded — 35.9 s vs the 29.8 s
median — so its phases are quoted as shares.)

**Proof size** (transmitted-payload accounting; forest-side = forest
sumchecks/evals + chunk folds + pre-sumchecks, open-side = ring-switch
`s_v` + the recursive Ligerito proof; `total` = the serialized stream):

| n | forest-side | s_v | ligerito | total |
|---|---|---|---|---|
| 16 | 7.7 KiB | 2.0 KiB | 31.1 KiB | 43.2 KiB |
| 18 | 9.6 KiB | 2.0 KiB | 60.1 KiB | 75.0 KiB |
| 20 | 12.3 KiB | 2.0 KiB | 80.5 KiB | 98.1 KiB |
| 22 | 16.2 KiB | 2.0 KiB | 98.7 KiB | 119.8 KiB |
| 24 | 22.2 KiB | 2.0 KiB | 125.4 KiB | 152.9 KiB |
| 26 | 32.4 KiB | 2.0 KiB | 156.8 KiB | 194.8 KiB |
| 28 | 50.7 KiB | 2.0 KiB | 181.5 KiB | 237.8 KiB |
| 30† | 85.1 KiB | 2.0 KiB | 374.9 KiB | 466.3 KiB |
| 31† | 86.8 KiB | 2.0 KiB | 397.1 KiB | 490.5 KiB |
| 32† | 151.8 KiB | 2.0 KiB | 415.4 KiB | 573.7 KiB |

Reading notes: **time is forest-dominated** (81 % at n=16 rising to ~92–96 %
at n ≥ 26) while **bytes are Ligerito-dominated** (72–82 % of the proof is
the recursive opening; the forest side is 8–51 KiB through n=28 and the
ring-switch a constant 2 KiB) — the inversion to keep in mind when
optimizing either axis. The n ≤ 28 rows are the (rate 1/2, k=4) fast
generation — proofs 41–57 % smaller than the † (k=6) generation at
n=22–28 for +2–10 % prove at n ≥ 24 (the fixed 16-bit-per-level grinding
shows mainly at n=22: +6 ms on a 25 ms prove); prove times additionally
reflect the pass-fusion work, ~10–15 % over the
previous defaults in same-session A/B at n ≥ 26 — differences vs the
previous table beyond that reflect measurement-session conditions.
Verify stays ms-class and proofs sub-MB throughout — prover RAM is the
only wall. The n=20→22 step in proof size (98 → 120 KiB) is the
`sha_lig_configs` boundary: the embedded FAST profile takes over at
`m ≥ 22` (hardcoding the tiny ad-hoc config at big shapes instead is
catastrophic — n=28 commit measured 292 s ad-hoc vs tens of ms embedded).

Measurement protocol (inherited from the zinc-plus lore): idle the box first;
for quotable *time* numbers at big shapes run one shape per process (the peak
numbers reset per shape and are fine in one sweep); quote medians.

**Single-threaded** (n ≤ 28; `--no-default-features`, so no rayon — the
sequential build; schedule L/4 throughout). Same k=4 default and `t ≈ 0.6n`
shapes as above, one sweep. Proofs are **byte-identical** to the
multi-threaded table (deterministic prover; threading changes only
wall-clock), so only time and peak differ:

| n | shape (t, s) | commit | forest+presum | ligerito open | prove | verify | prove peak | config |
|---|---|---|---|---|---|---|---|---|
| 16 | 10, 6 | 0.47 ms | 1.64 ms | 0.64 ms | 2.64 ms | 1.53 ms | 0.69 MB | adhoc r1/4 |
| 18 | 12, 6 | 0.45 ms | 3.16 ms | 0.77 ms | 4.58 ms | 1.02 ms | 2.44 MB | adhoc r1/4 |
| 20 | 13, 7 | 0.67 ms | 7.90 ms | 1.48 ms | 9.87 ms | 1.34 ms | 7.83 MB | adhoc r1/4 |
| 22 | 14, 8 | 0.77 ms | 31.0 ms | 9.15 ms | 39.7 ms | 1.59 ms | 23.9 MB | k=4 r1/2 |
| 24 | 15, 9 | 2.92 ms | 118 ms | 26.3 ms | 146 ms | 2.16 ms | 86.2 MB | k=4 r1/2 |
| 26 | 16, 10 | 9.38 ms | 461 ms | 53.6 ms | 517 ms | 3.17 ms | 328 MB | k=4 r1/2 |
| 28 | 17, 11 | 39.7 ms | 1.90 s | 180 ms | 2.09 s | 5.98 ms | 1.28 GB | k=4 r1/2 |

The MT→ST prove ratio grows with n (~1.6× at n=22 to ~3.6× at n=28) as the
forest fold parallelizes better at scale; verify and proof size are
unchanged. The n=20→22 `config` step (adhoc r1/4 → k=4 r1/2) is the
`sha_lig_configs` `m ≥ 22` boundary. Reproduce: `F2Z_BENCH_SHAPES="10:6:1
12:6:1 13:7:1 14:8:1 15:9:1 16:10:1 17:11:1" F2Z_BENCH_REPS=3
F2Z_LIG_PROFILE=custom:1:4 OBLONG_PROFILE=1 RUSTFLAGS="-C target-cpu=native"
cargo bench --bench pcs --no-default-features --features unchecked`.

**Packed-rows commit.** The harnesses generate the instance straight into
per-column bit rows and commit via `commit_rs_ligerito_rows` — the `u128`
cell tensor (16 B per cell) never exists, so peak memory sits at the
packed/forest scale. (The pre-restructure dense path held ~16 B/bit —
4.28 GB and a 7.9 s commit at n=28 — and could not reach n ≥ 30 on 16 GB
at all; the pre-LTO build was a further ~1.7–3× slower at the big shapes.)

**Factored LUT tables + one-multiply materialising folds (2026-07-25).**
The 16-case LUT rounds of the two bottom forest layers (`Pair3Bits` round
1, `Leaf3Bits` round 2) used four precombined tables (`64·2^k` entries —
16 MiB at n = 28) whose gather streams fall out of L2 and dominated the
rounds (53 + 46 ms of a 626 ms profiled prove, line-level flamegraph
attribution). The default now keeps only the suffix-weighted `w·te` array
and recombines against the set's raw `to` per slot — three wide multiplies
instead of one, 4× less table footprint (`F2Z_PAIR2_FACTORED=0` opts out)
— and the three LUT→Dense materialising folds use the canonical
one-multiply fold `v0 + ρ(v0 + v1)` in place of `(1+ρ)v0 + ρv1` (exact
distributivity, same canonical bits). Byte-identical proofs (in-process
pins vs the eager forest; cross-process `fuse_check` vs the pre-change
build). Measured (alternated in-window A/B, medians of 5, both changes):
n=28 prove 616 → 558 ms on a cool box (**−9.4 %**) and 658 → 559 ms on a
churned box (−15 % — the factored form is markedly less sensitive to
cache/thermal pressure), n=26 ≈ −4 %, n=22 a wash, peaks unchanged. The
reference table above predates this change (its n ≥ 26 prove rows are now
~5–10 % pessimistic, pending a fresh-box re-measure). The profile scopes
`eqf:msg:*` / `eqf:fold:*` now split the aggregate round buckets by shape
(`OBLONG_PROFILE=1`).

**Flock-derived open-path kernels + fused Ligerito round 0 (2026-07-26).**
The ring-switch and basis passes replaced their data-dependent bit scans
with flock's kernel shapes, operating on the packed message in place (a
`PackedBits` word view — no conversion pass): the `s_v` in-pack marginals
run the method-of-four-Russians fold (per 8 elements: two 16-entry
subset-sum tables, one 8×8 bit transpose per byte position, then two
lookups + one RMW per output bit — flock's
`fold_1b_rows_1way_mfr_8wide_k4` shape), and the `Φ_{r″}` basis maps run
16 η-premultiplied byte-table subset-sum gathers per element (flock's
`fold_b128_elems` shape; premultiplying the batching `η` into the tables
also deletes the per-element `η·Φ` field multiply). The mod-q prover
additionally fuses the Ligerito **round-0** message `(u_0, u_2)` into the
basis-fill pass (deferred-reduction accumulators) and enters flock through
`recursive_prover_with_basis_precomputed_round0`, skipping flock's own
full `(f, b)` read pass; and the pre-sumcheck's `R·m` rounds got a
wide-accumulating `RoundPolyEvaluator` (the crate's first use of that
hook). `F2Z_RS_FAST=0` opts out of all of it. Byte-identical proofs
(kernel unit tests vs the scalar scans; cross-process `fuse_check`
old-vs-new). Measured (alternated in-window A/B pairs, `prof_probe`,
n=28 t=17 s=11): `mq:bcomb` 17.3 → 4.5 ms (**−74 %**), `mq:rings`
13.6 → 8.0 ms (−41 %), `mq:lig` ≈ −2 ms (the skipped round-0 pass),
`mc:presum_run` 2.07 → 0.87 ms (−58 %); prove total ≈ 566/556 →
552/550 ms (≈ **−2 %** end-to-end — the open-side machinery is a small
slice of a forest-dominated prove). The batched and virtual-XOR paths
inherit the kernels through the shared helpers.

**Forest bucket ranking + the leaf ΔΔ-table experiment (2026-07-26).**
A full n=28 phase tree (`prof_probe`, post-`F2Z_RS_FAST`, 558 ms prove)
ranks the 466 ms forest: fused dense rounds `eqf:fmsg` 118.6 ms (at the
measured kernel floor), `mf:bitgen` 53.5 ms and `mf:build_levels` 49.0 ms
(streaming transposes/gathers over the bit store), `eqf:msg:leaf_r1`
37.0 ms, materialising folds 49.3 ms, 16-case rounds 41.5 ms,
`leaf3_r2/r3` 35.4 ms, dense round-1s 30.3 ms. Against the ledger's
"leaf_r1 pair-slot 4-Russians would cut ~⅓" estimate: pair-slot merging
GROWS table bytes (4+4-case → 16-case per pair is 2× the `t_a0`/`t_a1`
bytes), i.e. the wrong direction under the bytes-rule lesson — so the
byte-SHRINKING variant was built instead (`LeafA2::Factored`,
`F2Z_LEAF_A2_FACTORED=1`): store the four raw ΔΔ cross products per slot
(halves total leaf-table bytes `24·2^k → 12·2^k`, one sequential 64 B
line per slot) and select via four branchless masked adds in a two-temp
tree. **Measured SLOWER at L2-resident shapes** — n=26, 3 alternated
in-window pairs: `leaf_r1` 6.2 → 7.4 ms (+18 %, net ≈ +0.5 ms after the
4× cheaper table build) — an L2-hit gather beats four masked selects, so
the default stays precombined and the factored form ships **opt-in** as
the A/B lever for the n ≥ 30 regime (precombined leaf tables are 25 MB
at n=30, past the P-cluster L2; the 12.6 MB factored form fits).
Byte-identical all ways (arm-equivalence unit test + cross-process
`fuse_check` over default/opt-in/pre-change). n ≥ 28 A/B was NOT
adjudicable that session (churned box: 180 MB free, 3.2 GB swap —
the recorded measurement pitfall now bites n=28 too).
**RESOLVED same day on a memory-fresh box (6.9 GB free), 3 alternated
in-window pairs per shape**: n=30 — `leaf_r1` 307/312/317 →
148/147/146 ms (**2.1×**), prove 2871 → 2672 ms median (**−6.5 %**
end-to-end), and the factored arm is far less volatile (±3 ms vs
±40 ms); n=28 — `leaf_r1` 34.6/36.3/38.5 → 29.5/32.6/31.5 ms
(−13–18 %), prove ≈ −0.5–1 %. The default is now **size-gated**:
factored iff `half ≥ 2^15` (precombined tables ≥ ~12.6 MB — the
P-cluster-L2 co-residency edge; n=26 and below stay precombined, where
factored loses ~1.2 ms). `F2Z_LEAF_A2_FACTORED=0/1` forces either arm.

**Software prefetch on the stash-gather rounds (2026-07-26).** The
`pair3_r2`/`leaf3_r3` message bodies and the two materialising folds
read the ρ-dependent fold-table stashes at data-dependent lines inside
sequentially-advancing windows — a pattern the hardware prefetcher
cannot follow, but whose indices are cheaply recomputable ahead from the
sequential bit words. The four loops now issue `prfm pldl1keep` for slot
`b+16`'s lines while slot `b` computes (`lut_prfm`, `prefetch_l1` —
inline asm, semantically inert ⇒ byte-identical by construction; pinned
via `fuse_check` with the knob forced). Measured (fresh box, 3 alternated
in-window pairs): n=30 — `fold:leaf3mat` 197 → 147 ms, `fold:pair3mat`
199 → 155 ms, `leaf3_r3` 132 → 106 ms (all **−20–25 %**), `pair3_r2`
−5 %; **prove 2518 → 2394 ms median (−4.8 %)**. At n=28/n=26 it LOSES
1–8 ms per scope (stashes 8.4 MB and below are L2-shallow; the index
recompute + LSU pressure beat the latency hidden). Default is therefore
**size-gated**: on iff the round's `half ≥ 2^14` (the n=30-class
boundary, stashes ≥ 16.8 MB); `F2Z_LUT_PRFM=0/1` forces either arm.
Combined with the factored leaf tables, the n=30 prove on this box went
2871 → 2394 ms (**−17 %**) this session; the README reference table's
n=30 row († k=6 generation) predates both changes AND the k=4 configs —
pending its fresh re-measure.

**T4-gather prefetch in the forest build/JIT (2026-07-26).** The n=30
phase tree ranked `mf:build_levels` (380 ms) and `mf:bitgen` (320 ms —
it contains the JIT level regeneration, not just the bit transposes) as
the top attackable buckets; both are dominated by `T4At` gathers into
the shared 16-case table (`t4[(j≪4)|(cE≪2)|cO]` — sequential 256 B
blocks, data-dependent line pick, 16.8 MB at n=30, past the P-cluster
L2, ~536 M gathers per prove). All eight sites (`gen_top` + fused-JIT ×
single/multi × L4/L8) now issue the same ahead-of-use `prfm` via
`T4At::prefetch_at` (a `look` hook on `dense_jit_fused_round1`).
Measured (3 alternated in-window pairs, n=30, stash-PRFM on in both
arms): `build_levels` 310 → 261 ms median (−16 %), `bitgen` 325 → 252 ms
(−22 %), **prove 2471 → 2341 ms median (−5.3 %)**. Default is
size-gated: on iff the `t4` table is ≥ 16 MiB (n ≥ 30; at n=28 the
8.4 MB table is L2-resident and forcing it on measures slightly worse);
`F2Z_T4_PRFM=0/1` forces either arm. Byte-identical (hint-only; pinned
via `fuse_check` with all three prefetch/table knobs forced). Session
cumulative at n=30: **2871 → ~2340 ms (−18.5 %)**.

**Mod-q RLC claim families — the XOR-triple prototype (2026-07-26).**
EXPERIMENTAL API (`prove/verify_mle_eval_mod_q_ligerito_rlc_family`, no
`proof_codec` wiring; construction and soundness obligations in
`docs/rlc-family-note-prompt.md`, prototyping plan in
`docs/rlc-family-proto-prompt.md`): k claims on F₂-linear forms of j
committed UAIR columns (shared column point) collapse into ONE forest per
weight chunk via the γ-RLC case weight `W_b(m) = Σ_i γ_i·w_{i,b}·L_i(m)
mod q` — 2^j-case leaves `α^{W_b^{(l)}(m(pos))}`, a (2^j−1)-channel
presum against the bit monomials (τ_S = char-2 subset zeta-transform of
the case α-powers), and one η-batched degree-(j+1) eq-sumcheck
discharging the |S|≥2 monomial residuals into j committed openings. The
j=2 lazy forest is a pure REWIRING of the existing driver kernels
(4-case leaf = `Pair2TauSet` keyed by the two columns' bit streams →
`Pair3Bits` leaf round, `T4Bits` product layer, L/4 build; byte-identical
to the eager reference, pinned) — no kernel changes. Measured (this box,
16 GB M-series, alternated in-window, medians of 5; the XOR triple k=3
j=2 with per-claim row points vs (a) `ind3` = three independent
claims-only vx proofs and (b) `vx3` = the batched claims-only vx path,
`single` = one claim as the unit; `examples/rlc_ab.rs`):

| n | single | rlc3 | vx3 | ind3 | rlc3 proof | vx3 | ind3 |
|----|--------|------|-----|------|-----------|-----|------|
| 22 | 27.2 ms | **29.5 (1.08×)** | 36.6 (1.35×) | 71.3 (2.62×) | **135 KB** | 169 KB | 384 KB |
| 24 | 33.5 ms | 82.5 (2.47×) | 84.0 (2.51×) | 111.5 (3.33×) | **183 KB** | 248 KB | 524 KB |
| 26 | 77.4 ms | 241.3 (3.12×) | 235.2 (3.04×) | 242.1 (3.13×) | **249 KB** | 378 KB | 719 KB |
| 28† | 234.5 ms | 1073 (4.58×) | 948 (4.04×) | 710 (3.03×) | **343 KB** | 602 KB | 1000 KB |

† churned-box caveat: the n=28 discharge tables (~3.2 GB of the ~5.5 GB
peak) ran against ~4 GB free on the 16 GB box; the rlc3 scaling step
n=26→28 (4.45× for 4× data vs 4.03× for vx3) shows the pressure.

**The forest collapse works exactly as designed** — phase trees
(`OBLONG_PROFILE=1`, per-prove): at n=26 the ONE 4-case forest costs
59.5 ms vs the vx batch's 182.7 ms (3.1×; k=3 pads to 4 tree-sets) and
~138 ms for three independent forests; the 4-case leaf overhead lands at
~1.3× a single-claim forest, inside the predicted 1.1–1.3 band. Proof
size wins unconditionally (ONE forest transcript + ONE u′ vector):
−26…−43 % vs vx3, −65 % vs ind3. **The headline prediction (triple ≈
1.6–1.8× single vs 3×) FAILS at n ≥ 24**: the Phase-1/2 discharge
realization costs 40.3 ms at n=24 and 162.8 ms at n=26 (tables 62.6 +
sumcheck 86.6) — ~2.1 single-claim-equivalents, 4–5× the modeled
0.4–0.6 forest-equivalents — because it streams THREE dense 16 B/position
K-element tables (`A`, `M₁`, `M₂`) through a generic multi-degree
sumcheck while the forest streams ~2 bits/position through case-LUT
kernels. Net: **n=22 a clear win (1.08× vs 1.35×), n=24 a tie, n≥26 a
loss on time** (and vx3 itself is ≈/worse than ind3 at n≥26 — the
pad-to-power-of-two forest costs the 1.33× that the shared tail saves).
Closed sub-experiment: re-realizing the j=2 discharge on the eq-factored
driver (no materialized eq table, NEON kernels) measured SLOWER
(dis_run 20.9 → 37.9 ms at n=24) — that driver parallelizes across
GROUPS and the discharge is ONE group; reverted. What stands between the
prototype and the prediction, in ranked order: (1) bit-LUT discharge
rounds — the M_i are 0/1, so nodes 0/1 of the round polynomial need no
multiplies and the post-fold values are 4-case (`{0,1,ρ,1+ρ}`-structured
— exactly the forest's `LeafFoldTables` cascade), projected to cut the
discharge to ~0.3–0.5 forest-equivalents and land the triple at
~1.5–1.9× single; (2) chunk-internal parallelism for one-group dense
eq-factored instances; (3) the generic prover's fold passes are
3-way-parallel only. Levers: `F2Z_RLC_EAGER=1` (materialized-leaf
forest: +29 % forest phase at n=24), `F2Z_RS_FAST=0` (generic discharge
evaluator), `F2Z_LUT3=0` (Pair2 leaf round). The j=1 corollary (k
same-column claims at different row points, 2-case forest, NO discharge)
and the j=3/k=4 family are implemented and tested; j ≥ 3 falls back to
the eager forest (the 8/16-case leaf-round kernels are the open lever).

**RLC families: the leaf-bit discharge lands the prediction
(2026-07-26, follow-up).** The j=2 discharge was re-realized on the
observation that `M_i = 1 + ¬m_i·1` — the complement-bit streams with
all-ones τ put `Σ_x eq(pt,x)·M₁(x)·M₂(x)` in EXACTLY the driver's
leaf-bit-affine shape, so the discharge now runs as a forest leaf layer:
**phase A** over the t' row variables with per-column
`Leaf3Bits` groups (scale `η_l·eq(pt_l⁺, c)`; the M tables are NEVER
materialised — the driver reads the packed bits, the eq side is
suffix-factored, parallelism is across the 2^s groups — the proven
forest-leaf configuration verbatim); **phase B** over the s column
variables as one tiny Dense pair group per chunk whose finals ARE the ω
openings; per-chunk entry sums β_l ride between the phases (absorbed,
tag 0x39 — phase A closes against `Σ_l eq(r_A, pt_l⁻)·β_l`, phase B
opens at `Σ β_l` and closes at `Σ_l η_l·eq(r_B, pt_l⁺)·ω₁·ω₂`). This
deletes all three dense tables (the ~3 GB n=28 pressure) and both
measured bottlenecks: at n=26 the discharge fell 162.8 → **17.1 ms**
per prove (tables 62.6 → 0.8, sumcheck 86.6 → 16.1) ≈ 0.29
forest-equivalents — inside the note's modeled 0.4–0.6. Re-measured
(same protocol, medians of 5; n=28 of 3, now clean — no dense tables):

| n | single | rlc3 | vx3 | ind3 |
|----|--------|------|-----|------|
| 22 | 26.8 ms | **21.5 (0.80×)** | 37.4 (1.39×) | 69.8 (2.60×) |
| 24 | 33.3 ms | **58.7 (1.76×)** | 84.5 (2.54×) | 110.8 (3.33×) |
| 26 | 78.3 ms | **146.7 (1.87×)** | 234.6 (3.00×) | 239.6 (3.06×) |
| 28 | 232.5 ms | **405.2 (1.74×)** | 851.5 (3.66×) | 707.9 (3.04×) |

**The construction's prediction (~1.6–1.8× single vs ~3×) is CONFIRMED
at every forest-dominated shape** — 1.74–1.87× at n=24–28 against
2.54–3.66× for the batched-vx baseline and 3.03–3.33× for three
independent proofs: the triple proves **21–52 % faster than the best
existing path** at every measured n, and at the tail-dominated n=22 the
marginal claims are nearly FREE: the family's own single (`rlc1`,
19.9 ms) vs its triple (20.9 ms) — two extra claims for ~1 ms, because
the per-proof tail (Ligerito recursion + ring-switch + basis, ~13–15 ms
here) is paid once per PROOF. (The headline 0.80×-of-single overstates
that by a few ms of measurement subtlety: FS **grinding luck**. The
m ≥ 22 configs grind 16-bit query + per-level fold PoW whose nonce
search length is a DETERMINISTIC function of the transcript — measured
296,927 blake3 tries for the vx-single statement vs 165,526 for `rlc1`,
identical on every rep, so alternated-median protocol cannot average it;
switching the single to a different claim moved it 26.3 → 21.6 ms.
Statement luck is worth ±3–5 ms — material at n=22's ~20 ms totals,
≤ ±2 % at n ≥ 26. Small-n comparisons should average over multiple
STATEMENTS, not just reps.) Proof sizes unchanged (the two-phase
discharge is slightly smaller: 135/182/249/341 KB). The j ≥ 3 discharge keeps the
generic multi-degree form (its |S| ≥ 3 channels are triple-plus
products — the pair-driver trick needs the AND rows as committed-side
factors, which would demand a second discharge level; open). The
supersession question (this API vs the batched-vx path) now has a
measured answer for shared-column-point XOR families: the RLC family
dominates on both axes.

**RLC families: zero-channel elision (2026-07-26, completeness fix).**
Legitimate degenerate families make presum channels vanish
IDENTICALLY: a pure-XOR family — every claim on the same ⊕-combination,
including a lone k=1 XOR claim — has `α^{W(m)}` factoring through the
XOR, so the AND channel's τ is the zero table and the verifier's
`μ_S = expected/R̂_S` would divide by zero (`RHatZero` on an HONEST
proof; caught by a new test). This is a distinct vanishing mode from
the affine/inert-variable case (there `W` doesn't depend on a variable;
here it does, but `α^W` factors through a linear quotient of the case
space). Fix: both sides derive the ACTIVE channel set per chunk from
the public case-weight τ tables and elide zero channels everywhere —
presum groups, discharge participation (per-chunk for j=2's two-phase
form, per-(chunk, S) pairs for j ≥ 3), the ω set and the rings all
follow the active sets; every family column must appear in ≥ 1 form.
Pure-XOR families (k=1 and multi-point) now roundtrip. Also measured
(`F2Z_AB_SINGLES=1`): a LONE claim gains nothing from the family API —
`rlc1` ≈ `vx-single` (33.8 vs 33.3 ms at n=24; 72.7 vs 76.3 at n=26)
and a lone XOR claim is CHEAPER via the vx extraction path (74.5 vs
97.5 ms at n=26 — extract-once + 1-bit-affine forest beats the 4-case
forest). Guidance: k = 1 → the vx path; k ≥ 2 with a shared column
point → the family.

**RLC families: the j ≥ 3 cascade discharge + general forests
(2026-07-27).** The j ≥ 3 path (eager forest + a dense-table multi-degree
discharge) was replaced end to end. (1) **The discharge is now a 2-level
CASCADE of the leaf-bit form for every j ≤ 4**: each active |S| ≥ 2
channel factors into a PAIR of sides — committed columns and, for
|S| ≥ 3, 2-bit AND intermediates taken from the already-extracted AND
rows (`M₁M₂M₃ = (M₁∧M₂)·M₃`; `rlc_channel_sides`) — and the whole batch
runs the same two-phase complement-bit sumcheck as j = 2 (level 1); the
AND openings at ρ are then η'-batched and discharged the same way at a
second level, exiting at committed openings at ρ' (level 2; j = 2 has no
AND sides and keeps its exact byte shape — `discharge_eqf/omegas` +
`discharge_eqf2/omegas2`, sides in canonical order, elision-aware
throughout). The dense A_S/M_i tables and the generic multi-degree
discharge are gone entirely. (2) **General forests**: j = 3, 4 default
to the EAGER forest — a Dense-JIT lazy form over shared case/product
tables (`prove_merged_forest_lazy_rlc_general`, byte-identical, pinned)
was built and measured SLOWER at n = 24–28 (−6 % at n=26, −9 % at a
churned n=28: the per-value regeneration closures cost more than the
saved materialisation), so it is the `F2Z_RLC_J34_LAZY=1` low-peak-memory
arm (~⅓ the peak) and the 8/16-case leaf-ROUND kernels remain the open
lever. Measured (same protocol/box; j=3 k=4 family — claims on
{0},{1},{2},{0,1,2} — vs the batched-vx and independent baselines,
`F2Z_AB_J3=1`; single = one vx claim at the same shape):

| n | single | rlc4 | vx4 | ind4 | proofs (rlc4/vx4/ind4) |
|----|--------|------|-----|------|------------------------|
| 24 | 33.3 ms | **81.9 (2.46×)** | 99.1 (2.98×) | 146.2 (4.39×) | **193**/284/698 KB |
| 26 | 78.3 ms | **224.6 (2.87×)** | 245.1 (3.13×) | 329.7 (4.21×) | **259**/445/959 KB |
| 28† | 232.5 ms | **893.6 (3.84×)** | 918.0 (3.95×) | 921.8 (3.96×) | **353**/736/1334 KB |

† churned box (~2 GB free). The j = 3 family now beats BOTH baselines at
every measured shape — −17/−8/−3 % vs the batched path and −44/−32/−3 %
vs independent proofs — with 32–52 % (vs vx4) and 72–74 % (vs ind4)
smaller proofs; per claim it runs at 0.61–0.96× a single proof. The
margin is thinner than j = 2's (1.74–1.87× for k = 3) for three
structural reasons: the 8-case leaves run Dense rounds (no LUT kernels),
the presum carries 7 channels, and k = 4 gives the batched baseline a
pad-free forest. Tests: j = 4 full-depth cascade (two AND intermediates)
+ level-2 tamper coverage; 83/83 green in both forest arms; clippy at
parity.

**RLC families: SHARED-POINT maximal families — one point, the full
XOR-closure (2026-07-27).** The deployed family API requires only a
shared COLUMN point; the motivating deployment has ALL points equal — k
claims `MLE[INT(a_i)](r) = c_i` at ONE `r`. The structural collapse:
with `w_{i,b} = w_b` the `[2^{t'}][2^j]` case-weight table is **rank-1**
— `W_b(m) = (w_b·Γ(m)) mod q`, `Γ(m) = Σ_i γ_i·L_i(m)`, 2^j values
total — the family is **capped and canonical** (two claims with the same
form at one point are the SAME claim: dedupe, `k ≤ 2^j − 1`; the maximal
family is the whole XOR-closure — j=2: k=3, j=3: 7, j=4: 15), and the
FS statement **shrinks** (ONE `2^{t'}` weight vector absorbed instead of
k). New API, nothing existing changes bytes:
`prove/verify_mle_eval_mod_q_ligerito_rlc_family_shared_point` — one
`row_weights_q` + `(form, claimed)` pairs; duplicates deduped (first
occurrence wins, equal-c enforced; a conflicting duplicate rejects as
Shape); collapsed absorb tag 0x41, deliberately a DIFFERENT transcript
from the general 0x40 path (cross-verification rejects both ways —
pinned); rank-1 case build `rlc_gamma_cases` +
`rlc_case_weights_shared_point` (pinned equal to the general build);
downstream of the γ draw both entries share one core (front/core split —
the general path is byte-stable, 86/86 green both guard modes, clippy at
parity). CLI presets `--family j2s|j3s|j4s` (k = 3/7/15); harness
`F2Z_AB_SHARED=1` (+ `F2Z_AB_STMTS=S` statement averaging). Measured
(same box/layout/protocol as the tables above; n = 22/24 = MEAN over 3
statements of medians-of-5 — FS grinding luck is deterministic PER
STATEMENT and the n=22 j2 row swung 21–52 ms across statements; n = 26
medians of 5, n = 28 of 3; `single` = one vx claim in-window; vx pads k
to 2^⌈log₂k⌉ tree-sets and is SKIPPED when its padded eager forest
estimate exceeds 8 GB):

| n | single | j2 k=3: rlcS / vx / ind | j3 k=7: rlcS / vx / ind | j4 k=15: rlcS / vx / ind |
|----|--------|------------------------|-------------------------|--------------------------|
| 22 | 27.9 ms | 34.2† / 38.3 / 60.4 | **37.7** / 69.7 / 153.3 | **60.1** / 131.7 / 339.8 |
| 24 | 34.3 ms | **54.3** / 87.6 / 113.2 | **88.0** / 177.2 / 280.7 | **141.8** / 346.2 / 606.8 |
| 26 | 80.0 ms | **126.5** / 257.3 / 250.7 | **227.4** / 484.3 / 598.4 | **431.1** / 967.2 / 1340.9 |
| 28‡ | 239.1 ms | **503.0** / 992.0 / 771.1 | **1118** / — / 1908 | **2638** / — / 4195 |

† grinding-luck-dominated (single-claim-scale totals). ‡ churned box
(vx3's 4.3 GB padded forest in-window; fresh-process CLI j4s at n=28:
2051 ms eager / **1810 ms lazy**). Proof sizes (rlcS/vx/ind, KB):
k=7: 258/650/1676 at n=26; k=15: **272/1193/3595** at n=26, 367/—/5010
at n=28 — the maximal family's proof is −77 % vs vx and −92 % vs ind at
n=26, 18–24 KB/claim. Verify (same order, ms): k=15: **11.0**/33.1/56.3
at n=26, 17.8/—/98.5 at n=28. Findings:

1. **Maximal families amortize hard.** Per claim, k=15 proves at
   4.0/9.4/28.7 ms at n=22/24/26 = **0.14×/0.28×/0.36× a single claim**
   (k=7: 0.19×/0.37×/0.41×) — well below the measured k=4/k=5 points
   (0.61–0.96×), as predicted: more claims over the same forest +
   discharge. rlcS beats vx by 1.9–2.4× and ind by 2.6–5.7× at n≤26.
   The batched-vx baseline also hits a **pad wall**: k=15 pads to 16
   eager tree-sets = 17.2 GB at n=28 — unrunnable on this box (k=7 pads
   to 8 = 8.6 GB, also skipped), while rlcS runs one 2^j-case forest.
2. **Rank-1 buys API/statement, not runtime** (predicted, confirmed):
   `rlc:casew` + `rlc:pows` are ≤ 0.6 % of prove at every j (0.6–1.7 ms
   + 0.2–0.6 ms at n=26), so the collapsed build's saving is invisible —
   rlcS ≈ rlcG within ±6 % both directions across shapes (statement
   luck). Verifier-side the O(2^j·2^{t'}) case-pow step (`rlcv:pows` +
   `rlcv:roots`) is only ~0.35–0.58 ms ≈ 5 % of the 5.7–11 ms verify at
   n=26 — NOT the dominant cost at these shapes; the verify win vs
   baselines (3.0×/5.1× cheaper at k=15 n=26) is structural (one forest
   transcript, one u vector). The exponent wall stands:
   `α^{(w_b·Γ_m) mod q} ≠ (α^{Γ_m})^{w_b}` — per-(row, case) pows
   remain; they are just already cheap.
3. **The discharge, not the forest, dominates the maximal-family
   prove.** Phase trees at n=26 (per prove): j=2 discharge 18.6 ms vs
   forest 64; j=3 90 vs 67; j=4 **257 vs 63** — ~20 ms per active
   |S| ≥ 2 channel (1/4/11 channels), linear in channel count, while
   the forest stays flat in k. At the maximal j=4 family the discharge
   is ~60 % of prove. Levers, in order: the 8/16-case leaf-ROUND
   kernels (the forest's open lever does NOT help the discharge), and
   the note's "riding the forest" fusion (one pass, two accumulators)
   which targets exactly these duplicated bit-streaming passes.
4. **The j ≥ 3 lazy forest arm flips at scale**: j4s eager/lazy = 154/158
   ms at n=24 (eager −2 %), 471/475 at n=26 (tie), 2051/**1810** at n=28
   (lazy −12 %) — the familiar size-gating; peak is tail-dominated
   either way (3.4 GB at n=28: `b_comb` + `p_msg` clone ≈ 2.1 GB of it),
   so the lazy arm's leaf saving shows up as time, not peak.
5. **Cluster-planner inputs** (the note's open problem, one-global-point
   case): within a ≤4-column group at one point, open the FULL closure —
   marginal claims are nearly free (k=15 adds 47 % over k=7 at n=26 for
   2.1× the claims) and every channel is exercised; prefer wider j over
   more groups until the discharge's ~20 ms/channel × (2^j−1−j) exceeds
   a fresh group's flat forest+tail (~150 ms at n=26) — i.e. j=4 groups
   are right at n≤26 today, and the discharge kernels/fusion move the
   crossover further toward wide j. Never route a shared-point family
   through the padding batched-vx path.

**Structured taps: ROT/SHIFT/word-offset claims — translated-eq
openings land; the stream family loses to extraction (2026-07-27;
re-measured same day under the corrected semantics).** Claims on
tapped convolutions `b_i[k] = ⊕ ROT^r/SHIFT^r(a_col[k−o])` — corrected
setting: W = 1 bit-vectors whose ENTRY axis is grouped into 32-bit
words (`p = 32k + j`, `grp_log2 = 5`); ROT/SHIFT translate the low-5
index field within each group, offsets translate the word field
(equivalently `b = M·a` for a banded block-rotation `F₂` matrix `M` —
the tap descriptor is the succinctness-preserving normal form of that
`M`; an arbitrary `M` costs the verifier O(N) at the closure) — are
provable two ways, both EXPERIMENTAL (the j=2 k=6 instance of
`docs/rlc-structured-taps-prompt.md`; Phase-0 analysis + corrected
conventions in `docs/rlc-structured-taps-phase0.md`).
(1) **The tap-claims path**
(`prove/verify_mle_eval_mod_q_ligerito_tap_claims`, tag 0x42):
`extract_virtual_tap_rows` derives the tapped rows (whole-run gathers:
the group field lives in the clear axis, so ROT/SHIFT permute clear
rows and word offsets shift the row_hi runs by the borrow) and the
claims run the batched x-forest; each residual expands into
**translated-eq committed openings** — the Phase-0 result that index
translation is NOT a coordinate permutation (counterexample recorded)
but IS a carry matrix product of bond dimension 2, so the opening
weight splits `K̃(v,y) = Σ_β A_β(v)·B_β(y)` over ≤ 3 public carry
classes, each class one ring whose tables are translated *slices* of
plain eq tables, closed succinctly by an MPS generalization of
`residual_b_evals` at ≤ 2× the plain cost. (2) **The stream family**
(`..._tap_family`, tag 0x43): the claims are per-position XORs of their
13 deduped tap streams, so the RLC family construction applies verbatim
over the streams — clustered `{b1,b3,b5}`/`{b2,b4,b6}` (j_eff = 6/7;
the monolithic union has ~900 active channels) with fused AND-scan
presums (27 + 31 channels), per-cluster 2-level cascades (45 monomial
channels), and all stream openings through the translated-eq rings.
Measured (`examples/taps_ab.rs`, corrected semantics, bv = 0, g = 5;
alternated in-window medians of 5, this 16 GB box; `single` = one
identity tap claim; caveats: one statement per n — FS-grinding luck
±3–5 ms — and the box was churned at n=26, ~2 GB free):

| n | single | tapf (family) | vx6 (batched taps) | ind6 | proofs (tapf/vx6/ind6) |
|----|--------|---------------|--------------------|------|------------------------|
| 22 | 25.5 ms | 257.9 (10.1×) | **118.7 (4.7×)** | 151.9 (6.0×) | 353/**338**/883 KB |
| 24 | 94.4 ms | 743.4 (7.9×) | **319.3 (3.4×)** | 401.2 (4.3×) | **452**/566/1262 KB |
| 26† | 146.0 ms | 3823 (26.2×) | 2006 (13.7×) | **940.1 (6.4×)** | **622**/994/1848 KB |

† churned box; n=28 skipped (the eager leaves + the 8-set batched
forest need multi-GB working sets against ~2 GB free — the numbers
would be swap noise; the 22–26 trend is monotone and unambiguous; the
pre-correction run measured the same verdict at 233/791/4297 vs
113/350/1885). **The family loses on prover time at every measured
shape — the channel count eats it**, exactly the risk the session
prompt flagged: the n=24 phase tree attributes ~600 of 743 ms to the
cascade (45 monomial channels × ~13 ms — the same ~per-channel
discharge constant the shared-point session measured independently),
160 ms to the two eager 64/128-case forests (the wide-leaf lazy
kernels are unbuilt), and ~50 ms to the 70 twisted rings + basis
fills, against the batched baseline's ~280 ms padded 8-set forest +
tail. The instance's sharing is thin — 13 streams for 6 claims, one
shared tap — so the family trades ~5 saved forest bodies for ~45
carry channels at a third of a body each: a structural loss whenever
claims bring mostly-fresh streams. What the family DOES win: proof
size at n ≥ 24 (−20 % vs vx6 at n=24, −37 % at n=26 — one forest
transcript + one fold vector per cluster) with verify within 1.6–2.1×
(the 70 MPS ring closures). Also measured: the batched path's
pad-to-8 forest beats 6 independent proofs at n ≤ 24 but INVERTS at
n=26 (2.1× worse — the 8-claim working set against a churned box; the
shared-point session's "vx pad wall" in miniature). **Guidance**:
tapped-convolution claims route through the tap-claims (extraction)
path — the family construction's amortization needs dense stream
reuse (many claims over few streams), which ROT/SHIFT tap schedules
of this shape do not have; revisit only if the per-channel discharge
cost falls an order of magnitude (kernels/fusion — the same top lever
the shared-point line identified) or for byte-bound deployments.
Conventions pinned by the harness and TO CONFIRM against the source
spec: `a_{3,k−2}` read as `a_1[k−2]`, the bare `ROT` in b₃ as
`ROT^2`, and ROT/SHIFT gather toward higher within-word positions
(`ROTR`/`SHR` flips are constant-level). Tests: 9 (extraction vs
naive, weight-split recombination, MPS closure vs naive,
both-geometry roundtrips for both paths incl. an untapped-word-axis
coexistence layout, pure-ROT, pure-XOR elision, two tamper suites);
95/95 green; existing proof bytes untouched.

**Structured taps: the single-tap shared-point COLLAPSE
(2026-07-27, follow-up).** k claims, each on a SINGLE tap (no XOR
mixing), all at ONE evaluation point, need none of the family or
translated-eq machinery: a single tap is a *weight transform* —
`Σ_p w[p]·tap(a)[p] = Σ_{p'} w[σ(p')]·a[p']` — and with the group
field inside the clear axis the transform keeps the row⊗column tensor
split, up to the word-offset carry, which contributes exactly one
extra branch with row weights advanced one `row_hi` step. So k claims
γ-collapse to at most **#columns × 2 plain single-column claims**
through the deployed claims-only path
(`prove/verify_mle_eval_mod_q_ligerito_tap_collapse`, statement tag
0x44; the verifier derives each branch value from the proof's own
forest-bound folds and checks their sum against `T = Σ γᵢcᵢ` —
soundness 1/q + the inner errors, no new proof fields). Measured
(`F2Z_AB_COLLAPSE=1`, the instance's 13 deduped streams as 13
individual claims at one point; medians of 5): clp = **53.5/177.3/603
ms** at n=22/24/26 (4 inner claims) vs the batched tap path's
215/609/3674 (13 claims pad to 16 tree-sets — the pad wall bites again
at n=26) and 13 independent proofs' 347/737/2125; proofs
**252/413/708 KB** vs 597/1054/1939 (−58/−61/−63 %); verify
11.5/19.5/37.8 ms vs 25.6/33.6/55.5. This is the right tool whenever
the claim set avoids XOR-mixing at one point: rotations-only claims
are pure column-weight transforms (branch 1 empty — one claim per
column), and offsets cost one extra weight branch, not a forest.
Guidance stack for tap workloads, best first: single-tap at one point
→ the collapse; XOR-mixed claims → the extraction (tap-claims) path;
the stream family only for dense stream reuse.

**Structured taps: the COMPOSED collapse — uniform outer ops over
MIXED sources (2026-07-27, follow-up session).** The collapse identity
`Σ_p w[p]·op(x)[p] = Σ_{p'} w[σ(p')]·x[p']` never used that its
source is a plain column XOR — only that the inner claim is provable.
Generalized (statement tag 0x45, `TapComposedClaim { source, outer,
claimed }`, `prove/verify_mle_eval_mod_q_ligerito_tap_composed`): the
source is a fixed **XOR-of-taps combination** `x = ⊕_t op_t(a_{i_t})`
(provable by the 0x42 path), and k claims `OUTER_i(x_{S_i})` at ONE
shared point γ-collapse to at most **#distinct-sources × 2 inner TAP
claims** — independent of k. The branch split depends only on
`(OUTER, layout)` (branch 1 = row weights advanced one `row_hi` step;
column side branch-masked + group-translated — the 0x44 builders
verbatim); the verifier derives each branch value from the proof's own
forest-bound fold vectors with `E_{S,β} = Σ_{i on S} γ_i·e_i^{(β)}`
and checks `Σ y = Σ γ_i·c_i` (soundness 1/q + the inner errors;
sources canonicalized by `tap_canonical_ops` — sort + char-2 pair
cancellation). **This is the XOR-mixed order-of-magnitude lever the
family construction wasn't**: schedule-shaped (shift-invariant)
workloads make every round a word-offset of ONE mixed combination, so
k rounds = 2 inner bodies total vs k padded forest bodies batched.
The outer offset's envelope stands alone (`off < 2^{s−g}`; it does
NOT compound with the source taps' offsets), so 48 rounds fit from
n = 22 (s = 11) at the harness split. Measured (`F2Z_AB_SCHED=1`,
48 claims `off^t(x)` of one σ-style source `ROT^7 a_0 ⊕ ROT^18 a_0 ⊕
SHIFT^3 a_0 ⊕ off^1 a_1`; claim values through the offset-FOLDED
extraction route, so every verified rep cross-checks the transform
algebra; alternated in-window medians of 5, FAST profile; vx48 = the
batched tap-claims path on the folded lists, 48 → 64 tree-sets):

| n | cmp (composed) | vx48 (batched) | ind48 | proofs (cmp/vx48) | verify (cmp/vx48) |
|----|----------------|----------------|-------|--------------------|--------------------|
| 22 | **39.2 ms** (2 inner) | 864.9 (22.1×) | 1405.8 (35.9×) | **189**/1996 KB | 18.8/320.8 ms |
| 24 | **94.8 ms** | 3242.3 (34.2×) | 2895.7 (30.6×) | **284**/3602 KB | 29.7/294.5 ms |
| 26 | **279.7 ms** | skipped† | 7334.8 (26.2×) | **448**/— KB | 58.0/— ms |

† the 64-set pad against ~7 GB free — and already INVERTED at n=24
(vx48 3242 vs ind48 2896: the pad wall pre-empts n=26). 48 XOR-mixed
claims land at **1.0–1.9× the cost of ONE claim** (single: 25.5/94.4/
146.0 ms) with per-claim proof bytes 3.9–9.3 KB; prover peak stays at
the single-proof footprint (26/95/366 MB vs the batched path's
64-set wall). CLI preset: `f2z <n> --taps sched` (rounds clip to the
shape's envelope). Known v1 slack: the two branch bodies of one
source duplicate their rings and extraction (identical tap lists —
the ring `s_v` depends on the exit point, not the row weights); rings
were ~4 % of prove, so this is bytes more than time. **Guidance stack
for tap workloads, updated**: single-tap or uniform-op-of-XOR-set at
one point → the 0x44 collapse; MANY outer ops of few mixed sources at
one point (schedules) → the 0x45 composed collapse; irreducibly
distinct XOR-mixed claims → the batched 0x42 path; the stream family
only for dense stream reuse. Tests: 4 new (schedule roundtrip with
inner-count asserts on both pack-cut geometries, folded-tap
distributed-extraction cross-check, statement/proof tampers,
canonicalization); 101/101 green; existing proof bytes untouched.

**Structured taps: BLOCKED batching kills the pad — and the 2-set
block is the sweet spot (2026-07-27, follow-up).** The batched
tap-claims path (0x42) padded its merged forest to `2^⌈log₂k⌉`
tree-sets (k=6 → 8: +33 % waste; k=48 → 64; the memory wall behind
the n=26 inversion and the shared-point line's 17.2 GB). Now claims
run in BLOCKS through the same batched common — each block its own
forest + presum absorbs and exit point (sequential FS composition;
per-claim exit points thread through the ring plans), all blocks
sharing the statement, the η-batched ring basis, and the ONE closing
Ligerito call; extraction is per block and freed between. Measured
block-size scan (the k=6 instance + the k=48 schedule baseline,
n=22–28): **2-set blocks are best-or-tie at every shape** — the
merged forest's marginal round-sharing saturates at two tree-sets
(the classic two-column shape the lazy kernels are tuned on) while
wider merges pay the cache regime (the 4+2 split cost +27 % at n=28)
— so the shipped policy is pairs + optional trailing singleton
(`TAP_CLAIM_BLOCK_CAP = 2`, structural, part of the proof shape;
k ≤ 2 keeps pre-blocking bytes, so `single` and the 2-body composed
schedule proofs are unchanged). The k=6 instance, before → after
(same-box medians of 5, `F2Z_AB_NO_FAMILY=1` unlocks n=28):

| n | vx6 padded (8 sets) | vx6 blocked (2+2+2) | ind6 | single |
|----|--------------------|---------------------|------|--------|
| 22 | 118.7 ms (4.7×) | **90.5 (3.64×)** | 149.8 (6.03×) | 24.8 ms |
| 24 | 319.3 (3.4×) | **250.5 (2.69×)** | 391.7 (4.21×) | 92.9 |
| 26† | 2006 (13.7×) | **713.1 (5.01×)** | 891.3 (6.26×) | 142.3 |
| 28 | — (wall, skipped) | **2441.0 (5.75×)** | 2616.4 (6.16×) | 424.8 |

† pre-P2 n=26 was churned-box; the blocked runs had 4–6 GB free. The
P2 accept criterion (vx6 ≤ 6.2× single at every n ≤ 28) is met, the
n=26 inversion is cured, and **the batch now beats independent proofs
at every measured shape** — the n=28 row exists for the first time
(peak ≈ 2 tree-sets for ANY k, where the pad scaled it with
`2^⌈log₂k⌉`). The schedule baselines improve too (vx48 = 24×2-set:
736.3/1898.1/5980.8 ms at n=22/24/26 — n=26 now RUNS), and the
composed collapse holds **18.8×/21.7×/21.7×** against the improved
baseline (cmp re-measured 39.1/87.6/275.6 ms, byte-identical).
Batched 0x42 proofs for k ≥ 3 change shape/bytes (EXPERIMENTAL API);
sizes shift ~+5–10 % from the extra per-block transcripts (vx6 n=24:
566 → 576 KB) — the price of the flat memory profile.

**Structured taps: `x_fold_extra` (δ) lands for the tap paths
(2026-07-27, follow-up) — proofs −30…−57 %, verify up to 5× faster,
prove faster too.** The tap machinery asserted δ = 0; now the low-δ
clear variables join the folded side across 0x42/0x44/0x45 (the
stream family 0x43 stays δ = 0). The structural fact that made this
cheap: the batched common's exit point is the FLAT x-coordinate list
— δ moves which coordinates the presum binds, not the list — so the
translated-eq rings, support tables, and MPS closures are
**δ-independent**; the work was the extraction re-split (regroup 2^δ
consecutive natural rows, as the vx path), the collapse branch
builders under the re-split (effective clear-axis fields `g' = g−δ`,
`amt' = amt≫δ`, `off` unchanged), and a δ-envelope for collapse
OUTER ops: `δ ≤ g` and `2^δ | amt` (pure-off schedule outers always
qualify, identities trivially; general amounts would need a carry
class at the δ cut — unbuilt). 0x42/0x45 SOURCE taps are
unconstrained. The trade: sent fold vectors shrink 2^δ× (the `us`
term — 68 % of vx6 bytes, ~58 % of the composed sched proof at
n=26); `q_rowbit` tables and the presum grow to `t'+δ` — measured,
δ WINS on every axis (the re-split moves the x tensor toward the
fold-heavy proof-size-optimal geometry; the taps harness's even
split was byte-suboptimal all along — at δ=3, n=24 even `single`
halves: 92.9 → 47.7 ms, 208 → 151 KB). The n=24 scan
(prove ms / proof KB / verify ms, medians):

| instance | δ=0 | δ=3 | δ=4 |
|----------|-----|-----|-----|
| cmp (48-claim sched) | 86.4 / 284 / 29.3 | 74.1 / 172 / 10.2 | **71.2 / 163 / 9.5** |
| vx48 (batched folded) | 1916.7 / 3861 / 305.9 | 1421.5 / 1163 / 254.8 | 1401.7 / **982** / 257.2 |
| vx6 (k=6 instance) | 250.5 / 576 / 21.9 | **183.3 / 250 / 17.1** | — |

Across n at δ=3 (cmp): 26.7 / 133 / 8.4 at n=22 (δ=0: 39.1/189/18.9)
and 225.5 / 222 / 13.6 at n=26 (δ=0: 275.6/448/58.5; δ=4:
222.7/205/11.6 — the proof **halves**). The knee is δ = 3–4; δ = 5
gives −3 KB more for the 2^{t'+5} table growth (86.7 ms). Env:
`F2Z_TAPS_DELTA` on `taps_ab` (family auto-skipped) and
`f2z --taps vx|sched`. Tests: +3 (extraction re-split vs manual
regroup, 0x42 δ=1/2 roundtrips with fold-shrink asserts, composed
δ=2 roundtrip + envelope rejection); 104/104 green both guard modes;
δ=0 proof bytes untouched.

**Structured taps: the cols4 config + the batched-path optimization
pass (2026-07-27, follow-up).** New 4-column instance (`--taps
cols4`, `F2Z_AB_COLS4=1`; the first `log_cols = 2` taps layout —
the machinery was already generic): identity claims on a₁..a₄ plus
the XOR-mixed pairs `ROT¹(a₁)⊕off¹(a₂)` and `ROT²(a₃)⊕off¹(a₄)`,
one shared point, through the blocked batched tap path (2+2+2;
XOR-mixed ⇒ the 0x42 route; bare ROT pinned as ROT¹). The
max-optimization pass on it and the batched path generally,
profile-driven (`OBLONG_PROFILE=1` now dumps in the cols4 A/B
block): attribution is **forest 61 % / Ligerito tail 22 % / rings
6 % / basis fills 3 % / extraction 0.4 %** — the path is at its
structural floor (each claim's derived bits must be forest-bound
once; 6 claims ≈ 6 bodies + one shared tail; the stream-family
alternative explodes to 143 channels here — claims on disjoint
stream supports multiply). What the pass landed: (1) the rings and
basis-fill phases built every member's translated-eq support tables
TWICE — now built once and reused (~4 % on cols4, neutral where
rings are thinner); (2) the δ knee for cols4 is **δ = 3** (the
re-split's fewer-but-taller forest trees also cut the per-tree
overhead); (3) the byte knee is `--profile custom:3:4` (r1/8).
Measured (A/B protocol, FAST, medians of 5):

| n | single | vx6 cols4 δ=0 | vx6 cols4 δ=3 | ind6 δ=3 |
|----|--------|----------------|----------------|-----------|
| 22 | 21.3 ms | 52.5 / 248 KB | **43.6 / 168 KB** | 124.9 (2.9×) |
| 24 | 26.6 ms | 125.2 / 380 KB | **99.3 / 215 KB** | 209.6 (2.1×) |
| 26 | 65.3 ms | — | **312.7 / 277 KB** | 420.0 (1.3×) |

Byte ladder at n=24, δ=3 (CLI, 6 claims): fast 209.4 → r1/4 168.8 →
**r1/8 153.3 (25.6 KiB/claim, prove ~116 ms, commit 2×)** → r1/16
144.1 (past the knee). Verify ~10–12 ms throughout.

The same pass applied to the **2-column k=6 instance** (per-claim
bodies are 2× cols4's — nv = 23 — and the 3-tap b-claims carry ~9
translated-eq members each, so rings run ~2× thicker: attribution
forest 49 % / lig 24 % / rings 11 % / fills 3 %): its time knee is
**δ = 4** (s = 12 starts one higher), byte config r1/8 again.
Measured (A/B protocol, FAST, medians of 5; δ=0 = the blocked
baseline of the P2 note):

| n | single δ4 | vx6 δ=0 | vx6 δ=4 | ind6 δ4 | proof vx6 δ4 |
|----|-----------|---------|----------|----------|---------------|
| 22 | 18.6 ms | 90.5 | **66.4** | 146.6 (2.2×) | 180 KB |
| 24 | 58.0 | 248.7 | **182.5** | 275.0 (1.5×) | 227 KB |
| 26 | 111.6 | 713.1 | **619.4** | 755.8 (1.2×) | 289 KB |

CLI byte ladder at n=24 δ4: fast 221.3 → r1/4 181.4 → **r1/8 163.6
KiB (27.3 KiB/claim)**; verify 19–22 ms. Against the session's
starting point (padded, δ=0): time −44/−43/−69 % at n=22/24/26 and
bytes −71 % at n=24 (566 → 164 KB). Remaining headroom on both
instances is ~10 % of prove (fixed per-block costs) plus the
line-wide forest-kernel levers (`riding the forest`, 8/16-case leaf
rounds) — body count itself is information-forced for XOR-mixed
claims of distinct shapes.

**Structured taps: 64-bit words (2026-07-27, follow-up) — the width
is free.** The group width is now a parameter: `F2Z_TAPS_GRP=6` on
the harness and `--taps-grp 6` on the CLI run the SAME cols4 and
2-column vx instances on 64-bit entry-axis words (g = 6). As the
machinery predicts (translated-eq chains are O(g), the carry-class
count is width-independent, extraction is whole-run gathers),
**every measured number lands in the 32-bit band**: cols4-64 at the
δ = 3 knee 46.1/103.0/337.2 ms at n=22/24/26 with proofs
169/216/277 KB (32-bit: 43.6/99.3/312.7, 168/215/277); vx-64 at the
δ = 4 knee 65.0/179.6/619.7 with proofs 179/—/289 KB (32-bit:
66.4/182.5/619.4, 180/227/289); byte knee `custom:3:4` unchanged
(cols4-64 n=24: 153.4 vs 153.3 KiB); verify unchanged. δ scans
re-run at g = 6 confirm the same knees (cols4 δ3, vx δ4; δ = 6 —
now inside the collapse envelope δ ≤ g — regresses on presum rounds
like δ5 did at g=5). What 64-bit words buy semantically at zero
cost: rotation amounts up to 63 (SHA-512/Blake2b-class constants)
and half the word count per trace. Envelope shifts only: s ≥ g + 2
(cols4-64 needs n ≥ 18) and the δ ceiling moves to 6. Tests: +2
(g=6 wide-amount extraction vs naive; end-to-end roundtrip with
amounts 33/40/47/63); 107/107 green both guard modes.

**Structured taps: the b3 G-step family (2026-07-27, follow-up) —
virtualize the defined columns, compose the checks.** The
8-role-vector family (a,b,c,d,a',b',c',d' with d' = ROT¹⁶(d⊕a'),
b' = ROT¹²(b⊕c'), off¹(d) = ROT⁸(d'⊕off¹a), off¹(b) = ROT⁷(b'⊕off¹c);
all eight opened at shared points), measured two ways
(`F2Z_AB_B3FAM=1`). OPT: commit SIX (d', b' virtual — relations 1–2
become definitions and vanish), open the eight through ONE 0x44
collapse (6 identity sets + {d,a'} at ROT¹⁶ — plain bodies, no
rings; δ = 4 since 2⁴ | 16) with the ROT¹² opening routed through
0x42 (8 ∤ 12 puts it off the collapse δ-envelope; 0x42 is
envelope-free), plus the two COMPOSED zero-checks
R₁ = off¹(d)⊕ROT²⁴(d)⊕ROT²⁴(a')⊕ROT⁸off¹(a),
R₂ = off¹(b)⊕ROT¹⁹(b)⊕ROT¹⁹(c')⊕ROT⁷off¹(c) (word-0 boundary rows
excluded by the zero-check weight masks) = 7 + 3 bodies. NAIVE:
commit all eight, open all eight (0x44 δ4), check the four relations
as 3-tap mixed vectors = 8 + 4 bodies. Measured (mixed layer δ = 4,
FAST, medians of 5; harness backward-generates the recurrence and
asserts the composed vectors vanish — the algebra self-check):
opt **63.8/123.9/326.1 ms, 275/349/438 KB** at n=22/24/26 vs naive
65.2/134.0/333.1, 283/357/450 (n=26 same-window churned box); at
g = 6 (64-bit words) opt 107.4 vs naive 124.8 (1.16×). Lessons
pinned: (1) virtualizing relation-defined columns deletes their
consistency bodies AND their witness columns; (2) composing
definitions into the residual checks (ROT⁸∘ROT¹⁶ = ROT²⁴) keeps the
check count at the number of INDEPENDENT relations; (3) the collapse
δ-envelope (2^δ | amt) can force a bad layer δ — route the offending
op through 0x42 instead of capping δ (the δ2 variant measured
SLOWER and fatter: 463.7 ms / 520 KB at n=26); (4) d, b cannot be
virtualized (their joint recurrence's resolvent is an O(#words) tap
list — the phase-0 succinctness wall). FOLLOW-UP — the fewer-body
routes, measured: the opening layer CAN compress to 6 bodies (two
j=2 shared-point families {d,a′}, {b,c′} + the {a},{c} idents + the
2 checks) or 4 bodies (two j=3 families absorbing a,c), with v(d′),
v(b′) delivered as the pair-XOR form claims at the PUBLICLY
ROT-relabeled point (an opening of d⊕a′ IS an opening of
d′ = ROT¹⁶(d⊕a′) — zero cost). Both arms landed in the harness and
verify — and both LOSE to the 10-body δ4 routing at real shapes:
fam6 74.0/151.0 ms, 510/669 KB and fam4 84.8/147.7 ms, 413/543 KB
vs opt 57.5/122.7 ms, 275/349 KB at n=22/24. Body count is not the
cost unit: the family paths run δ=0 (full-width folds and forests —
a δ4 collapse body costs ~0.55× a δ0 single), family bodies carry
1–4 AND discharge channels each (~13–20 ms), and the fam route pays
4 sub-proof tails vs 2. In δ0-single-claim EQUIVALENTS the 10-body
routing already costs ≈ 2.9 singles at n=24. The open lever that
could flip it: δ-enabling the RLC-family path + merging the two
families into one proof (projected ~10–20 %, bounded by the
channels). n=26 row (heavily churned box — 80 MB free — ratios are
same-window): opt 312.0 / 438 KB, naive 321.3, **fam6 310.6 = 1.00×
(a tie)**, fam4 350.5 / 713 KB — the fam6/opt trend across n =
1.29× → 1.23× → 1.00× at n=22/24/26: the family route's case-forest
economy GROWS with n while the collapse bodies stay
one-body-per-claim, so even δ0 families reach parity at n=26 and
project to WIN at n ≥ 28; δ-enabling them (which also fixes the 2×
byte gap — the full-width δ0 folds are the entire size loss) would
flip the verdict at every shape. n=28 CONFIRMS the crossover (two
runs, 67 MB free at start): **fam6 950.3 ms = 0.92× of opt** on the
steadier run (1031.4/1034.2 opt/naive) and 0.72× under peak churn —
fam6's j2-lazy working set is churn-IMMUNE (951.6/950.3 across
runs while every other arm swung ~25 %); fam4 needs
`F2Z_RLC_J34_LAZY=1` at this scale (1982.8 eager → 1414.4 lazy) and
still loses (the j3 channel + case-width cost). Bytes at n=28: opt
539 KB, fam6 1134, fam4 937 — the δ0 fold gap unchanged. Verdict:
the 6-body family structure WINS prover time from n=28 up at δ0
already, with the δ-family extension the remaining piece for the
byte axis and the smaller shapes.

**δ for the family paths — verified and measured (2026-07-27,
late).** The RLC-family core was p_x-parameterized all along
(extraction, folds, forests, presum, cascade, rings all derive
their shapes from `virtual_xor_params`), so δ needed no code — what
was missing was PROOF of a correct re-split: the new cross-split
consistency test pins it (product-form weights `colw0 = f ⊗ g`,
`rw_δ = rw0 ⊗ f`: the δ path must prove the SAME claimed values as
the flat reading; j = 2 and j = 3 with the level-2 cascade, δ = 1, 2,
tampers rejected; 109/109 green). Harness: `F2Z_AB_FAM_DELTA`.
Measured (fam6, δ4 vs δ0): time ~flat at n = 24–26 (147.5 vs 151.0;
306.0 vs 310.6 — the tree-count gain cancels against the presum/
discharge round growth), **−10 % at n = 29** (1728.3 vs 1915.2,
cross-window) where fam6 = **0.85× of opt same-window** (0.68× on
the churned δ0 window; n=29 row: opt 2041.2/581 KB, naive 1997.5,
fam6 1728.3/985 KB, fam4 3307.1 — fam4's δ4 presum growth × j3
channels regresses it; keep fam4 at δ0 + lazy). Bytes: fam6
−8/−13/−20 % at n=24/26/29 (669→613, 874→760, 1225→985 KB) — less
than the fold-vector share because **the fam route's byte floor is
its FOUR Ligerito blobs vs opt's two**: sub-proof merging (one
recursive call across fam1+fam2+idents+checks) is now the bigger
byte lever than δ, worth ~2 blobs ≈ 300–400 KB at n=29. **The MERGED multi-family proof (2026-07-28) — tag 0x47, the
fam-route tails eliminated.** The family core was refactored at its
natural seam (front = extraction/forests/presums/cascade/rings, all
absorbs; closure = r″/η/basis/ONE Ligerito) — byte-stable for the
landed single-family paths — and a merged API added:
`RlcFamilySpec` + `prove/verify_mle_eval_mod_q_ligerito_rlc_families_
shared_point` run k families' fronts in ONE transcript (per family:
γ's → rank-1 case weights → front, sequential FS) with ONE shared
closing call. Tests: merged roundtrip (two j2 + one j1 family, δ = 0
and 2, tampered values and cross-part rings rejected); 110/110.
Harness: `fam6m` in B3OPEN = {d,a′} j2 + {b,c′} j2 + {a} j1 + {c} j1
merged, δ4. Measured (8 openings, no checks; FAST):

| n | vx8 (1 tail) | fam6 (3 tails) | **fam6m (1 tail)** |
|----|--------------|----------------|--------------------|
| 24 | **68.3 / 178 KB** | 117.2 / 458 | 78.7 (1.15×) / 209 |
| 26 | 203.7 / 229 | 218.0 / 560 | **210.2 (1.03×) / 256** |
| 28 | 760.2 / 290 | 666.5 / 664 | **647.6 (0.85×) / 307** |
| 29 | 1962.0 / 313 | 1390.2 / 731 | **1314.4 (0.67×) / 334** |

The merge removes ~2 blobs (731 → 334 KB at n=29 — byte-parity with
vx8) and −5 % time; fam6m is the best arm from n ≈ 26 up on time at
near-vx8 bytes. Against the base ONE-opening floor at n=29
(1228.7 ms / 260 KiB): **fam6m delivers all 8 openings at 1.07×
prove / 1.28× bytes**. Updated openings routing: 0x44 below n ≈ 26;
the MERGED pair-family proof above.

**Openings-only b3 arms (2026-07-28): the family crossover survives
without the checks.** `F2Z_AB_B3OPEN=1` — commit SIX (d', b'
virtual), open all eight with NO relation bodies (v(d'), v(b') as
the pair-XOR forms at the relabeled point): `vx8` = ONE 0x44
sub-proof, 8 plain bodies incl. the two ident-op XOR sets (no rings,
one tail, δ4); `fam6` = two j2 families (δ4) + one 0x44 for {a},{c};
`fam4` = two j3 families (δ0 + lazy). Measured (FAST, medians):

| n | vx8 | fam6 | fam4 |
|----|-----|------|------|
| 24 | **66.1 ms / 178 KB** | 113.6 (1.72×) / 458 | 106.4 / 385 |
| 26 | **198.1 / 229** | 216.3 (1.09×) / 560 | 283.0 / 517 |
| 28 | 795.5 / **290** | **653.7 (0.82×)** / 664 | 970.7 / 705 |
| 29 | 1701.8 / **313** | **1361.2 (0.80×)** / 731 | 2178.2 / 748 |

Same shape as the with-checks contest: vx8 (the collapse) owns
n ≤ 26; fam6 crosses at n ≈ 27 and wins ~20 % at 28–29 — the pair
case-forests amortize 2 columns' bits at ~1.3× one body and stay
churn-lean while per-claim bodies pay full price at scale. Against
the base ONE-opening floor at n=29 (1228.7 ms): **fam6 delivers all
8 openings at 1.11×** (vx8: 1.39×). fam4 loses everywhere
openings-only (absorbing the idents into j3 families buys bodies but
pays channels + case width). Bytes: vx8 tracks base (313 vs 260 KB);
fam6's 731 KB is 3 sub-proof tails — the merge lever. Routing rule,
final: openings-only at one point → 0x44/0x46 below n ≈ 27, two-column
pair-families above (when XOR-image openings are in the set at all;
pure identity sets stay 0x44 at every n unless bodies ≫ 8).

**Openings-only measurement (2026-07-28; the outer protocol owns
constraint checking).** `F2Z_AB_OPEN8=1`: 8 committed columns, 8
identity MLE openings at ONE shared point through a single 0x44
sub-proof (8 plain bodies, one Ligerito tail, δ = 4 via
`F2Z_AB_OPEN_DELTA`), against the base prover's one-claim floor on
the same 2^n bits (`f2z <n> --profile fast`, same window):

| n | base (1 opening) | OPEN8 (8 openings) | ratio | marginal/opening |
|----|------------------|--------------------|-------|------------------|
| 26 | 153.6 ms / 195 KiB / 3.2 ms | 201.7 ms / 229 KB / 7.9 ms | **1.31×** | 22.3 ms |
| 29 | 1228.7 ms / 260 KiB / 4.8 ms | 1456.9 ms / 313 KB / 11.1 ms | **1.19×** | 172 ms (0.14× base) |

Eight openings for 1.19–1.31× the price of ONE; each opening beyond
the first costs ~0.14–0.15× a base proof (the collapse amortization:
shared statement, shared forest tails, shared Ligerito call, δ-thin
fold vectors). Routing rule under the openings-only scope: identity
openings of committed columns → ONE 0x44 (or 0x46 for per-claim
weights) sub-proof at the δ knee — no taps, no rings, no channels;
the tap/family machinery enters only when openings of DERIVED
vectors (rotated/shifted/XOR images) are wanted.

Compression
math for this family: one word = one G, 56 G-words per compression
⇒ 2^{n−8}/56 compressions per proof (~292/1.2k/4.7k/18.7k/37.4k at
n=22/24/26/28/29); fam6 at n=29 ≈ **46 µs per compression** for the
xor-rot layer + openings.

### RS rate study: lower-rate profiles (`F2Z_LIG_PROFILE`)

The bench's `F2Z_LIG_PROFILE=slim` selects flock's embedded SLIM profile —
fewer queries plus 16-bit per-level grinding at the same 100-bit
`johnson_ood` target as the default FAST (base rate 1/2). The slim
profile's base rate is whatever the flock checkout's current generation
says — it was **rate 1/4** when the first sweep below ran, then
regenerated to **rate 1/8** at k=6 (Johnson at rate 1/8 sanctioned;
ladder 1/8…1/128, L0 60 queries), and is now **(rate 1/8, k=4)** — so
the bench header prints the live geometry (`lig=slim@r1/8k4`). The
rate-1/4-generation sweep (n=22–32; since the
proof is Ligerito-dominated, halving the query cost nearly halves the
proof):

| n | prove fast | prove slim | proof fast | proof slim | Δproof |
|---|---|---|---|---|---|
| 22 | 18.9 ms | 29.6 ms | 276.7 KiB | 140.7 KiB | −49 % |
| 24 | 46.3 ms | 50.7 ms | 303.9 KiB | 159.1 KiB | −48 % |
| 26 | 146 ms | 168 ms | 346.8 KiB | 188.2 KiB | −46 % |
| 28 | 569 ms | 654 ms | 399.9 KiB | 229.1 KiB | −43 % |
| 30 | 3.15 s | 4.99 s* | 466.3 KiB | 280.0 KiB | −40 % |
| 31 | 8.92 s | 12.4 s* | 490.5 KiB | 296.1 KiB | −40 % |
| 32 | 29.8 s | 35.7 s* | 573.7 KiB | 371.6 KiB | −35 % |

The Ligerito blob itself halves at every shape (e.g. 343 → 174 KiB at
n=28, 415 → 215 at n=32); the forest side is untouched. Costs: commit ~2×
time and ~2.2× peak (the rate-1/4 codeword — n=32: 624 ms / 3.73 GB vs
390 ms / 2.64 GB), prove +9–15 % intrinsic (all in the open phase:
grinding + the 2× encode; verify is mostly slightly *faster* from fewer
queries), prove peak +~1 GB at n=32 (12.6 GB — still inside 16 GB).
(*The slim sweep's n ≥ 30 rows ran under initial memory pressure and
overstate the prove delta — their rate-independent forest phase inflated
vs the fast sweep's; the intrinsic overhead from the open+commit deltas is
~+10–15 %, falling to ~+3–5 % at n=32.) Below `m = 22` every profile
falls back to the ad-hoc rate-1/4 config, so n < 22 is profile-invariant.

**RS rate 1/8, four ways (n ≤ 28).** Two rate-1/8 configurations were
measured: `F2Z_LIG_PROFILE=r8` (the ad-hoc UDR generator at
`log_inv_rate = 3`, embedded-matching `initial_k = 6` — the
small-`initial_k` ad-hoc geometry is the catastrophic-commit trap; ~121
L0 queries, UNAUDITED probe) and the regenerated **Johnson slim at rate
1/8** (60 L0 queries + 16-bit grinding, ladder validated by flock's
`LigeritoSecurityConfig::validate` and reproduced by
`scripts/soundness.py --log-inv-rate 3 --query-grind 16 --target 100`):

| n | proof: fast(1/2) | udr(1/8) | johnson(1/4) | johnson(1/8) | prove fast → j-1/8 |
|---|---|---|---|---|---|
| 22 | 276.7 | 197.0 | 140.7 | **108.5 KiB** | 18.9 → 29.4 ms |
| 24 | 303.9 | 224.1 | 159.1 | **123.1 KiB** | 46.3 → 54.3 ms |
| 26 | 346.8 | 276.2 | 188.2 | **150.7 KiB** | 146 → 162 ms |
| 28 | 399.9 | 344.0 | 229.1 | **187.0 KiB** | 569 → 666 ms |

Findings: **the analysis is worth more than the rate** — at the same
rate 1/8, Johnson (60 queries) vs UDR (121) is 187 vs 344 KiB at n=28, a
46 % gap from the analysis alone; and under the Johnson analysis the
1/4 → 1/8 step buys a further ~15–23 %. Net vs the FAST default:
**proofs −53…−61 %** (n=28: 400 → 187 KiB) for **+11–17 % prove at
n ≥ 24** (+56 % at n=22, where the fixed per-level grinding dominates a
30 ms prove; the cost is all open phase + ~3× commit — commit peak
372 MB vs 168 at n=28 from the 8× codeword) and slightly *faster*
verify. The UDR `r8` probe proves at FAST speed (no grinding) but its
proofs stay far above both Johnson configurations — a geometry probe,
not a candidate. Protocol-grade `slim@r1/8` rows (idle-fronted sweep,
same protocol as the main tables; n < 22 is the profile-invariant
ad-hoc config):

| n | commit | forest+presum | ligerito open | prove | verify | proof | prove peak |
|---|---|---|---|---|---|---|---|
| 22 | 1.32 ms | 17.0 ms | 12.6 ms | 29.4 ms | 1.64 ms | 108.5 KiB | 26 MB |
| 24 | 4.38 ms | 42.0 ms | 13.7 ms | 54.3 ms | 2.31 ms | 123.1 KiB | 97 MB |
| 26 | 17.2 ms | 150 ms | 28.8 ms | 162 ms | 2.90 ms | 150.7 KiB | 373 MB |
| 28 | 65.7 ms | 558 ms | 97.7 ms | 666 ms | 3.60 ms | 187.0 KiB | 1.46 GB |

**Pushing proof size further: `F2Z_LIG_PROFILE=custom:<log_inv_rate>:<initial_k>`.**
The bench can build Johnson configs at any (base rate, L0 interleaving)
geometry — ladder per `scripts/soundness.py`'s rule, queries/grinding/OOD
solved against flock's own `paper_predicted_*` formulas and gated by
`LigeritoSecurityConfig::validate` (same 100-bit per-level target; these
are validator-checked but not part of flock's shipped TOML set). Two
levers beyond the slim default: lower rate (halves the codeword's rate,
~2× commit per step) and **smaller `initial_k`** (halves each query's
opened row; commit-neutral — the total codeword is `k`-independent).
Measured at n=28 (reference split, prove ~unchanged throughout — the
forest dominates it):

| config | commit | prove | proof | lig blob | peaks (commit/prove) |
|---|---|---|---|---|---|
| slim (r1/8, k6) | 82 ms | 636 ms | 187.0 KiB | 132.3 | 372 MB / 1.46 GB |
| custom:3:4 (r1/8, k4) | 76 ms | 618 ms | **154.6 KiB** | 99.9 | 420 MB / 1.54 GB |
| custom:4:4 (r1/16, k4) | 141 ms | 643 ms | **140.5 KiB** | 86.1 | 740 MB / 1.90 GB |
| custom:5:4 (r1/32, k4) | 276 ms | 648 ms | 134.7 KiB | 80.5 | 1.38 GB / 2.62 GB |

`custom:3:4` is a near-free −17 % (the `initial_k` 6 → 4 step alone);
`custom:4:4` reaches −25 % for ~2× commit; rate 1/32 is past the knee
(−6 KiB for another 2× commit — at that point the lig blob is SMALLER
than the forest side, whose `2^s·16 B` sent-folds term takes over).
Shifting the split to shrink that term is a bad trade at n=28 under the
current prover (t=17→19 costs +77 % prove for −21 KiB — the shared
`2^t` tables and per-layer costs steepen with n), so the frontier is
config-only at the reference split.

**`k = 4` is the knee at every rate** (k=3 backfires: the extra ladder
level adds queries), so the slim profile was regenerated once more —
**the slim default is now (rate 1/8, k=4)** = the `custom:3:4` point
(`examples/gen_lig_configs.rs` rewrites the flock checkout's slim TOMLs
for m = 22..=35 through the same validator-gated generator; the tables
above labeled `slim (r1/8, k6)` are the previous generation). The k=4
family, measured at n=28:

| config | commit | prove | proof | lig blob | peaks (commit/prove) |
|---|---|---|---|---|---|
| fast previous gen (r1/2, k6) | 22 ms | 569 ms | 399.9 KiB | 343.0 | 168 MB / 1.25 GB |
| **fast = r1/2, k4 — DEFAULT** | 26 ms | 587 ms | **237.8 KiB** | 181.5 | 180 MB / 1.27 GB |
| custom:2:4 (r1/4, k4) | 45 ms | 596 ms | **178.3 KiB** | 123.2 | 260 MB / 1.36 GB |
| **slim = r1/8, k4** | 76 ms | 618 ms | **154.6 KiB** | 99.9 | 420 MB / 1.54 GB |
| custom:4:4 (r1/16, k4) | 141 ms | 643 ms | 140.5 KiB | 86.1 | 740 MB / 1.90 GB |

The k lever alone at rate 1/2 is −40 % (400 → 238 KiB) at fast-like
commit cost, dominating the previous-generation rate-1/4 slim (229 KiB)
on every axis — so **both shipped profiles were regenerated at k=4**
(`gen_lig_configs -- 1 4 fast` / `-- 3 4 slim`): fast stays the rate-1/2
default (the reference tables above), slim the rate-1/8 proof-size
profile; fast's regeneration also moves it onto the slim generation's
16-bit query-grinding convention (183 vs 218 L0 queries — ~ms-scale
grinding for −29 KiB).

### The b127 field study (`GF(2^127)`)

`benches/field.rs` benches the GHASH field head-to-head against
`GF(2^127)` mod `X^127 + X + 1` ("b127", after Reilabs'
`ghash-powers-bench`) on the prover's hot patterns:

```sh
RUSTFLAGS="-C target-cpu=native" cargo bench --bench field --features unchecked
```

The field module (`src/poly/univariate/binary_b127.rs`) mirrors the
`GF(2^128)` pipeline (NEON-resident, wide accumulators, fused eqf kernels)
with a PMULL-free SHA3-BCAX trinomial reduction and a square-specialized
fold, and its multiplicative group has prime (Mersenne `M_127`) order —
every `α ∉ {0,1}` generates. The measured verdict (M4, interleaved-rep
harness): **b127 is 0.88–1.02× — equal at best, ~10 % behind on the
prover-dominant patterns**. The "~30 % faster than GHASH" folklore
replicates only against scalar-reduction GHASH baselines (Reilabs' own
bench: 1.22× on this box); against F2Z's PMULL-fold GHASH the trade
"fewer PMULLs, more shifts" loses — Apple's PMULL throughput makes the
GHASH fold nearly free. A protocol-level swap is in any case
architecturally blocked at the ring-switch (`[K:F₂] = 2^7` packing) and
flock's GHASH-native Ligerito; the full analysis and numbers are in
[`docs/b127-field.md`](docs/b127-field.md).

**The 16 GB ceiling is n = 32.** The peak is forest-dominated
(`F2_FOREST_SCHEDULE=l8` ⇒ ~`2^n · 2 B` for the forest + ~`2^n · 0.7 B`
retained by the commit hint): n=33 needs ≈ 22 GB, n=34 ≈ 40 GB, n=36 ≈
160 GB — beyond this machine regardless of schedule. Unlocks, in order of
realism: more RAM (a 64 GB box runs n=34–35 with today's code); an L/16
forest schedule (a fourth bit-driven leaf round — upstream engineering,
not yet built there either); a streamed two-pass forest (redesign).
Verifier time and proof size stay essentially flat (ms-class / sub-MB),
so the statement scales — prover RAM is the only wall.

## Layout

| Path | Contents |
|---|---|
| `src/pcs.rs` | Shared core: params, `GF(2^128)` exponent-fold math, α-generator binding, forest-leaf builders, mod-`q` chunking, SHA bit-tensor layout |
| `src/ligerito.rs` | Ring-switch primitives + the backend-independent prover/verifier prefix (merged forest + pre-sumcheck) |
| `src/ligerito_flock.rs` | The flock-backed opener: commit, ring-switch → recursive Ligerito, `prove`/`verify_mle_eval_mod_q_ligerito`, the proof object + its host codec |
| `src/merged_forest.rs` | The lazy merged GKR grand-product forest (L/4 default; L/8 via `F2_FOREST_SCHEDULE=l8`) |
| `src/proof_codec.rs` | The host proof-stream byte codec (§ *Proof-stream serialization*) |
| `src/piop/` | The eq-factored & multi-degree sumcheck drivers + the GKR product forest |
| `src/poly/` | `GF(2^128)` and `GF(2^127)` (NEON pipelines), bit-packed `F₂[X]` cells, MLE/eq utilities |
| `src/transcript/` | Blake3 Fiat–Shamir transcript + the `Transcribable` codec |
| `docs/DESIGN.md` | Protocol description, soundness chain, serialization format |
| `docs/b127-field.md` | The `GF(2^127)` field study: boundary analysis, head-to-head measurements |
| `docs/verifier-note/` | LaTeX note: the mod-q verifier line by line (Ligerito as a black box); build with `latexmk -pdf` |
| `docs/forest-gkr-note/` | LaTeX note: how the prover performs the forest GKR (merged statement, eq-factored driver, lazy L/4 / L/8 schedules, case-LUT rounds, discharge); build with `latexmk -pdf` |

## Caveats

- **Not zero-knowledge**: the Ligerito opening reveals the queried committed
  rows.
- **flock Merkle leaf/node domain separation (upstream-flagged, pre-production).**
  flock's `merkle` module does *not* domain-separate leaf vs internal-node
  hashing — its own module note flags it as a "micro-benchmark module, not
  production code". F2Z inherits flock's commitment/Merkle verbatim; treat the
  current Merkle binding accordingly until flock ships the fix.
- The verifier requires `α` to generate `GF(2^128)^×` (checked against the known
  factorization of `2^128 − 1`); the per-chunk fold magnitude must stay below
  `ord α`, enforced structurally by per-chunk range checks.
- Optimizations are tuned for Apple Silicon (PMULL); other targets run the
  portable scalar `GF(2^128)` pipeline, bit-identically.

## License

_License not yet chosen — placeholder. `flock-core` is `MIT OR Apache-2.0`._

## Provenance

Extracted from `zinc-plus` (Nethermind), branch `f2-int-unified-ligerito`
(2026-07): the state including the merged forest, the flock-backed Ligerito
opener, and the host proof-stream serialization. The optimization history and
measurement ledger live in that repository
(`documentation/f2x-sha-todo.md`). Repository URL: _placeholder._
