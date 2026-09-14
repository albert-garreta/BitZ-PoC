# SHA-256 + ECDSA verifier: value-identical recomputations (2026-09-14)

Branch `sha256-ecdsa-verifier-opt`, off master `414c194` (the `218c601` prover
tip plus the bench-suite commit). Every change below recomputes a quantity the
verifier already derives from public data (statement, commitment, proof bytes,
prepared relation) by an exact identity; nothing absorbed into or squeezed from
the transcript moves, `security.rs`, grinding, query counts and the Ligerito
profile are untouched, every `Err` path stays, and the proofs are byte-identical
(the bench rows carry a BLAKE3 digest of the proof bytes; every A/B below reports
one digest across all binaries, and the 2^3 transcript pin recorded before any of
this work still passes). Each commit keeps the replaced computation as a
`#[cfg(test)]` oracle and adds the comparison test named in its section.

Starting point (2^7 compressions, rate 1/2, λ = 100, the suite fixture
`bench_results/suite-sha256-ecdsa-20260913/fixtures/i7-seed0.json`, medians of
2 × 5 interleaved samples, ms; 1 thread | 10 threads): verifier 19.4 | 9.8,
of which `mv:rswitch` 13.8 | 4.1 (`mqv:vaprime` 10.0 | 1.8, `mqv:vwprep` 3.7 | 2.2),
`ecdsa:coefficient_evaluate` 4.0 | 2.6, `ecdsa:matrix_projection` 1.3 | 1.4,
`mv:lig` 0.4 | 0.4. Binius64 (BaseFold, rate 1/2): 12.8 | 6.2.

**Result.** Verifier at 2^7: 19.5 → 4.4 ms at 1 thread (Binius64 12.8) and
9.7 → 4.2 ms at 10 threads (Binius64 6.2); seven commits, one proof digest
throughout, prover within noise (its shared functions gain 1–4 ms).

Measurement: A/B = the baseline binary and every intermediate state's binary
built with the root release profile (fat LTO, one codegen unit,
`-C target-cpu=native`), interleaved on the same fixture (`--method f2z-split
--r 7 --c 0 --target 100 --threads T --reps 5 --seed 0`, `RAYON_NUM_THREADS=T
HARDWARE_CONCURRENCY=T F2Z_LIG_PROFILE=custom:1:4`), two passes, 20 s
cool-downs, through `scripts/bench_gate.py`. Scopes are `verify_phases_seconds`
of the bench rows (nested scopes are inclusive).

Gates before every commit: `cargo test --release --features ecdsa --lib --
ecdsa_sha256 raw_monty spartan::sumcheck` (38 tests on master, 47 on the tip),
`cargo test --release --features ecdsa --test transcript_pins` (all 8 pins,
recorded before any of this work), `cargo test --release --features
sha256-ecdsa-compare --test sha256_ecdsa_comparison` (public-input and proof
tampering rejected), the vendored crate's `matrix_wengert` suite, and clippy
on the touched files (`cargo +1.97.1 clippy`: the pinned 1.98.1 toolchain is
a minimal profile without the clippy component; the touched files carry no
new warnings — the pre-existing ones are upstream's unused destructured
fields in the vendored tape and older `ligerito_flock` lints). No `unsafe`,
no new dependencies. (The measured binaries predate four lint-driven edits
made afterwards — a closure signature, two `#[cfg(test)]` attributes on
oracle-only items, the roots loop in iterator form — after which the tip
re-passed every gate; the clippy warning set of the tip equals master's.)

## Two structural facts the changes rest on

1. **The P-256 map is the identity.** `p_map` (the F₂-linear map from the
   committed P-256 source bits to the assignment cells) has 1,215,663 rows,
   each with exactly the one entry on its diagonal: the constraint generator
   calls `f2z` once per Boolean witness, in order. The tail's ring-switch
   weights are therefore the eq-tensor values
   `E[row_offset + 257 + (j − f_offset)]` at the tail source columns, never a
   per-column fold. (The 257 alias columns — the constant and the 256 digest
   bits read from the last compression's output cells — keep their fold.)
   The tail's pack phase `(h_offset − f_offset + 257) mod 128` is 0 for
   2^7 and above and `8·(n/8) mod 128` below (2^3: phase 8), so the engine's
   straddling-pack path is exercised by the pinned 2^3 instance.
2. **The tape's runs cover the tail.** The Wengert tape (259 levels, 97,986
   edges, 66,412 nodes) has 4,844 power groups covering 1,203,088 of the
   1,215,663 columns; only 12,575 columns are scalar inputs. The verifier
   needs only the scalar `⟨w, (A + xB + x²C)·eq⟩`, not the column vector.

## Commit 1 — `ecdsa:matrix_projection`: native coefficient residues

**What changed.** `ModQCoefficients::from_relation` (shared by prover and
verifier) reduces the 3,422 distinct matrix coefficients from their
two's-complement words (`RawMontyCtx::signed_words_residue`, Horner by the
residue of 2^64, the kernel the outer product tables adopted in `218c601`),
in parallel, instead of two `BigInt` remainders and a conversion each; the
words are stored once at relation build (`LocalRelation::coefficient_words`).
In the vendored tape, `WengertTape::prepare` reduces its 1,029 coefficients by
the same word-Horner kernel (`horner_reduce_2`, one Montgomery product by
`2^64·R mod q` per word) instead of the bit-serial `RuntimeModulus::reduce`
(which is bit-serial for every modulus below `2^127`, i.e. for every sampled
prime).

