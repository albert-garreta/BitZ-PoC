# The b127 field study — `GF(2^127)` vs the GHASH `GF(2^128)`

**Status: field module landed (`src/poly/univariate/binary_b127.rs`), protocol
unchanged — and the study's verdict is that it should stay unchanged.** BitZ's
exponent fold, forest, pre-sumcheck, ring-switch and flock opener all run over
the GHASH field. This note records (1) why a protocol-level swap to
`GF(2^127)` is architecturally blocked in the Ligerito pipeline, (2) the
measured head-to-head against BitZ's own `GF(2^128)` pipeline on this repo's
hot patterns — **b127 is equal-to-12 % slower on Apple M4**, the "~30 %
faster" folklore being a statement about GHASH implementations with off-PMULL
reductions — and (3) what would carry over if a b127 variant were ever built
on a compatible opener and PMULL-starved hardware.

Provenance: the b127 representation (`u128 < 2^127`, `f(X) = X^127 + X + 1`,
carryless product + trinomial fold) follows Reilabs'
[`ghash-powers-bench`](https://github.com/reilabs/ghash-powers-bench)
(`b127`), re-implemented in this repo's NEON-resident idiom and optimized
well past the reference (§4.2) — the verdict below is best-vs-best.

## 1. Why b127 is attractive on paper

* `f(X) = X^127 + X + 1` is an irreducible **trinomial**: reduction of a
  carryless product `P = L + X^127·H` is `L ⊕ H ⊕ (H << 1)` — one fold,
  **zero PMULLs** (canonical operands have `deg ≤ 126`, so
  `deg((X+1)H) ≤ 126` and the fold is exact). GHASH reduction is a 3-PMULL
  fold. A reduced NEON multiply drops from 7 PMULL to 4; a squaring from 5
  PMULL to 2.
* `|B^×| = 2^127 − 1` is the **Mersenne prime `M_127`** — every `α ∉ {0, 1}`
  generates, so the exponent binding's generator check collapses from the
  9-factor primitive-element test (≈49 % acceptance, resampling loop) to
  `α ∉ {0, 1}` ([`is_generator_b127`]), and `n ↦ α^n` is injective on
  `[0, 2^127 − 1)`.
* 127 prime ⇒ the only proper subfield is `F_2` — a clean, tower-free field.

The trade behind the first bullet — **fewer PMULLs, more shift/logical µops**
— is exactly what the measurement adjudicates (§4), and on Apple silicon it
loses: PMULL throughput there is abundant enough that GHASH's all-PMULL
pipeline is the better mix. The third bullet is also what blocks the swap.

## 2. The architectural boundary: the swap is blocked at the ring-switch

The soundness chain is **one unbroken field pipeline** (see `DESIGN.md`):

```
forest GKR (K) → exit claim (K-point) → pre-sumcheck (K)
  → bit-MLE claim M̂(r*, ξ) = μ, point ∈ K^n
  → ring-switch (128 = 2^7 partial evals s_v; K ⊗_{F₂} K tensor algebra)
  → flock recursive Ligerito (K)
```

Every arrow reuses the previous stage's challenge point, so the field cannot
change mid-chain, and a claim bound over one field cannot be discharged by an
opening over another (an `F_2`-MLE claim at a `B`-point says nothing about the
same data's evaluation at a `K`-point). Three independent facts then pin
`K = GF(2^128)`:

1. **Ring-switch packing needs `[K : F_2] = 2^κ`.** The bit-matrix is packed
   128 bits per field element along `LOG_PACKING = 7` boolean coordinates
   (`src/ligerito.rs`), and the recombination (`transpose_bits_128`, the
   `K ⊗_{F₂} K` column algebra in `tensor_eq_phi_eval`/`residual_b_evals`)
   uses that the packing count (2^7) *equals* the field dimension (128).
   `[B : F_2] = 127` is not a power of two: a 127-slot packing does not admit
   the boolean-coordinate split at all. (A 64 = 2^6-slot packing into b127
   is mathematically salvageable with a re-derived Φ map, at a 2× committed-
   data blowup — a research-grade change, not a swap.)
2. **flock-core is GHASH-hardwired**: `ntt/additive_ntt_f128`,
   `field/gf2_128`, the BaseFold/Ligerito folds, the `GF(2^8)`/φ₈
   acceleration. And because 127 is prime, `GF(2^127)` has **no subfield
   towers whatsoever** — the `GF(2^8)`-style machinery has no analogue, so a
   "flock over b127" is a fork with a redesign, not a port.
3. The forest cannot run in a different field from the opener (the chain
   above), so "b127 for the exponent fold only" is unsound — there is no
   field bridge.

The one opener design that *is* b127-compatible is the flat `F_2`-code
(RAA/Brakedown-style) opener that the Ligerito-only conversion removed:
an `F_2`-linear code folds with scalars from **any** extension field. A b127
protocol variant would mean resurrecting that opener (from the zinc-plus
parent) and accepting its ~√N proof sizes back.

## 3. What carries over for free (recorded for any future variant)

* **Chunk geometry unchanged**: `c_w = 127 − t − W` already keeps every
  per-chunk fold `< 2^127`. The honest maximum is
  `2^t (2^{c_w}−1)(2^W−1) = 2^127 − 2^{c_w+t} − 2^{W+t} + 2^t ≤ 2^127 − 3`,
  strictly below `ord α = 2^127 − 1`.
* **Range check tightens by one value**: the verifier's per-chunk bound must
  become `u < 2^127 − 1` (rejecting exactly `u = 2^127 − 1 = ord α`, which
  would collide `α^u = α^0`). Completeness is unaffected (slack ≥ 2).
* **Generator check trivializes** to `α ∉ {0, 1}` (prime group order).
* Sumcheck/SZ-type soundness denominators go `2^128 − 1 → 2^127 − 1`
  (one bit).
* **Transcript sampling**: a raw 128-bit draw folds into the field by
  `X^127 ≡ X + 1` (`PrimeField::new_with_cfg`) — an exactly-2-to-1 map, so
  uniform bytes stay uniform. **Codec canonicality**: deserialization must
  *reject* a set bit 127 (`try_from_words`), never fold it — folding would
  make two byte streams decode to one element (malleability).

## 4. Measurements (Apple M4, `-C target-cpu=native`, LTO, `unchecked`)

### 4.1 The harness lesson first

The first version of `benches/field.rs` timed all GF128 patterns, then all
b127 patterns, in one process — and reported a phantom "b127 1.30× on batch
multiply". That was **clock-ramp shading**: the GF128 half always occupied
the first seconds of the process (measuring 1.16–1.27 ns/mul on a pattern
whose steady-state is ~0.82), while b127's half ran post-ramp. The landed
harness alternates the two fields rep-by-rep *within* each pattern, so both
sample every thermal/clock window; runs became reproducible to ±1–3 % and
the ratios flipped. (This is the zinc measurement lore — "interleave runs in
one thermal window before claiming a regression" — now enforced by the
harness structure itself. Cross-run comparisons of a single field remain
ramp-shaded; only in-window ratios are quotable.)

### 4.2 In-repo head-to-head — `cargo bench --bench field`

Both fields run this repo's NEON-resident pipelines: schoolbook 4-PMULL
products; GHASH reduces with its verbatim-upstream 3-PMULL fold; b127 with
the SHA3 BCAX trinomial fold and a square-specialized fold (below).
Consolidated over three 9-rep interleaved runs:

| pattern | proxy for | GF128 ns/op | b127 ns/op | b127 speedup |
|---|---|---|---|---|
| `mul/batch` | forest layer products (throughput) | 0.81–0.92 | 0.88–0.96 | **0.92–0.95×** |
| `mul/chain` | dependent product chains (latency) | 4.33–4.38 | 4.76–4.81 | **0.91×** |
| `square/chain` | α-power place-value chains | 2.49–2.50 | 2.45 | **1.02×** |
| `powers/comb-w8-100b` | `chunk_pow2_table` / root recompute | 24.0–25.1 | 27.2–28.4 | **0.88×** |
| `wide-dot` | delayed-reduction inner products | 0.51–0.52 | 0.51–0.52 | **1.00×** |
| `eqf-round` | the fused sumcheck round kernel | 0.90–0.92 | 0.93–0.95 | **0.96×** |
| `eqf-fold` | the multilinear bind cascade | 1.32–1.34 | 1.43–1.45 | **0.92×** |
| `inverse` | Itoh–Tsujii ladder (see below) | 302 | 300 | **1.01×** |

(The `inverse` row is the 2026-07-19 generation: BOTH fields' inverses
were upgraded from the naive Fermat all-ones ladder — 125/126 chained
multiplications — to the Itoh–Tsujii addition chain
`1→2→3→6→12→24→48→96→120→126(→127)` with register-resident squaring
runs (`square_n`): 9 mults for b127, 10 for GHASH, ~3× faster inverses
(~300 ns vs ~870–915 estimated from the measured chain latencies) —
best-vs-best preserved. The tie is structural: the ladder is a serial
squaring CHAIN, i.e. latency-bound, and both fields' square latencies
are ~2.5 ns; b127's 2-vs-5-PMULL square advantage is a throughput
property, which a dependency chain cannot spend. The known next rung —
precomputed `x ↦ x^{2^k}` F₂-linear nibble-table maps for the long runs
— would cut either inverse to ~100 ns at ~8 KB of tables per fixed `k`,
recorded here in case inverses ever reach a hot path; they are not on
one.)

The b127 pipeline behind those numbers is already the *optimized* endpoint
of a ladder, each step validated in-window:

* naive port of the reference (scalar-composed reduce): the starting point —
  already 1.4–1.5× faster than Reilabs' own b127 binary once NEON-resident;
* SRI reduction (`H` and `H<<1` lifted straight off the product limbs by
  shift-right-insert): +8–15 % across patterns;
* SHA3 **BCAX** fold (both corrective masks fused into `a ⊕ (b & ~c)`:
  7 µops, ~8-cycle chain, zero PMULLs): batch −6 %, kept;
* square-specialized fold (`a² ≡ S(a_0) ⊕ (S(a_1)≪1) ⊕ (S(a_1)≪2)` via
  `X^128 ≡ X² + X`; the canonical invariant zeroes `a_1`'s bit 63 so no
  second fold; one EOR3): squares 0.90× → 1.02× — the only pattern b127
  ends up winning;
* 3-PMULL Karatsuba product: **rejected**, 0.74× vs schoolbook (in-window);
* GHASH-side EOR3 in the 0x87 fold: **tried and rejected** — it slowed
  GHASH itself (+13 % batch, +9 % chain, +18 % squares; the SHA3-unit op
  costs latency/ports where a plain EOR tree runs on any SIMD pipe), so the
  baseline stays verbatim-upstream;
* GHASH-shaped PMULL fold (`mul_pfold`, 2026-07-19 revisit): reduce via
  `X^128 ≡ X² + X = 0x6` with instruction-for-instruction GHASH's 3-PMULL
  `0x87` fold (lane-aligned, no cross-lane bit-127 extraction), then the
  bit-127 canonicalization (`X^127 ≡ X + 1`) that a degree-128 modulus
  never owes: **rejected**, 0.71× vs GF128 in-window (1.17 vs 0.83 ns
  batch) — 7 PMULLs *plus* a serial canonicalization tail loses to both
  GHASH and the BCAX shift fold. The reduction cannot be bought back with
  PMULLs; the canonicalization is the irreducible tax of the
  lane-misaligned degree.

Both escape directions are thereby measured shut: spending *fewer* PMULLs
on the product (Karatsuba, 0.74×) and spending the reduction *on* the
PMULL ports like GHASH (pfold, 0.71×) each lose to the balanced BCAX
endpoint, which itself sits at 0.88–0.96× on the prover-dominant patterns.
And the ceiling is analytic, not just empirical: the one b127 multiply
that ties GHASH exactly is the `0x6` fold on a **relaxed** (bit-127-
allowed, non-canonical) representation — byte-identical to GHASH's
instruction sequence with only the fold constant swapped, hence exactly
1.00× by construction — at the price of forfeiting canonical equality,
hashing and serialization (two representations per element; every Eq /
transcript-absorb / codec boundary must canonicalize). Parity is the hard
ceiling for b127 multiplication on this core; "+20–30 %" is unreachable
against this baseline.

### 4.3 Cross-validation — Reilabs' own bench on the same box

`ghash-powers-bench` (their binary, `--mul` and powers/win-15):

| | ghash128 | b127 | b127 speedup |
|---|---|---|---|
| mul, ns/mul (2^20–2^22) | 1.805–1.844 | 1.477–1.514 | **1.22×** |
| powers win-15, ns/elem (2^16–2^18) | 39.7–40.5 | 31.6–32.9 | **1.22×** |

So the ~"30 % faster" claim **replicates against Reilabs' GHASH baseline**
— which reduces GHASH in scalar `u128` ops. BitZ's pipelines are a different
regime entirely: its GHASH multiplies ~2.1× faster than Reilabs' GHASH, and
this b127 port ~1.6× faster than Reilabs' b127, on the same box. Once both
reductions are engineered to their best, the ordering inverts: **fewer
PMULLs + more shift/logical µops loses to all-PMULL on Apple silicon**,
whose PMULL throughput (≥2/cycle) makes the GHASH fold nearly free — a
GHASH multiply retires in ~3.7 cycles vs b127's ~4.0. The b127 trade is the
right one precisely on cores where carryless multiply is port-constrained
(one CLMUL/crypto pipe — older x86, small ARM cores, and any scalar
target); it is the wrong one here.

### 4.4 Net prover impact if the swap were free

Weighting the prover's phases by their kernels — forest layer products
(`mul/batch`, 0.92–0.95×), the GKR round kernels / pre-sumcheck / bind
cascades (`eqf`, 0.92–0.96×), wide-dot accumulations (1.00× — delayed
reduction already amortizes the reduction, neutralizing b127's one
structural advantage exactly where it would matter most), α-power tables
(0.88×) — a b127-fielded BitZ prover on this hardware would run
**~5–10 % slower**, before recalling that the swap is architecturally
blocked (§2) and that the b127-compatible opener trades away Ligerito's
proof sizes.

## 5. Conclusions

1. The b127 field module is landed, tested (NEON↔scalar byte-parity pins,
   bit-reference multiply, algebra laws, the `a^{2^127} = a` order
   certificate, kernel value-exactness, canonicality rejection) and
   optimized well past the reference implementation (SRI/BCAX fold,
   specialized square). It is the honest best-known b127 on this hardware.
2. **The "b127 ≈ 30 % faster than GHASH" claim is a statement about GHASH
   implementations whose reduction runs off the PMULL ports.** Against
   BitZ's PMULL-fold GHASH on Apple M4, b127 is 0.88–1.02× — equal at best
   (squares, wide-dot), ~10 % behind on the patterns that dominate the
   prover. A swap would cost ~5–10 % prover time here; it would pay only on
   CLMUL-port-constrained hardware. (Re-confirmed 2026-07-19 on a fresh
   build: 0.88–1.04× across the table, and the remaining untried
   direction — a PMULL-ported b127 reduction, `mul_pfold` — measured
   0.71×, closing the "optimize it differently" escape; see the ceiling
   argument at the end of §4.2.)
3. The swap is in any case blocked by the ring-switch packing
   (`[K:F_2] = 2^7`) and flock's GHASH-native Ligerito; the prime-order
   generator-check simplification and the unchanged chunk geometry are real
   but only reachable via a non-Ligerito opener or a re-derived 64-packing
   ring-switch over a forked flock.
4. Methodological: field micro-benches must interleave the compared fields
   rep-by-rep within each pattern — the sequential harness manufactured a
   spurious 1.30× from clock-ramp shading (§4.1).
