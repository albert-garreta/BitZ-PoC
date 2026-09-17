# Binius64 with the BitZ opener (`binius64-ligerito`)

A comparison scheme that answers "what if Binius64 used BitZ's binary-field
PCS?": Binius64's own circuits and PIOP, unchanged, with every oracle
committed and opened by the opener BitZ itself uses — default rate 1/2, the Johnson
(list-decoding) proximity regime, fold and query grinding, and Round 0 (the
out-of-domain sample). It is available in the supported benchmarks listed below. The composed SHA+P-256 ECDSA circuit is currently unsupported because it contains BMUL constraints.

| bench | scheme id | what it measures |
| --- | --- | --- |
| `benches/mul_e2e_compare` (u32, BabyBear, u64, u128) | `binius64-ligerito` | Binius64's native multiplication circuits, end to end |
| `benches/sha256_e2e_compare` | `binius64-ligerito` | Binius64's two-lane SHA-256 circuit, end to end |
| `benches/hybrid_u32_sha256` | mode `binius-ligerito` | the all-Binius circuit (four-limb mod-2^32 gadget + chained SHA) |
| `benches/u32_pcs_compare`, `benches/baby_bear_pcs_compare` | `bitz-ligerito-binary` | PCS only: the Binius64 packed rows and the identical bit-MLE claim, opened by the BitZ opener instead of BaseFold |

Code: `src/binary_pcs.rs` (the opener as a stand-alone binary PCS) and
`src/binius_ligerito/` (the Binius64 PIOP adapter; feature `binius64-bench`).

## Protocol

1. **Statement.** The transcript is seeded with a digest of the constraint
   system's shape, the opener configuration of every oracle, and the public
   words.
2. **PIOP prefix.** `IOPProver::prove_to_evaluation` — the fork's exposure of
   Binius64's prover up to the witness evaluation claim its ring switch would
   consume: the IntMul, BinMul (the GHASH-field multiplication zerocheck,
   which commits no extra oracle), BitAnd, zero and shift reductions, exactly
   as upstream, on a BLAKE3 transcript (`src/binius_ligerito/channel.rs`).
3. **Oracles.** Every `send_oracle` the PIOP makes — the packed witness, and
   the IntMul reduction's logup* pushforward (2^16 words) when the circuit
   multiplies — is committed by `BinaryPcs`: an interleaved Reed–Solomon
   codeword at rate 1/2 by default (`Prepared::with_rate` / the bench's
   `BITZ_BINIUS_LOG_INV_RATE` select another; the rate is part of the
   statement digest), 32 lanes (512-byte leaves), BLAKE3 Merkle tree. The
   root is bound into the transcript and **Round 0 is taken immediately**:
   the prover grinds, the verifier draws `ζ`, the prover sends
   `y = MLE[oracle](ζ, ζ², ζ⁴, …)`. This pins the committed word to one
   element of its Johnson list before the next challenge, which is what the
   paper's theorem requires of a Johnson-regime opener.
4. **Relations.** Oracle linear relations the PIOP queues
   (`prove_oracle_relation`) are recorded, not opened — the same deferral
   Binius64's BaseFold channel performs at `finish()`. The witness evaluation
   claim is a bit-MLE claim; each pushforward relation is
   `⟨transparent, oracle⟩ = claim` with a transparent basis the verifier can
   evaluate anywhere (the closure Binius64 hands its verifier channel). The
   IntMul reduction (fork rev `bc73510`, transparent logup*) queues two on
   its one pushforward oracle: an eq-basis evaluation claim and a product
   claim against the power table itself; the adapter combines them under one
   draw into a single opening.
5. **Openings.** After the PIOP, the evaluation value and every relation
   claim are bound, and each opening runs on its own fork of the transcript
   (domain-separated by oracle index): the witness claim through BitZ's ring
   switch (128 partial evaluations) and one Ligerito continuation; each other
   oracle's relations (combined by one draw if there are several) through one
   Ligerito continuation on that oracle's basis. Every continuation batches
   its oracle's Round-0 claim in with one draw `η_ood`, exactly as the hybrid
   does. The forks are load-bearing: flock's Ligerito prover and verifier end
   their final level in different transcript states (never visible before,
   since every other protocol in this repository ends with its Ligerito
   opening), so a second opening run sequentially on the same transcript
   draws different challenges on the two sides and rejects.