**Equivalence.** Both kernels compute the canonical residue `x mod q` of the
same integer: Horner over 64-bit words is `x = Σ w_i 2^{64 i}` evaluated
modulo q (each word is below `2^64 < q`, so it is a residue; the
`2^{64·len}` correction accounts for the sign of a two's-complement value);
the Montgomery conversion of a canonical residue is unique. Same words in,
same words out.

**Tests.** `coefficient_residues_match_bigint_projection` (both outer modes,
a transcript-sampled 113-bit prime and `2^127 − 1`; also round-trips the
stored words to the `BigInt`s) against the kept oracle
`ModQCoefficients::bigint_residues`; in the crate,
`horner_reduction_matches_bit_serial_reduce` (random two's-complement values
of every width up to nine words, both signs, extreme words, at a 113-bit
prime, `2^127 − 1` and `2^128 − 159`) against `RuntimeModulus::reduce`.
`signed_words_residue_matches_bigint` (existing) covers the kernel itself.

**Shared with the prover:** yes (`from_relation`, `prepare`); the prover's
`ecdsa:matrix_projection` moves by the same amount.

## Commit 2 — `ecdsa:coefficient_evaluate`: the tape in forward mode

**What changed.** `ModQCoefficients::evaluate_batched_matrix_mle` (verifier
only) no longer materializes the 1.2 M-entry tail `r·(A + xB + x²C)` by the
reverse pass and then sums it against the equality weights. It runs the tape
FORWARD: the tail cells' equality weights are the column values — scalar
columns `eq(offset + j) = low[·]·high[·]`, a power group's
`Σ_k 2^k eq(offset + first + k)` by the same backward recurrence
`Q[t] = low[t] + 2·Q[t + 1]` the run-structured sum already used — every sum
node takes `Σ coefficient · value[source]` over its terms, and the roots
close with the row weights (`PreparedWengertEvaluator::apply_forward_weighted`,
`ForwardColumns`, `TailEqualityColumns`). The forward program is the tape's
edge list grouped by sum node, built once at tape construction. The reverse
output buffer is now allocated by the first reverse pass, so a verifier never
touches the 19 MB.

**Equivalence.** The tape is a DAG program for the linear map `M` from
column values to the rows' `A`, `B`, `C` values. Reverse mode computes
`Mᵀw` (the column vector) and the verifier then forms `⟨Mᵀw, eq⟩`; forward
mode computes `M·eq` row by row and forms `⟨w, M·eq⟩`. These are the same
bilinear form by associativity in the field; the two programs are transposes
of each other edge for edge (the forward terms are exactly the reverse
edges, grouped by dependent instead of by source), and a power group's
`Σ_k 2^k · x_{first+k}` in forward mode matches its reverse expansion
`2^k · (adjoint_full + [k < low]·adjoint_low)`. Field arithmetic is exact, so
the summation order does not matter.

**Tests.** `forward_matrix_evaluation_matches_reverse` (both outer modes,
random claims and points, a sampled 113-bit prime and `2^127 − 1`) against the
kept oracle `evaluate_batched_matrix_mle_reverse`;
`streamed_matrix_evaluation_matches_prepared_mle` (the verifier's evaluation
equals the prover's materialized MLE at random and at Boolean points, now at a
sampled prime); in the crate, `forward_pass_matches_reverse_dot_product` (the
example tape and the SHA-256 compression tape, random weights and column
values, `2^127 − 1` and `2^128 − 159`).

**Shared with the prover:** no (the prover keeps the reverse pass for its
batched MLE; `WengertTape::from_recorder` builds the forward program for both,
a construction-time cost).

## Commit 3a — `dual_unpack` in closed form (shared)

**What changed.** `virt_batch::dual_unpack` (A⁻¹ of the GHASH dual basis,
`bit_a(ŝ) = c₀(X^a·s)`) is now O(1): `dual_pack` is a bit reversal of the
slots onto the monomials plus the seven `DUAL_CORR` bits, so slot 0 is bit 0
of the low word, slots 1..=63 are the reversal of the high word's bits 63..=1
(untouched by the corrections), the corrections are then known, and slots
64..=127 are the reversal of the corrected low word's bits together with bit 0
of the high word. The 128-step multiply-by-`X` chain is kept as the oracle
`dual_unpack_by_multiplication`.

**Equivalence.** The closed form is the inverse of `dual_pack` derived from
its definition; `dual_pack` is a bijection, so the inverse is unique.

**Tests.** `dual_unpack_closed_form_matches_multiplication_chain` (2,048 random
elements, every monomial, every dual-basis column, all-ones, zero); the
existing `dual_unpack_inverts_and_extracts_bits`.

**Shared with the prover:** yes — `PackedSourcePlanes::new` unpacks every
local column sum (6,888 per chunk for SHA-256) at every prove; the prover's
`mqv:planes` moves.

## Commit 3c — `mqv:vaprime`, `mqv:vwprep`: the verifier's basis through the plane engines

**What changed.** The verifier's ρ-batched Ligerito basis
`a′(y) = Σ_v Φ_ρ(W_{(y,v)})·A(e_v)` is computed by three engines over the
three parts of the weights instead of the per-cell kernel over the folded
weights (`verifier_a_prime`):

- the plain packed-source repetition (the SHA compressions and the constant
  column) by `PackedSourcePlanes::add_a_prime`, the prover's plane engine
  (its per-task kernel factored into `a_prime_task`, which the prover's
  `a_prime` still calls unchanged);
- the identity compact tail by the new `AffineTailPlanes`: the same
  dual-basis identity with the high row index `c = r >> t` as the instance
  factor `zc_l[c]` and the low row index `b` as the local column `eq_l[b]`,
  `a′_tail(y) = Σ_l Σ_c Σ_a ρ′_{l,a}(zc_l[c])·R_{l,a}(y, c)`, where the local
  planes `R` depend on the pack only through its aligned low-index block and
  the tail's constant phase — one 128-plane table per chunk and block; packs
  straddling a high-index step take the block's upper part with `c` and block
  0's lower part with `c + 1`; the two packs partially covered by the tail
  take masked pieces transposed on the spot;
- the chained cross-instance/boundary terms and the alias corrections by the
  existing per-pack kernel (`extra_a_prime_deltas`, factored out of the
  prover's `add_extra_a_prime`).

`VirtColumnWeights::new_factored_tail` (verifier only) keeps an identity tail
factored (`AffineTailWeights`) instead of folding 1.2 M columns and copying
them; `pack_weights` still evaluates every weight per cell, so the streamed
kernel and the tests see the same weights.

**Equivalence.** `W = W_plain + W_tail + W_extra` cell for cell (the three
parts partition the map's entries: the repetition's local map, the identity
tail's diagonal, and the chain/alias entries), `Φ_ρ` is F₂-linear and the
dual-basis combination is linear, so `a′ = a′_plain + a′_tail + a′_extra`. The
plain and tail engines use the identity
`bit_b(e·s) = Σ_a bit_a(e·A(e_b))·bit_a(ŝ)` (`c₀(X^u·A(e_v)) = δ_{uv}`), an exact
rearrangement pinned for the plain part by `virtual_planes_match_cellwise`.
For the tail, `E_r = Σ_l zc_l[c]·eq_l[b]` is exactly `VirtRowCoeffs::coeff(r)`
(the value the dense fold summed for the one entry of each column).

**Tests.** `affine_tail_planes_match_cellwise_sum` (the engine against the
per-cell sum on random tensors: one and two chunks, `t ∈ {7, 8, 10}`, phases
0/5/64/80, tails starting and ending inside a pack, tails shorter than a
pack); `verifier_basis_matches_streamed_basis_on_the_ecdsa_map` (the real
relation at 2^3 — phase 8, straddling packs — and 2^7, both outer modes, one
and two chunks: factored weights equal the dense weights pack for pack, and
`verifier_a_prime` equals `virtual_a_prime` on the dense weights for a random
ρ and the protocol's eq-tensor ρ; no modulus enters — the weights live in
GF(2^128)); the prover pins `virtual_planes_match_cellwise` and
`chained_compact_tail_weights_and_planes_match_generic`.

**Shared with the prover:** the plane engine's kernel (`a_prime_task`, a pure
factoring of `a_prime`) and `extra_a_prime_deltas`; the prover's values and
operation order are unchanged. Shared with the other virtual-map benches:
the SHA-chain and plain-SHA verifiers now take the plane engine for their
basis too (measured below).

## Commit 4 — thread gates and task sizes (speed only, shared)

**What changed.** (a) `build_eq_x_r_helper` (the eq-table builder used by
every prover and verifier) split every level across the rayon pool, down to
the one-parent level; each split is a pool dispatch, and a 15-variable table
made 15 of them. Levels with fewer than 2^12 parents now run inline. (b) The
verifier's basis engines run in tasks of 2^8 packs (64 tasks for a 2^21-cell
source) instead of the prover's 2^11 (8 tasks, of which the tail covered four
and the SHA part three, so ten threads were mostly idle). (c) Sub-scopes
inside the verifier's matrix evaluation and weight preparation.

