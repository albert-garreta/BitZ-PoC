# F2Z — an integer-MLE-evaluation PCS over an F₂ commitment

F2Z is a polynomial commitment scheme for **integer data committed over a
cheap characteristic-2 code**: it proves

```
MLE[INT(D)](r) = y ∈ F_q
```

for a `2^t × 2^s` matrix `D` of `W`-bit cells whose **bits** are committed
with an F₂-RAA Brakedown commitment. The integer row-fold is carried **in
the exponent** of `K = GF(2^128)` — `α^{v_c} = ∏_b α^{w_b·D[(b,c)]}`, each
factor affine in one committed bit — certified by a GKR grand-product
forest that touches the commitment only through `K`-linear queries; the
column combination `y = Σ_c e_c·v_c` is read off in the clear over `F_q`.
The construction is due to Lev Soukhanov (the char2-fieldswitch note, §9,
"A one-round, characteristic-2 Brakedown instantiation").

Why two fields: `K = GF(2^128)` binds each `v_c` in the exponent (no
parity collapse — `v_c` is sent and reused as an integer, never reduced
mod 2), while the evaluation field `F_q` (any characteristic ≠ 2 ring)
carries the final combination on the sent integers. Full-width `F_q`
row-weights are **chunked** (`L = ⌈q_bits/c_w⌉` sub-folds, each bound
injectively below `ord α`) and recombined in the clear, so the scheme is a
genuine `F_q` MLE opening over an unchanged F₂ commitment.

This repository is the standalone extraction of the **most-optimized
implementation** from the `zinc-plus` repository (branch
`f2-int-unified-ligerito`, 2026-07-15), including:

- the **bit-affine lazy forest**: the GKR's leaf layer is never
  materialised — leaves `1 + bit·τ` are consumed straight from the packed
  committed bits via case-LUT sumcheck rounds (`LeafBits`, shared
  `w·τ`-product tables, branchless masked accumulation);
- the **eq-factored sumcheck driver** with `WideMulAcc` delayed reduction
  (unreduced 256-bit carryless accumulation, one reduction per accumulator
  per round) and hand-fused ILP round/fold kernels;
- the **NEON-resident GF(2^128) pipeline** (aarch64): schoolbook 4-PMULL
  product + 3-PMULL fold reduction, 2-PMULL squaring, vector-register wide
  accumulators — no NEON↔GPR bounces anywhere on the multiply paths
  (measured on Apple M4: ~1.0 ns/mul streaming, ~0.85 ns/mul-equivalent
  inside the sumcheck kernels; a scalar-word pipeline remains on every
  other target, pinned bit-identical by `neon_mul_matches_scalar_pipeline`).

## Usage

```rust
use f2z::pcs::*;
use f2z::transcript::Blake3Transcript;

// Instance shape: 2^t × 2^s cells of word_bits-bit integers.
let p = IntEvalParams { t: 12, s: 8, word_bits: 1 };
let code = /* RaaF2Code, see `code_for` in the tests */;
let alpha = smallest_generator();           // any generator of GF(2^128)^×
let num_openings = 987;                     // per the code's distance calibration

// Commit the bits (before any challenge).
let hint = commit_bits(&code, &p, &data);   // -> Merkle root in hint.root

// Prove MLE[INT(D)](r) = y ∈ F_q  (row_weights_q = eq(r₁,·) lifts in [0,q)).
let mut pt = Blake3Transcript::new();
let proof = prove_mle_eval_mod_q(&mut pt, &hint, &p, &row_weights_q, q_bits, alpha, num_openings);

// Verify against the root and the claimed y (R = any char-≠2 ring).
let mut vt = Blake3Transcript::new();
verify_mle_eval_mod_q(&mut vt, &code, &hint.root, &proof, &p,
                      &row_weights_q, &col_weights, q_bits, alpha, y, num_openings)?;
```

`pcs::tests::run_mod_q_roundtrip` is the end-to-end reference (including a
concrete 100-bit prime `F_q` and the RAA code instantiation); the plain
integer evaluation (`prove` / `verify`) and the batched variants
(`commit_batch` / `prove_batch` / `verify_batch`) are exercised by the
sibling tests.

## Building and testing

```sh
cargo test --release                      # 75 tests
RUSTFLAGS="-C target-cpu=native" cargo test --release   # with the NEON pipeline
# measurement harnesses (idle the machine first):
RUSTFLAGS="-C target-cpu=native" cargo test --release ghash_mul_baseline -- --ignored --nocapture
RUSTFLAGS="-C target-cpu=native" cargo test --release scaling_sweep      -- --ignored --nocapture
RUSTFLAGS="-C target-cpu=native" cargo test --release cost_profile       -- --ignored --nocapture
```

`-C target-cpu=native` is load-bearing on aarch64 (enables PMULL for the
NEON pipeline). Features: `parallel` (default, rayon), `unchecked`
(release-style integer guards off).

## Layout

| Path | Contents |
|---|---|
| `src/pcs.rs` | The PCS: params, `commit_bits`, forest orchestration, `prove`/`verify`, mod-q chunking (`prove_mle_eval_mod_q`/`verify_mle_eval_mod_q`), batched variants, α-generator binding |
| `src/piop/` | The eq-factored sumcheck driver (case-LUT bit rounds, fused kernels) + the GKR product forest |
| `src/poly/` | `GF(2^128)` (NEON pipeline), bit-packed `F₂[X]` cells, MLE/eq utilities |
| `src/code/` | The F₂-RAA linear code (rate 1/4, seeded permutations) |
| `src/merkle.rs` | Blake3 Merkle tree over packed codeword columns |
| `src/transcript/` | Blake3 Fiat–Shamir transcript |
| `docs/DESIGN.md` | Protocol description, soundness chain, optimization history |

## Caveats

- **Not zero-knowledge**: openings reveal the queried committed bits.
- Brakedown-style proof size (the sampled columns dominate); the verifier
  requires `α` to generate `GF(2^128)^×` (checked against the known
  factorization of `2^128 − 1`).
- The per-tree fold magnitude must stay below `ord α` — the mod-q path
  enforces this structurally via per-chunk range checks.
- Optimizations are tuned for Apple Silicon (PMULL); other targets run the
  portable scalar pipeline, bit-identically.

## Provenance

Extracted from `zinc-plus` (Nethermind), branch `f2-int-unified-ligerito`,
2026-07-15 — the state including the NEON GF(2^128) pipeline (`641b008`,
`f6a9db5`). The optimization history and measurement ledger live in that
repository (`documentation/f2x-sha-todo.md`, "Integer-MLE-eval" /
"f2-int over RS+Ligerito" / "GHASH" entries), and the as-deployed protocol
note in `documentation/f2-int-eval-doc/`. Omitted relative to the source
branch: the flock/Ligerito recursive opener (external dependency), the
merged multi-claim forest and virtual-XOR claims (SHA-host machinery), and
the host proof-stream serialization — F2Z keeps the self-contained RAA
flat-Brakedown opener.