6. **Verifier.** Replays the PIOP prefix with Binius64's own
   `verify_to_evaluation`, checks each Round 0, and runs flock's succinct
   basis verifier per opening; the pushforward's basis is evaluated at the
   residual through Binius64's transparent closure.

Proof bytes = PIOP messages + every oracle root and Round-0 message + the
witness opening (ring switch + Ligerito) + one Ligerito opening per oracle
with relations, in a canonical codec (`Prepared::proof_from_bytes` rejects
non-canonical or trailing bytes).

## Security accounting

`Prepared::security()` reports both a whole-protocol union bound and the
round-by-round minimum. `Prepared::with_options` selects which accounting
model gates the proof at 100 bits; the default uses the union bound, in the
style of `hybrid::security::account`:

- Binius64 PIOP: the AND/zero/shift overcount `4096 · (log witness words +
  log AND + log zero + 64) / 2^128`; when the circuit multiplies integers, an
  IntMul overcount `4096 · (log IMUL + 64 + 16 + 8) / 2^128` over the 64-layer
  GKR step, the Frobenius/product sumchecks, the limb product check and the
  logup* lookup over the 2^16-row generator table; and when it multiplies in
  the GHASH field (the P-256 gadget's select lowering, for example), a BinMul
  overcount `4096 · (log BMUL + 64 + 8) / 2^128` over the degree-2 zerocheck
  rounds, the word-domain collapse into the shift claim and the batching
  draws (BinMul commits no extra oracle);
- per oracle: Round 0 (`C(L_δ,2)·(2^{m_p}−1)/|K|` at level 0's Johnson
  parameters, topped up to 108 bits by proof of work), every Ligerito level's
  proximity folds (with fold grinding), queries (with query grinding) and the
  deeper levels' out-of-domain samples, the field rounds and the `η_ood` draw;
- the ring switch (128/|K|).

The opener's round-by-round target is the **smallest** in 100..=112 whose
selected accounting clears 100 bits (the hybrid at rate 1/2 uses 106; here
the union bound uses 104 at 2^10 SHA
compressions, for example), and the achieved bits are reported. This modeled
composition bound includes grinding. BitZ rows separately report economic
per-challenge bounds and a statistical bound without grinding. Binius64's own
"100 bits" is its FRI query-phase
target only (`calculate_n_test_queries`), which counts neither its folding
phase nor its PIOP. The opener's rate follows the campaign's Binius rate
(`BITZ_BINIUS_LOG_INV_RATE`, default 1 = rate 1/2; 3 = rate 1/8, through
`Prepared::with_rate`). The 2026-09-13 suite runs the opener rows at both
rates under the round-by-round accounting
(`BITZ_BINIUS_LIGERITO_ACCOUNTING=rbr`) — the model the paper's tables
render; the union bound stays available and is recorded alongside.

`BinaryPcs::with_log_inv_rate` and `Prepared::with_log_inv_rate` also accept
rates 1/2, 1/4, and 1/8. Native multiplication rows can override the campaign
rate with `BITZ_BINIUS_LIGERITO_LOG_INV_RATE=1|2|3`.

As everywhere in this repository the bound is algebraic/IOP-level under
BLAKE3 Fiat–Shamir and 256-bit Merkle hashing; it is not an unconditional
Fiat–Shamir theorem.

## Running

```sh
# Native multiplication tables (adds the binius64-ligerito rows at the campaign's
# Binius rate, BITZ_BINIUS_LOG_INV_RATE=1 (default, rate 1/2) or 3 (rate 1/8), gated
# under BITZ_BINIUS_LIGERITO_ACCOUNTING=union (default) or rbr (round-by-round)).
BITZ_BENCH_SHAPES="15 16" BITZ_BENCH_REPS=5 BITZ_MUL_COMPARE_WORKLOADS="u32" \
BITZ_MUL_COMPARE_BACKENDS="bitz binius64 binius64-ligerito" \
RAYON_NUM_THREADS=8 bash scripts/run_native_mul_compare.sh
python3 scripts/native_mul_table.py PerfRuns/<run> --workload u32

# SHA-256 comparison.
BITZ_SHA_COMPARE_BACKENDS=binius64,binius64-ligerito BITZ_SHA_COMPARE_LOG_INV_RATE=3 \
  bash scripts/run_native_sha256_compare.sh

# Hybrid table: the all-Binius circuit with the BitZ opener, at the campaign's
# rate and accounting (the same knobs as the native-mul rows).
RUSTFLAGS="-C target-cpu=native" RAYON_NUM_THREADS=10 \
BITZ_BINIUS_LOG_INV_RATE=3 BITZ_BINIUS_LIGERITO_ACCOUNTING=rbr \
  cargo bench --bench hybrid_u32_sha256 --features hybrid -- \
  --sweep --mode binius-ligerito --iterations 11

# PCS-only rows.
BITZ_PCS_COMPARE_BACKENDS="binius64-basefold bitz-ligerito-binary" BITZ_BINIUS_LOG_INV_RATE=3 \
  cargo bench --bench u32_pcs_compare --features bench-internals,plonky3-whir-bench,binius64-bench
```

Unit tests: `cargo test --release --lib --features binius64-bench -- binary_pcs binius_ligerito`
(a multiplication circuit with the two-oracle path, an AND-only circuit, the
opener's bit-MLE and arbitrary-basis openings, tamper rejection).

## The allocator: use Binius64's pool, not the global allocator (2026-09-12)

`Prepared` holds a `binius_compute::BufferPool` and passes `&self.pool` to
`prove_to_evaluation`, which is what Binius64's own `Prover` does
(`prove.rs`: `let alloc = &self.pool`, a pool kept for the prover's lifetime
and recycled across proofs). The adapter previously passed `GlobalAllocator`.

That one line was worth **14 % of the whole prover** at u64 2^20, and it sat
entirely inside the *shared* PIOP, so it was measured as if it were a cost of
the BitZ opener:

| phase (u64 2^20, ms) | Binius64 own | opener, GlobalAllocator | opener, BufferPool |
| --- | ---: | ---: | ---: |
| commit | 29.3 | 35.1 | 35.0 |
| PIOP | 483 | 552 | 490 / 482 (two runs) |
| opening | 21.3 | 37.7 | 38.1 |
| total | 535 | 626 | 564 / 556 |

The PIOP phase lands on Binius64's own 483 ms, leaving a 4-5 % gap that is
genuinely the PCS: the rate-1/2 codeword and 32-lane BLAKE3 Merkle tree in
`commit`, and Round 0's two out-of-domain evaluations plus two Johnson
openings with grinding in `opening`, against one BaseFold opening. Proof bytes
are byte-identical (342472 / 389360 / 417976 at 2^15 / 2^18 / 2^20).

Two things to keep in mind:

- **It is size-dependent.** At 2^15 and 2^18 the pool changes nothing (PIOP
  27.9 vs 27.9, 131.8 vs 133.5): only 2^20-scale buffers are large enough for
  malloc/mmap churn and page faults to show up.
- **It costs retained memory, and that reaches the verifier.** Peak RSS at
  u64 2^20 goes 6.05 -> 6.58 GB, and verification goes 10.4 -> 15.4 ms
  (reproducible), because the pool keeps its blocks and the verifier running
  next in the same process allocates cold pages. Binius64's own rows have
  always paid exactly this (`src/hybrid/sha.rs` documents the same effect), so
  pooling both sides is what makes the verifier column comparable too - before
  this change our verifier numbers were flattered.

- **When the working set already exceeds RAM, the pool is a large LOSS.** At
  u128 2^20 (which pages in every scheme) the retained blocks deepen the swap
  footprint - peak swap 7.2 -> 10.7-12.6 GB - and the prover goes 7.6 -> 17.7 s
  with verification 154 -> 1201 ms. That is not a regression to fix: Binius64's
  own row has always paid it (18.3 s / 1218 ms at the same shape), and the
  pre-pool opener looked 2.4x faster there purely because it was not holding
  pooled memory. Pooling both sides is what makes that row honest.

With both sides pooled the opener's prover now tracks Binius64's own within a
few percent at every u128 size - 2^17 310 vs 294 ms, 2^18 597 vs 577, 2^19
1231 vs 1233 (parity), 2^20 17.7 vs 18.3 s - where the GlobalAllocator rows
read +17 %, +18 %, +14 % and then a spurious -58 %. The verifier columns
converge the same way (2^19: 39.0 vs 39.1 ms, was 31.8).

The same fix was applied to `benches/integer_pcs_compare/binius.rs`, where
`GlobalAllocator` was handicapping **Binius64's own** BaseFold rows; any
PCS-compare numbers recorded before 2026-09-12 predate it. `src/hybrid/sha.rs`
was already pooled. BitZ's own prover does not allocate through Binius's
`Allocator` at all, and the analogous idea for it - a retained scratch arena -
was **measured dead** (`docs/fields-witch-compare.md`: page reclaims at n = 27
are 63.7k for one proof and 68.6k for three, i.e. ~2-3 ms per proof after
warm-up, because macOS libmalloc already retains the large chunks).

## Four configurations for the paper tables (2026-09-12, Apple M5 24 GB, 8 threads)

The `binius64-ligerito` rows of `paper/native-mul{,-u64,-u128}-table.tex` are
measured in four configurations, 2^15..2^20, one backend per campaign process,
5 samples after one warm-up, the top size re-measured alone (the peak-RSS
squeeze), `PerfRuns/rerun-{u32,u64,u128}-lig4-{r2,r8}-{union,rbr}*`:

| table family | rate | 100-bit gate | per-round target | L0 queries | L0 fold grind (2^15→2^20) | achieved | same terms under the other model |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `binius64-ligerito@1` | 1/2 | union bound over every term | 105 | 194 | 16→21 | 100.2–100.4 | round-by-round 105.0 |
| `binius64-ligerito@3` | 1/8 | union bound | 105 | 63 | 17→22 | 100.3–100.6 | round-by-round 105.0 |
| `binius64-ligerito-rbr@1` | 1/2 | round-by-round minimum | 100 | 183 | 11→16 | 100.0 | union 95.2–95.4 |
| `binius64-ligerito-rbr@3` | 1/8 | round-by-round minimum | 100 | 60 | 12→17 | 100.0–100.1 | union 95.3–95.7 |

The round-by-round model is the one BitZ's own rows report (`SoundnessAccounting::
achieved_bits`, the minimum over terms; the opener's level-0 geometry is then
identical to BitZ's `custom:1:4` / `custom:3:4` at 32 lanes). The union bound over
the ~30 terms of a two-opening proof costs about 5 bits, hence the 105-bit
per-round target, 6 more fold-grinding bits and 11 / 3 more level-0 queries.
The opener's fold terms are counted one per fold round (each is exactly
`2^-(eps_pg + fold_grinding_bits)`; the level's union is `k_recursive` of them).

Prover ms / verifier ms / proof KB, pooled allocator throughout (peak GB in
the generated tables). Binius64's own BaseFold rows for reference:

| workload, N | Binius64 r1/2 | Binius64 r1/8 | opener r1/2 union | opener r1/2 rbr | opener r1/8 union | opener r1/8 rbr |
| --- | --- | --- | --- | --- | --- | --- |
| u32 2^15 | 31 / 1.6 / 324 | 36 / 1.5 / 243 | 35 / 3.0 / 362 | 34 / 3.0 / 346 | 47 / 2.5 / 187 | 38 / 2.4 / 180 |
| u32 2^18 | 146 / 3.8 / 432 | 180 / 3.8 / 306 | 159 / 5.3 / 408 | 153 / 5.2 / 392 | 188 / 4.8 / 215 | 176 / 4.7 / 207 |
| u32 2^20 | 536 / 16.6 / 519 | 680 / 17.5 / 355 | 570 / 18.3 / 437 | 556 / 18.1 / 419 | 715 / 17.3 / 230 | 645 / 17.2 / 222 |
| u64 2^15 | 31 / 1.3 / 324 | 34 / 1.1 / 242 | 37 / 2.6 / 362 | 35 / 2.6 / 342 | 40 / 2.1 / 187 | 39 / 2.1 / 179 |
| u64 2^18 | 147 / 2.9 / 431 | 168 / 2.7 / 305 | 157 / 4.4 / 407 | 154 / 4.3 / 389 | 179 / 3.8 / 215 | 177 / 3.8 / 206 |
| u64 2^20 | 535 / 13.6 / 518 | 632 / 13.1 / 355 | 568 / 15.4 / 438 | 559 / 15.4 / 418 | 675 / 14.7 / 229 | 653 / 14.7 / 223 |
| u128 2^15 | 86 / 3.7 / 388 | 97 / 3.5 / 283 | 94 / 5.5 / 387 | 92 / 5.5 / 370 | 111 / 4.8 / 200 | 103 / 4.8 / 194 |
| u128 2^18 | 577 / 21.4 / 519 | 668 / 21.0 / 356 | 622 / 23.1 / 441 | 597 / 22.9 / 418 | 716 / 22.6 / 231 | 693 / 22.4 / 223 |
| u128 2^19 | 1233 / 39.1 / 559 | 1879 / 168 / 383 | 1261 / 39.2 / 463 | 1231 / 39.0 / 440 | 1421 / 38.6 / 248 | 1384 / 38.4 / 238 |
| u128 2^20 (all page) | 18255 / 1218 / 607 | 23574 / 2001 / 408 | 18864 / 1457 / 479 | 17709 / 1201 / 458 | 23772 / 1569 / 256 | 24719 / 862 / 246 |

Reading: the accounting model moves proof size by 4-5 % (round-by-round is
smaller: fewer queries, less grinding) and prover time by 1-9 %; the rate moves
proof size by ~1.9x (rate 1/8 wins) and prover time by 10-20 % (rate 1/2 wins).
With the pooled allocator the opener's prover sits 3-6 % above Binius64's own
at rate 1/2 (u64 2^20: 559 vs 535 ms; u128 2^19: 1231 vs 1233, i.e. parity),
and that residual is the PCS: the rate-1/2 codeword and 32-lane BLAKE3 Merkle
tree in commit, plus Round 0 and two Johnson openings with grinding against one
BaseFold opening. At rate 1/2 the opener's proof is larger than Binius64's own
up to 2^17 and 15-20 % smaller at 2^20; at rate 1/8 it is 22-37 % smaller than
Binius64's own rate-1/8 proof at every size. Peak memory is Binius64's PIOP
either way. u128 2^20 pages in every configuration (10-13 GB of swap with the
pool, Binius64's own rows likewise), so that size group is reported but not
comparable with the rest. The 2^19 u128 cases need ~14 GB resident and were run
only with >=14 GB unused.

## First measurements (2026-09-10, Apple M5 24 GB, 8 threads, smoke runs)

**Historical: every number in this section was taken with the opener at rate
1/8 (before `LOG_INV_RATE` moved to 1 on 2026-09-11); the rate-1/2 rows are
the ones in the regenerated paper tables.**

Single-sample smoke runs to validate the wiring — not the 11/21-sample
campaigns the paper tables use. Binius64 rows at rate 1/8 with its 100-bit
query-phase target (121 queries); `binius64-ligerito` gated at a 100-bit
whole-protocol union bound.

| bench / shape | scheme | prover (ms) | verifier (ms) | proof (B) | notes |
| --- | --- | ---: | ---: | ---: | --- |
| SHA-256 compare, 2^10 compressions (2 samples) | binius64 | 44.4 | 6.17 | 185,456 | commit 8.6, PIOP 33.8, opening 3.3 |
| | binius64-ligerito | 59.6 | 7.51 | 95,064 | commit 7.3, PIOP 32.3, opening 19.8; one oracle; target 104 → 100.4 bits, 62 queries |
| native u32 mul, 2^15 (1 sample) | binius64 | 46.2 | 1.70 | 245,072 | peak 446 MiB |
| | binius64-ligerito | 69.9 | 2.57 | 187,632 | peak 390 MiB; PCS 30.6 ms = two openings (witness 2^17 + logup* pushforward 2^16); target 105 → 100.6 bits, 63 queries, 17 fold-grinding bits |
| hybrid table, 2^15 mul : 2^7 SHA (3 iterations) | all-binius (same session) | 57.1–61.8 | 2.9–3.3 | 245,104 | rate 1/8, 100-bit FRI target; the paper's quiet 11-iteration median is 50.5 / 2.59 |
| | binius-ligerito | 76.5–83.4 | 3.3–3.7 | 188,880 | setup 245 ms; oracles [2^17, 2^16]; target 105 → 100.56 bits (hybrid v3 at this shape: 50.1 / 3.29 / 109,952) |
| u32 PCS compare, 2^15 rows (1 sample) | binius64-basefold | commit 1.21, open 1.05 | — | 128,096 | 121 queries |
| | bitz-ligerito-binary | commit 1.07, open 9.48 | — | 70,040 | 62 queries, 16 query + 14 fold grinding bits, target 104 → 100.8 bits |
| BabyBear PCS compare, 2^15 rows (1 sample) | binius64-basefold | commit 1.11, open 0.95 | — | 128,128 | 121 queries |
| | bitz-ligerito-binary | commit 1.55, open 10.4 | — | 70,136 | 62 queries, target 104 → 100.8 bits |

Larger sizes (same box, 3 samples + warm-up, medians; `PerfRuns/large-*`):

| bench / shape | scheme | prover (ms) | verifier (ms) | proof (B) | peak | notes |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| native u32 mul, 2^18 | binius64 | 200 | 9.1 | 308,400 | 3.01 GB | |
| | binius64-ligerito | 239 (+20 %) | 9.9 | 217,968 (−29 %) | 2.90 GB | |
| native u32 mul, 2^20 | binius64 | 773 | 48.0 | 357,808 | 9.39 GB | |
| | binius64-ligerito | 1009 (+30 %) | 48.0 | 234,128 (−35 %) | 6.86 GB (−27 %) | 20 bits of level-0 fold grinding; 100.3 bits |
| SHA-256 compare, 2^13 compressions | binius64 | 215 | 44.1 | 251,872 | – | commit 59, PIOP 141, opening 15 |
| | binius64-ligerito | 266 (+24 %) | 51.1 | 121,624 (−52 %) | – | commit 53, PIOP 149, opening 58; one oracle; target 104 → 100.1 bits, 62 queries |
| hybrid 2^19:2^11 (same session) | all-binius | 458 | 33.6 | 333,344 | – | first iteration 552 |
| | binius-ligerito | 549 (+20 %) | 31.6 | 225,008 (−32 %) | – | oracles [2^21, 2^16], fold grind 21, OOD grind 13; 100.34 bits |
| hybrid 2^20:2^12 (same session) | all-binius | 1166 | 80 | 357,840 | – | first iteration 2097; the paper's quiet run: 902 / 72 |
| | binius-ligerito | 1383 (+19 %) | 67 | 233,520 (−35 %) | – | first iteration 2045; oracles [2^22, 2^16], fold grind 22, OOD grind 14; 100.34 bits |

The session running the larger shapes was noisier than the paper's campaigns
(both modes' first iterations are 1.2–1.8× their later ones); the ratios
above are same-session medians. As the witness grows the opener's fixed
costs amortize: the prover overhead settles around +20–30 %, the verifier
becomes equal or faster (fewer, cheaper queries; the PIOP verifier dominates),
the proof gap widens to −35 %, and peak memory is 27 % lower at 2^20 (the
rate-1/8 Reed–Solomon codeword plus a Merkle tree, against BaseFold's
folded-oracle ladder).

Reading: the proof-size win of the BitZ opener over BaseFold at rate 1/8 is
−45 % on the identical claim (PCS rows) and −49 % on the SHA circuit (one
oracle), but only −23 % on the multiplication circuits, where Binius64's
IntMul reduction commits a second, 2^16-word logup* oracle that costs a
whole second Ligerito opening (FRI batches extra oracles into one ladder;
Ligerito's recursion cannot absorb a foreign oracle mid-ladder). The
hybrid's 110 KB at the same shape is what a *single* shared opening buys
over this row's two openings.

Where the prover time goes (`LIG_PROVE_TRACE=1`, hybrid 2^15:2^7,
`binius-ligerito`): the two openings cost 13.3 ms (2^17 witness) and 11.5 ms
(2^16 pushforward) — nearly size-independent. The solver's ladders are
`witness: L0 k=5 fold-grind 17 (tapered 17..13) / query-grind 16 / 63 queries,
L1 fold 12, L2 fold 10, L3 fold 8, query-grind 16 at every level` and
`pushforward: L0 fold 16, L1 11, L2 9, query-grind 16 at every level`, i.e.
about 2^19.0 + 2^18.3 ≈ 2^19.7 expected hash evaluations in total, half of
them the 16-bit query grinds (a fixed ~1 ms per level) and most of the rest
the level-0 fold grind. That is ~17 ms of the 24.8 ms, at roughly 50 M
hashes/s — a third of the throughput flock's parallel grind reaches on the
hybrid's 2^23-hash grinds at 2^21, because at 2^15–2^17 hashes per search
the rayon dispatch and `find_first` verification dominate. The actual
folding, encoding and Merkle work is ~7 ms. So at small sizes the Johnson
opener is a ~10 ms fixed cost per oracle, four-fifths proof of work; it
amortizes with size (the hybrid's shared opening is 14 % of the prover at
2^20). Two levers: batch the pushforward into the witness opening with the
hybrid's two-root virtual oracle (one ladder, one query set; both claims are
already packed-level linear relations), and a batched small-grind in flock.