**Equivalence.** Same products in the same per-element order; only the split
of independent work across threads changes (each pack, and each eq-table
entry, is written by exactly one task).

**Tests.** No new arithmetic; the existing tests and pins run through the
gated builder (the `spartan::sumcheck` and `raw_monty` suites build their eq
tables through it).

**Shared with the prover:** (a) — the prover's 10-thread time drops too
(44.7 → 40.7 ms at 2^7: its small eq tables also paid the dispatches).

## Commit 5 — `mqv:vrho`, `mqv:vplanes`: RhoTables from the sparse monomial images (shared); basis-only plane tables

**What changed.** `RhoTables::new` built its coefficient matrix
`C_{u,a} = Σ_b ρ_b·bit_a(X^u·A(e_b))` by 128 dual-image chains, 128 bit
transposes and 16,384 byte-table lookups. By the symmetry of the pairing
(`bit_a(g·A(e_b)) = c₀(g·A(e_b)·A(e_a))`), `C_{u,a} = Φ_ρ(X^u·A(e_a))`, and
`X^u·A(e_a)` is a monomial plus at most the seven dual-basis corrections,
shifted and reduced, so each entry is a sum over its few set bits along a
multiply-by-`X` chain per column. The verifier's `PackedSourcePlanes` also
skips the plane-major copy of its tables, which only the prover's batching
message reads.

