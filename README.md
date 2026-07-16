# F2Z — an integer-MLE-evaluation PCS over an F₂ commitment

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
table). Everything hot — commit, additive-NTT encode, BaseFold/Ligerito folding
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

## Dependencies

- **`flock-core`** — the ring-switch / additive-NTT / BaseFold / Ligerito hot
  paths (Succinct Labs' *Flock*, `MIT OR Apache-2.0`). **Pinned as a LOCAL PATH
  dependency** in `Cargo.toml`
  (`flock-core = { path = "…/flock/crates/flock-core" }`), exactly as
  `zinc-plus` pins it — **adjust the path for your checkout**. flock-core itself
  pins `bincode 1.3` and `serde 1`.
- `crypto-primitives` / `crypto-bigint` (field & bigint), `blake3`, `rayon`.

## Building and testing

```sh
RUSTFLAGS="-C target-cpu=native" cargo test --release
RUSTFLAGS="-C target-cpu=native" cargo run --release --example reference_measure
# both merged-forest schedules are pinned byte-identical to the eager forest:
RUSTFLAGS="-C target-cpu=native" F2_FOREST_SCHEDULE=l8 cargo test --release \
  merged_forest
```

`-C target-cpu=native` is load-bearing on aarch64 (enables PMULL for the NEON
`GF(2^128)` pipeline and flock's NEON kernels). Features: `parallel` (default,
rayon), `unchecked` (release-style integer guards off). The suite covers the
merged-forest lazy-vs-eager byte-identity (both `F2_FOREST_SCHEDULE` schedules),
the mod-`q` Ligerito roundtrip with tamper / range / generator rejections, the
NEON-vs-scalar field equivalence (`neon_mul_matches_scalar_pipeline`), and the
serialization roundtrip + tampered-byte rejection.

### Reference measurement

Single machine (Apple M4, `-C target-cpu=native`, median of 5), one genuine
`F_q = 2^100 − 15` MLE opening (post the 2026-07-16 parallel-`flatten` t4
fix; `examples/reference_measure.rs`):

| shape | prove | verify | serialized proof |
|---|---|---|---|
| **n=16** (t=10, s=6, W=1, 1 chunk) | **3.9 ms** | **1.6 ms** | **43.2 KiB** |
| n=18 (t=12, s=6, W=1, 1 chunk) | 5.9 ms | 1.4 ms | 75.0 KiB |
| (t=4, s=8, W=32, 2 chunks) | 8.6 ms | 2.5 ms | 65.7 KiB |

The n=18 proof size (75.0 KiB) matches the source branch's ~71 KiB Ligerito
proof at n=18.

## Benchmarks

`benches/pcs.rs` (plain `harness = false` binary, no criterion) reports, per
shape: commit / prove / verify wall-clock (medians), serialized proof size,
codec round-trip time, and **peak heap** per phase — the live-heap high-water
("net outstanding bytes"), the same notion as flock's benches and zinc-plus's
`f2_int_ligerito_mem`, so the numbers compare directly across the three repos.

```sh
RUSTFLAGS="-C target-cpu=native" cargo bench --bench pcs --features unchecked
# specific shapes (t:s:W triples) and rep count:
F2Z_BENCH_SHAPES="10:6:1 14:8:1" F2Z_BENCH_REPS=5 \
  RUSTFLAGS="-C target-cpu=native" cargo bench --bench pcs --features unchecked
```

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
idle-fronted, one shape per process at n ≥ 24. `data` is the committed
instance size; `schedule` is the forest memory schedule
(`F2_FOREST_SCHEDULE=l8` opt-in, required at n ≥ 31 on 16 GB).

| n | shape (t, s) | data | commit | prove | verify | proof | prove peak | schedule |
|---|---|---|---|---|---|---|---|---|
| 16 | 10, 6 | 8 KiB | 0.48 ms | 4.00 ms | 1.22 ms | 43.2 KiB | 0.85 MB | L/4 |
| 18 | 12, 6 | 32 KiB | 0.32 ms | 7.63 ms | 1.66 ms | 75.0 KiB | 3.1 MB | L/4 |
| 20 | 13, 7 | 128 KiB | 0.74 ms | 17.4 ms | 2.86 ms | 98.1 KiB | 9.1 MB | L/4 |
| 22 | 14, 8 | 512 KiB | 0.87 ms | 25.2 ms | 1.89 ms | 276.7 KiB | 26.1 MB | L/4 |
| 24 | 15, 9 | 2 MiB | 2.03 ms | 69.9 ms | 2.97 ms | 303.9 KiB | 88.4 MB | L/4 |
| 26 | 16, 10 | 8 MiB | 9.01 ms | 202 ms | 3.07 ms | 346.8 KiB | 330 MB | L/4 |
| 28 | 17, 11 | 32 MiB | 60.9 ms | 1.10 s | 5.39 ms | 399.9 KiB | 1.27 GB | L/4 |
| 30 | 18, 12 | 128 MiB | 91 ms | 5.13 s | 9.3 ms | 466 KiB | 4.99 GB | L/4 |
| 31 | 19, 12 | 256 MiB | 186 ms | 11.2 s | 9.7 ms | 490 KiB | 5.81 GB | l8 |
| 32 | 19, 13 | 512 MiB | 359 ms | 32.8 s | 14.8 ms | 574 KiB | 11.5 GB | l8 |

Reading notes: prove scales ~3–3.5× per +2 in n while cache-resident,
easing toward ~5× per step once the forest working set exceeds cache
(n ≥ 26) — expect the wider end of the ±5–15 % run-to-run band there (a
second n=28 run measured 0.82 s). Verify stays ms-class and proofs
sub-MB throughout — prover RAM is the only wall. The n=20→22 step in
proof size (98 → 277 KiB) and the verify blip at n=22 are the
`sha_lig_configs` boundary: the audited embedded FAST profile takes over
at `m ≥ 22` (hardcoding the tiny ad-hoc config at big shapes instead is
catastrophic — n=28 commit measured 292 s ad-hoc vs tens of ms embedded).

Measurement protocol (inherited from the zinc-plus lore): idle the box first;
for quotable *time* numbers at big shapes run one shape per process (the peak
numbers reset per shape and are fine in one sweep); quote medians.

**Packed-rows commit.** The harnesses generate the instance straight into
per-column bit rows and commit via `commit_rs_ligerito_rows` — the `u128`
cell tensor (16 B per cell) never exists, so peak memory sits at the
packed/forest scale. (The pre-restructure dense path held ~16 B/bit —
4.28 GB and a 7.9 s commit at n=28 — and could not reach n ≥ 30 on 16 GB
at all; the pre-LTO build was a further ~1.7–3× slower at the big shapes.)

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
| `src/poly/` | `GF(2^128)` (NEON pipeline), bit-packed `F₂[X]` cells, MLE/eq utilities |
| `src/transcript/` | Blake3 Fiat–Shamir transcript + the `Transcribable` codec |
| `docs/DESIGN.md` | Protocol description, soundness chain, serialization format |

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
