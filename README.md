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
```

Prover knobs (every configuration produces byte-identical proofs):
`F2Z_EQF_FUSE=0` disables pass fusion (restores the eager two-pass fold);
`F2Z_LUT3=0` disables the deeper L/4 LUT prefixes; `F2Z_EQF_NOKERNEL=1`
forces the generic (non-NEON) round/fold kernels (diagnostic);
`F2_FOREST_SCHEDULE=l8` opts into the L/8 forest memory schedule.
`F2Z_LIG_PROFILE` (bench-only, changes the proof: `fast` default /
`slim` = base RS rate 1/4 / `secure`) selects the embedded Ligerito
profile — see the SLIM section below.

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
`F2Z_LUT3`, opt-out `=0`). `schedule` is the forest memory schedule
(`F2_FOREST_SCHEDULE=l8` opt-in, required at n ≥ 31 on 16 GB). The
`forest` / `open` phase columns come from ONE profiled prove per shape
(`OBLONG_PROFILE=1`; forest = pow2 + forest GKR + fold-v + pre-sumcheck,
open = ring-switch + B-combination + recursive Ligerito; they sum to
within ~2 % of the prove median except where noted).

**Prover time** (`prove` = median; phases from the profiled prove):

| n | shape (t, s) | commit | forest+presum | ligerito open | prove | verify | prove peak | schedule |
|---|---|---|---|---|---|---|---|---|
| 16 | 10, 6 | 0.19 ms | 3.31 ms | 0.63 ms | 3.93 ms | 1.17 ms | 0.69 MB | L/4 |
| 18 | 12, 6 | 0.35 ms | 5.63 ms | 1.14 ms | 6.73 ms | 1.34 ms | 2.4 MB | L/4 |
| 20 | 13, 7 | 0.67 ms | 9.05 ms | 1.71 ms | 10.9 ms | 1.90 ms | 7.7 MB | L/4 |
| 22 | 14, 8 | 0.70 ms | 17.1 ms | 1.89 ms | 18.9 ms | 1.81 ms | 22.8 MB | L/4 |
| 24 | 15, 9 | 1.67 ms | 42.6 ms | 4.01 ms | 46.3 ms | 2.67 ms | 83.8 MB | L/4 |
| 26 | 16, 10 | 5.34 ms | 132 ms | 12.2 ms | 146 ms | 3.34 ms | 320 MB | L/4 |
| 28 | 17, 11 | 22.1 ms | 525 ms | 43.5 ms | 569 ms | 3.95 ms | 1.25 GB | L/4 |
| 30 | 18, 12 | 88.1 ms | 2.98 s | 139 ms | 3.15 s | 5.55 ms | 4.95 GB | L/4 |
| 31 | 19, 12 | 180 ms | 8.51 s | 314 ms | 8.92 s | 7.83 ms | 5.81 GB | l8 |
| 32 | 19, 13 | 390 ms | ~96 % | ~4 % | 29.8 s | 11.8 ms | 11.5 GB | l8 |

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
| 22 | 16.2 KiB | 2.0 KiB | 255.3 KiB | 276.7 KiB |
| 24 | 22.2 KiB | 2.0 KiB | 276.3 KiB | 303.9 KiB |
| 26 | 32.4 KiB | 2.0 KiB | 308.5 KiB | 346.8 KiB |
| 28 | 50.7 KiB | 2.0 KiB | 343.0 KiB | 399.9 KiB |
| 30 | 85.1 KiB | 2.0 KiB | 374.9 KiB | 466.3 KiB |
| 31 | 86.8 KiB | 2.0 KiB | 397.1 KiB | 490.5 KiB |
| 32 | 151.8 KiB | 2.0 KiB | 415.4 KiB | 573.7 KiB |

Reading notes: **time is forest-dominated** (84 % at n=16 rising to ~96 %
at n ≥ 31) while **bytes are Ligerito-dominated** (72–93 % of the proof is
the recursive opening; the forest side is 8–152 KiB and the ring-switch a
constant 2 KiB) — the inversion to keep in mind when optimizing either
axis. Proof sizes are unchanged from the pre-fusion table (the fused
prover is byte-identical); prove times improved ~10–15 % over the
previous defaults in same-session A/B at n ≥ 26 — differences vs the
previous table beyond that reflect measurement-session conditions.
Verify stays ms-class and proofs sub-MB throughout — prover RAM is the
only wall. The n=20→22 step in proof size (98 → 277 KiB) is the
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

### RS rate 1/4: the SLIM profile (`F2Z_LIG_PROFILE=slim`)

The bench's `F2Z_LIG_PROFILE=slim` selects flock's audited SLIM profile —
base RS rate 1/4 (`log_inv_rate = 2`, recursion levels 1/8…1/64), fewer
queries plus 16-bit per-level grinding, the same 100-bit `johnson_ood`
target as the default FAST (base rate 1/2). Since the proof is
Ligerito-dominated, halving the query cost nearly halves the proof:

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

**RS rate 1/8 (`F2Z_LIG_PROFILE=r8`, ad-hoc UDR — UNAUDITED probe).** No
embedded profile exists below rate 1/4, so `r8` goes through the ad-hoc
`default_config` generator at base `log_inv_rate = 3` with the embedded
profiles' interleaving (`initial_k = 6` — the small-`initial_k` ad-hoc
geometry is the catastrophic-commit trap; with 6 the commit is sane).
UDR needs ~121 L0 queries at rate 1/8 vs the audited Johnson SLIM's 90 at
rate 1/4, and the measurement (n ≤ 28) shows **analysis quality beating
rate**:

| n | prove fast/r8/slim | proof fast/r8/slim | commit fast/r8/slim |
|---|---|---|---|
| 22 | 18.9 / 19.3 / 29.6 ms | 276.7 / 197.0 / 140.7 KiB | 0.7 / 1.9 / 1.4 ms |
| 24 | 46.3 / 47.4 / 50.7 ms | 303.9 / 224.1 / 159.1 KiB | 1.7 / 4.6 / 2.5 ms |
| 26 | 146 / 149 / 168 ms | 346.8 / 276.2 / 188.2 KiB | 5.3 / 17.9 / 9.7 ms |
| 28 | 569 / 592 / 654 ms | 399.9 / 344.0 / 229.1 KiB | 22.1 / 69.1 / 50.4 ms |

r8 proves as fast as FAST (no grinding; fewer query openings offset the
8× encode) at ~3× commit, but its proofs (−14–29 % vs fast) stay ~30 %
LARGER than the audited slim's — the Johnson+grinding analysis at rate
1/4 dominates UDR at rate 1/8 on size at every shape. A Johnson-analyzed
rate-1/8 profile would need fewer queries than slim and could reorder
this, but producing one is a security-analysis task (none is embedded);
the `r8` numbers are a geometry probe, not a deployable configuration.

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