**Equivalence.** The identity above; `rho_tables_match_transpose` compares
the read-off coefficients with the definition `Σ_b ρ_b·bit_a(e·A(e_b))`.

**Tests.** `rho_tables_match_transpose` (existing, now against the sparse
build), `affine_tail_planes_match_cellwise_sum`,
`verifier_basis_matches_streamed_basis_on_the_ecdsa_map`, the prover pins.

**Shared with the prover:** the RhoTables build (the prover's `a_prime` calls
it once per prove).

## Commit 6 — `mqv:vaprime_tail`: the tail's basis by per-block lookup tables

**What changed.** The tail's 594 high indices share 16 aligned low-index
blocks (t = 11), so for each block `m` the engine precomputes
`D_{l,m}[u] = Σ_a C_{u,a}·R_{l,a}(m)` (128 products per `u`, 128 × 16 per
chunk) and its 16 byte-position subset-sum tables; a full pack then costs 16
gathers instead of 128 multiply-accumulates:
`Σ_a ρ′_{l,a}(zc_l[c])·R_{l,a}(m) = Σ_a Σ_u bit_u(zc_l[c])·C_{u,a}·R_{l,a}(m)
= Σ_u bit_u(zc_l[c])·D_{l,m}[u]`. The mode is chosen by the tail's shape
(lookup when the tail spans at least 256 high indices per block; the plane
products otherwise) and never by the thread count.

**Equivalence.** `ρ′_{l,a}(e) = Σ_{u : bit_u(e)} C_{u,a}` (F₂-linearity of
`Φ_ρ`), then the sums are exchanged; the straddling packs use two extra
pseudo-blocks for their two halves.

**Tests.** `affine_tail_lookup_matches_products` (both modes agree pack for
pack on random tensors, every phase class, one and two chunks);
`affine_tail_planes_match_cellwise_sum` now alternates the mode against the
per-cell sum; `verifier_basis_matches_streamed_basis_on_the_ecdsa_map` (the
real map takes the lookup mode at both 2^3 and 2^7).

**Shared with the prover:** no.

## Commit 7 — `ecdsa:coefficient_evaluate`: the evaluation in raw residues

**What changed.** The verifier's matrix evaluation now stays in raw Montgomery
residues end to end: the row weights and the SHA factors are built with the
raw equality tables (`RawEqualityWeights`, `make_equality_factors_raw`), the
SHA part is evaluated as `(Σ_i instances[i]·eq(i)) · (Σ_h high[h]·Σ_l low[l]·sha[h·L+l])`
(the high factor applied once per block), and the constant and public-bit
terms use the tail's equality tables. The generic field element
(`MontyField`, 80 bytes with an embedded configuration) was costing more in
copies and configuration checks than in arithmetic. The prover converts the
shared builders' raw output to field elements where its materialized MLE
needs them.

**Equivalence.** Every quantity is the same product of the same residues;
`eq(index) = low[index mod L]·high[index / L]` with the tables split at half
the point, as before. Field arithmetic is exact, so the grouped SHA sum is
the factored MLE's evaluation.

**Tests.** `raw_factor_builders_match_field_builders` (row weights, public
bits, constant, instances and SHA factors residue for residue, and the
grouped raw dot against the factored MLE's evaluation; both outer modes, a
sampled prime and `2^127 − 1`) against the kept oracles
`build_row_weights_field` / `build_sha_factors_field`;
`forward_matrix_evaluation_matches_reverse` (the reverse oracle now runs on
the field-domain builders, so it is independent of every new piece);
`streamed_matrix_evaluation_matches_prepared_mle`.

**Shared with the prover:** the two builders (`build_row_weights`,
`build_sha_factors`); the prover's `coefficient_combine` gains their share.

## Which benches to rerun

- **SHA-256 + ECDSA (`tab:sha256-ecdsa-f2z-opt`, suite phase `sha-ecdsa`)**: every
  F2Z cell — both rates, both thread counts, every size. Verifier ≈ 4.4× lower,
  prover −2 % at 1 thread and −9 % at 10 threads. The Binius rows are
  unchanged.
- **Native SHA-256 comparison (`sha256_compressions` / `sha256_chain`,
  `run_native_sha256_compare.sh`)**, if it returns to the paper: the F2Z rows —
  verifier −67 % at 1 thread and −42 % at 10 threads (2^10), prover unchanged
  at 1 thread and −8 % at 10 threads.
- **Integer multiplication (`tab:f2z-u32-mul`, `tab:native-mul-u64`,
  `tab:native-mul-u128`), hybrid (`tab:hybrid-sha256-mul`, equal counts),
  MultiSwap (F2Z row), raw performance (`tab:f2z-raw-performance`)**: the base
  opener or a repeated/generic map, so only the eq-table gate applies —
  the 10-thread columns move (verifier −10 % class at u32 2^21; the MultiSwap
  10-thread verifier, 11.9 ms against 8.8 at 1 thread, is the same dispatch
  overhead), the 1-thread columns do not. Rerun the 10-thread F2Z cells.
- Binius64 native rows and the Binius64-with-F2Z-opener rows do not run any
  changed code except the eq-table builder inside the opener's verifier; not
  re-measured, expected within noise.

## Tried and dropped: the tape's Montgomery kernel

Replacing the vendored tape's two-limb FIOS Montgomery product by the
schoolbook-product-plus-two-REDC-rounds form of F2Z's raw kernel (the only
other candidate inside the 0.9 ms forward pass: 33,645 non-unit edges,
17,647 roots, 12,575 scalar inputs and 4,844 power sums over 53,837 sum
nodes, per the tape's shape) measured as a wash — `coefficient_evaluate`
1.38 → 1.39 ms at 1 thread, 1.55 → 1.55 at 10 (round 4 below) — and is not
on the branch. The forward pass is bound by its dependent chain of
two-term sums, not by the kernel.

## A/B tables

### Round 1: baseline and the states after commits 1, 2, 3a, 3c

2^7, rate 1/2, λ = 100, medians of 2 × 5 interleaved samples, ms. One proof
digest (`926468c1…81fd`, 135,265 bytes) across all five binaries.

| state | thr | verify | `mv:rswitch` | `mqv:vwprep` | `mqv:vaprime` | `coefficient_evaluate` | `matrix_projection` | `mv:lig` | prove |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| baseline (414c194) | 1 | 19.48 | 13.30 | 3.17 | 10.10 | 3.82 | 1.27 | 0.36 | 87.8 |
| + commit 1 (native residues) | 1 | 18.48 | 13.29 | 3.15 | 10.08 | 3.79 | **0.31** | 0.37 | 86.7 |
| + commit 2 (forward tape) | 1 | 16.28 | 13.34 | 3.19 | 10.11 | **1.75** | 0.16 | 0.36 | 86.7 |
| + commit 3a (dual_unpack) | 1 | 16.26 | 13.32 | 3.16 | 10.12 | 1.75 | 0.15 | 0.36 | 85.5 |
| + commit 3c (plane engines) | 1 | **5.92** | **2.98** | **0.27** | **2.68** | 1.75 | 0.16 | 0.36 | 85.8 |
| baseline | 10 | 9.69 | 3.96 | 2.08 | 1.78 | 2.60 | 1.45 | 0.41 | 45.6 |
| + commit 1 | 10 | 8.52 | 4.00 | 2.07 | 1.77 | 2.58 | **0.31** | 0.41 | 44.6 |
| + commit 2 | 10 | 8.06 | 3.94 | 2.06 | 1.76 | **2.28** | 0.16 | 0.41 | 44.6 |
| + commit 3a | 10 | 8.15 | 4.02 | 2.07 | 1.77 | 2.26 | 0.16 | 0.40 | 44.8 |
| + commit 3c | 10 | **6.90** | **2.72** | **1.14** | **1.49** | 2.29 | 0.16 | 0.42 | 44.5 |

Inside `mqv:vaprime` after commit 3c (1 thr | 10 thr): `mqv:vplanes` 0.30 | 0.24,
`mqv:vrho` 0.48 | 0.35, `mqv:vaprime_plain` 0.56 | 0.25, `mqv:vaprime_tail` 1.07 | 0.53,
`mqv:vaprime_extra` 0.26 | 0.11. Prover side (shared functions): `ecdsa:matrix_projection`
1.22 → 0.15 | 1.40 → 0.14 (commit 1), `mqv:planes` 1.43 → 0.31 | 0.42 → 0.30 (commit 3a);
`mqv:aprime`, `mqv:hs`, `mqv:wprep` unchanged (6.9 / 13.2 / 3.2 ms at 1 thread).
Binius64 (BaseFold, rate 1/2) on the same fixture: 12.8 | 6.2.

### Round 2: the states after commits 3c, 4, 5

| state | thr | verify | `mv:rswitch` | `mqv:vwprep` | `mqv:vaprime` | `mqv:vplanes` | `mqv:vrho` | plain | tail | extra | `coefficient_evaluate` | prove |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| baseline | 1 | 19.58 | 13.35 | 3.18 | 10.14 | — | — | — | — | — | 3.87 | 88.0 |
| commit 3c | 1 | 5.92 | 2.97 | 0.27 | 2.67 | 0.31 | 0.48 | 0.56 | 1.06 | 0.26 | 1.76 | 86.2 |
| + commit 4 (gates) | 1 | 5.67 | 2.88 | **0.14** | 2.73 | 0.30 | 0.49 | 0.58 | 1.10 | 0.26 | 1.69 | 84.8 |
| + commit 5 (RhoTables) | 1 | **5.36** | 2.59 | 0.14 | **2.44** | **0.22** | **0.30** | 0.57 | 1.08 | 0.26 | 1.67 | 84.6 |
| baseline | 10 | 9.61 | 3.96 | 2.06 | 1.77 | — | — | — | — | — | 2.58 | 46.0 |
| commit 3c | 10 | 6.93 | 2.77 | 1.12 | 1.49 | 0.23 | 0.37 | 0.25 | 0.50 | 0.10 | 2.29 | 44.7 |
| + commit 4 | 10 | **4.72** | **1.49** | **0.26** | **1.24** | 0.25 | 0.37 | **0.16** | **0.35** | 0.08 | **1.92** | **40.7** |
| + commit 5 | 10 | **4.61** | 1.39 | 0.25 | 1.13 | 0.18 | 0.32 | 0.16 | 0.35 | 0.08 | 1.91 | 40.7 |

Sub-scopes of `ecdsa:coefficient_evaluate` after commit 5 (1 thr | 10 thr):
`ce_forward` 0.92 | 1.05, `ce_sha_eval` 0.29 | 0.33, `ce_sha_factors` 0.25 | 0.29,
`ce_rows` 0.15 | 0.17, `ce_eq` 0.03 | 0.04, `ce_columns` 0.02 | 0.02.
The 10-thread verifier's serial sections run 10–15 % slower than at 1 thread
throughout (`mv:lig` 0.35 → 0.41, `mv:rhat` 0.09 → 0.15): the main thread
shares the cores with the idle pool.

### Round 3: the states after commits 5, 6, 7

| state | thr | verify | `mv:rswitch` | `mqv:vaprime` | tail | `coefficient_evaluate` | `ce_forward` | `ce_sha_factors` | `ce_rows` | `ce_sha_eval` | prove |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| baseline | 1 | 19.57 | 13.37 | 10.14 | — | 3.87 | — | — | — | — | 87.8 |
| commit 5 | 1 | 5.45 | 2.61 | 2.46 | 1.09 | 1.71 | 0.92 | 0.25 | 0.15 | 0.29 | 85.2 |
| + commit 6 (tail lookup) | 1 | 4.70 | **1.92** | **1.78** | **0.38** | 1.68 | 0.92 | 0.25 | 0.15 | 0.29 | 84.9 |
| + commit 7 (raw evaluation) | 1 | **4.42** | 1.93 | 1.79 | 0.38 | **1.36** | 0.93 | **0.23** | **0.09** | **0.08** | 84.5 |
| baseline | 10 | 9.70 | 3.98 | 1.77 | — | 2.59 | — | — | — | — | 45.5 |
| commit 5 | 10 | 4.61 | 1.40 | 1.16 | 0.36 | 1.91 | 1.05 | 0.29 | 0.17 | 0.33 | 41.3 |
| + commit 6 | 10 | 4.52 | 1.28 | 1.01 | **0.23** | 1.91 | 1.05 | 0.29 | 0.17 | 0.33 | 40.6 |
| + commit 7 | 10 | **4.22** | 1.29 | 1.04 | 0.22 | **1.54** | 1.05 | 0.26 | 0.11 | 0.09 | 40.7 |

### Round 4: the final state re-measured, and the dropped kernel change

| state | thr | verify | `mv:rswitch` | `mqv:vaprime` | `coefficient_evaluate` | prove |
|---|---:|---:|---:|---:|---:|---:|
| baseline | 1 | 19.60 | 13.37 | 10.15 | 3.86 | 88.1 |
| commit 7 (the branch tip) | 1 | **4.45** | 1.94 | 1.79 | 1.38 | 85.0 |
| + kernel change (dropped) | 1 | 4.44 | 1.92 | 1.78 | 1.39 | 85.4 |
| baseline | 10 | 9.66 | 4.01 | 1.82 | 2.57 | 46.6 |
| commit 7 (the branch tip) | 10 | **4.19** | 1.28 | 1.04 | 1.55 | 40.8 |
| + kernel change (dropped) | 10 | 4.16 | 1.30 | 1.04 | 1.55 | 41.1 |

What remains at the tip (1 thr | 10 thr, ms): `mqv:vaprime` 1.79 | 1.04
(`vaprime_plain` 0.57 | 0.16 — the SHA repetition's 6,888 packs at one
multiply-accumulate per cell; `vaprime_tail` 0.38 | 0.24; `vrho` 0.33 | 0.35 —
the 8 MiB byte tables; `vplanes` 0.23 | 0.17; `vaprime_extra` 0.26 | 0.08),
`coefficient_evaluate` 1.36 | 1.54 (`ce_forward` 0.92 | 1.05, the tape's
serial dependent chain; `ce_sha_factors` 0.23), `mv:lig` 0.36 | 0.42 (left
alone), `ecdsa:matrix_projection` 0.16, the forest/presum/roots/rhat
verification ≈ 0.3, the prime sampling 0.10, and the Spartan sumcheck
verifications. The serial sections run 10–15 % slower at 10 threads than at
1 (the main thread shares the cores with the pool).

**Shared code (other benches) — corrected.** The first check of the chained
SHA-256 bench reported here (and in commit `01d51a2`'s message: 40.60 → 40.63 ms
at 2^10) was invalid: the second worktree's build had silently reused the
baseline's artifacts (cargo keys local crates by package id, not by path), so
it compared the baseline with itself. Rebuilt with the local crates cleaned
and re-measured (2^10, λ = 100, fat LTO, interleaved 2 × 5, ms; 1 thr | 10 thr):

| bench | verifier before → after | prover before → after |
|---|---:|---:|
| chained SHA-256 (`sha256_chain`) | 41.35 → **12.38** \| 13.54 → **8.20** | 158.6 → 157.5 \| 70.0 → 64.7 |
| independent SHA-256 (`sha256_compressions`) | 48.97 → **16.32** \| 17.54 → **10.20** | 371.8 → 367.7 \| 111.1 → 102.2 |
| u32 multiplication (`f2z --mul 21`, base opener, no virtual map) | 10.01 → 9.86 \| 5.99 → **5.33** | 1664 → 1654 \| 399 → 394 |

The SHA benches take the plane engines for their basis (their maps are
packed-source repetitions): `mqv:vaprime` 37.2 → 7.6 ms at 2^10 in the
chained verifier's scope profile (`vaprime_plain` 4.7, `vaprime_extra` 2.2 —
the 1,024 chained terms). The u32 relation opens through the base path and
only sees the eq-table gate: at 10 threads `mv:rhat` 2.79 → 2.30 and
`mv:rswitch` 0.32 → 0.07 ms; at 1 thread nothing moves. The 10-thread provers
of all three gain 1–8 % from the same gate.

### Round 3: the states after commits 5, 6, 7

| state | thr | verify | `mv:rswitch` | `mqv:vaprime` | tail | `coefficient_evaluate` | `ce_forward` | `ce_sha_factors` | `ce_rows` | `ce_sha_eval` | prove |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| baseline | 1 | 19.57 | 13.37 | 10.14 | — | 3.87 | — | — | — | — | 87.8 |
| commit 5 | 1 | 5.45 | 2.61 | 2.46 | 1.09 | 1.71 | 0.92 | 0.25 | 0.15 | 0.29 | 85.2 |
| + commit 6 (tail lookup) | 1 | 4.70 | **1.92** | **1.78** | **0.38** | 1.68 | 0.92 | 0.25 | 0.15 | 0.29 | 84.9 |
| + commit 7 (raw evaluation) | 1 | **4.42** | 1.93 | 1.79 | 0.38 | **1.36** | 0.93 | **0.23** | **0.09** | **0.08** | 84.5 |
| baseline | 10 | 9.70 | 3.98 | 1.77 | — | 2.59 | — | — | — | — | 45.5 |
| commit 5 | 10 | 4.61 | 1.40 | 1.16 | 0.36 | 1.91 | 1.05 | 0.29 | 0.17 | 0.33 | 41.3 |
| + commit 6 | 10 | 4.52 | 1.28 | 1.01 | **0.23** | 1.91 | 1.05 | 0.29 | 0.17 | 0.33 | 40.6 |
| + commit 7 | 10 | **4.22** | 1.29 | 1.04 | 0.22 | **1.54** | 1.05 | 0.26 | 0.11 | 0.09 | 40.7 |

### Round 4: the final state re-measured, and the dropped kernel change

| state | thr | verify | `mv:rswitch` | `mqv:vaprime` | `coefficient_evaluate` | prove |
|---|---:|---:|---:|---:|---:|---:|
| baseline | 1 | 19.60 | 13.37 | 10.15 | 3.86 | 88.1 |
| commit 7 (the branch tip) | 1 | **4.45** | 1.94 | 1.79 | 1.38 | 85.0 |
| + kernel change (dropped) | 1 | 4.44 | 1.92 | 1.78 | 1.39 | 85.4 |
| baseline | 10 | 9.66 | 4.01 | 1.82 | 2.57 | 46.6 |
| commit 7 (the branch tip) | 10 | **4.19** | 1.28 | 1.04 | 1.55 | 40.8 |
| + kernel change (dropped) | 10 | 4.16 | 1.30 | 1.04 | 1.55 | 41.1 |

What remains at the tip (1 thr | 10 thr, ms): `mqv:vaprime` 1.79 | 1.04
(`vaprime_plain` 0.57 | 0.16 — the SHA repetition's 6,888 packs at one
multiply-accumulate per cell; `vaprime_tail` 0.38 | 0.24; `vrho` 0.33 | 0.35 —
the 8 MiB byte tables; `vplanes` 0.23 | 0.17; `vaprime_extra` 0.26 | 0.08),
`coefficient_evaluate` 1.36 | 1.54 (`ce_forward` 0.92 | 1.05, the tape's
serial dependent chain; `ce_sha_factors` 0.23), `mv:lig` 0.36 | 0.42 (left
alone), `ecdsa:matrix_projection` 0.16, the forest/presum/roots/rhat
verification ≈ 0.3, the prime sampling 0.10, and the Spartan sumcheck
verifications. The serial sections run 10–15 % slower at 10 threads than at
1 (the main thread shares the cores with the pool).

**Shared code (other benches).** The chained SHA-256 bench (`sha256_chain`,
2^10, λ = 100, 5 samples × 2 passes, interleaved, ms): verifier baseline
40.60 | 13.63, after commit 5 40.63 | 13.56; prover 152.0 | 68.5 → 153.1 | 71.4
(1 thr | 10 thr). Its verifier is dominated by terms outside the ring-switch
read-off, and the prover differences are within the 10-thread drift of that
bench (its 10-thread samples spread 66–74 ms).
