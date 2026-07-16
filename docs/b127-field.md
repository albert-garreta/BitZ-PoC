# The b127 field study — `GF(2^127)` vs the GHASH `GF(2^128)`

**Status: field module landed (`src/poly/univariate/binary_b127.rs`), protocol
unchanged.** F2Z's exponent fold, forest, pre-sumcheck, ring-switch and flock
opener all still run over the GHASH field. This note records (1) why a
protocol-level swap to `GF(2^127)` is architecturally blocked in the Ligerito
pipeline, (2) what the b127 field measures head-to-head against F2Z's own
`GF(2^128)` pipeline on this repo's hot patterns, and (3) what *would* carry
over if a b127 protocol variant were ever built on a compatible opener.

Provenance: the b127 representation (`u128 < 2^127`, `f(X) = X^127 + X + 1`,
Karatsuba product + two-XOR trinomial fold) follows Reilabs'
[`ghash-powers-bench`](https://github.com/reilabs/ghash-powers-bench),
re-implemented in this repo's NEON-resident idiom and further optimized
(SRI-based reduction; see below).

## 1. Why b127 is attractive on paper

* `f(X) = X^127 + X + 1` is an irreducible **trinomial**: reduction of a
  carryless product `P = L + X^127·H` is `L ⊕ H ⊕ (H << 1)` — one fold, two
  shifted XORs, **zero PMULLs** (canonical operands have `deg ≤ 126`, so
  `deg((X+1)H) ≤ 126` and the fold is exact). GHASH reduction is a 3-PMULL
  fold (or a ~14-op shift cascade). A reduced NEON multiply drops from
  7 PMULL to 4; a squaring from 5 PMULL to 2.
* `|B^×| = 2^127 − 1` is the **Mersenne prime `M_127`** — every `α ∉ {0, 1}`
  generates, so the exponent binding's generator check collapses from the
  9-factor primitive-element test (≈49 % acceptance, resampling loop) to
  `α ∉ {0, 1}` (`is_generator_b127`), and `n ↦ α^n` is injective on
  `[0, 2^127 − 1)`.
* 127 prime ⇒ the only proper subfield is `F_2` — a clean, tower-free field.

That last bullet is also exactly what kills the drop-in swap.

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

### 4.1 In-repo head-to-head — `cargo bench --bench field`

Both fields run this repo's own NEON-resident pipelines (schoolbook 4-PMULL
product; GHASH reduces with the 3-PMULL fold, b127 with the SRI trinomial
fold). Medians of 5; ±5 %.

| pattern | proxy for | GF128 ns/op | b127 ns/op | b127 speedup |
|---|---|---|---|---|
| `mul/batch` | forest layer products (throughput) | 1.262 | **0.970** | **1.30×** |
| `mul/chain` | dependent product chains (latency) | 4.731 | 4.736 | 1.00× |
| `square/chain` | α-power place-value chains | 2.512 | 2.781 | 0.90× |
| `powers/comb-w8-100b` | `chunk_pow2_table` / root recompute | 25.48 | 29.01 | 0.88× |
| `wide-dot` | delayed-reduction inner products | 0.525 | 0.528 | 0.99× |
| `eqf-round` | the fused sumcheck round kernel | 0.930 | 0.982 | 0.95× |
| `eqf-fold` | the multilinear bind cascade | 1.323 | 1.505 | 0.88× |

Product-variant check: the 3-PMULL Karatsuba product is **worse** than
schoolbook for b127 on this core (batch 1.265 vs 0.970 ns — logical-op
pressure, not PMULL count, is the binding constraint), mirroring the GF128
pipeline's earlier schoolbook-over-Karatsuba finding. The first-cut b127
reduction (12 logical ops, ~14-cycle chain) measured 1.19× / 0.78× / 0.80×
on batch / squares / powers; rewriting it with `SRI` (shift-right-insert:
`H` and `H<<1` come straight off the product limbs as `(hi<<1)|(pre>>63)`
and `(hi<<2)|(pre>>62)`, 9 µops, ~10-cycle chain) moved those to the table
above — that version is the landed default.

### 4.2 Cross-validation — Reilabs' own bench on the same box

`ghash-powers-bench` (their binary, `--mul` and powers/win-15):

| | ghash128 | b127 | b127 speedup |
|---|---|---|---|
| mul, ns/mul (2^20–2^22) | 1.805–1.844 | 1.477–1.514 | **1.22×** |
| powers win-15, ns/elem (2^16–2^18) | 39.7–40.5 | 31.6–32.9 | **1.22×** |

So the ~"30 % faster" claim **replicates against Reilabs' GHASH baseline**.
The catch: that baseline reduces GHASH in scalar `u128` ops. F2Z's GHASH
already reduces on the PMULL ports — it is **1.45× faster** than Reilabs'
GHASH on the same pattern (1.26 vs 1.83 ns/mul) — and against *it*, b127's
edge survives only where reduction µop count shows up as throughput
(`mul/batch`, 1.30×) and inverts wherever the reduction's ~10-cycle
shift-insert chain sits on a latency path against Apple's cheap PMULL fold
(squares, powers, the eqf kernels). For the record, this port is also
1.53× faster than Reilabs' own b127 on the same box (0.97 vs 1.48 ns/mul) —
the NEON-resident restructuring, not the field, accounts for that.

### 4.3 Net prover impact if the swap were free

Weighting the prover's phases by their kernels: the forest's pairwise layer
products are `mul/batch`-shaped (**+30 %**), but the GKR round kernels, the
pre-sumcheck and the bind cascades are `eqf`-shaped (**−5 to −12 %**), the
wide-dot accumulations are a wash (delayed reduction already amortizes the
reduction — the existing optimization neutralizes b127's advantage exactly
where it would matter most), and the α-power tables lose ~12 %. Net
estimate: **~1.0–1.1× end-to-end prover** — even before recalling that the
swap is architecturally blocked (§2) and that the b127-compatible opener
trades away Ligerito's proof sizes.

## 5. Conclusions

1. The b127 field module is landed, tested (NEON↔scalar byte-parity pins,
   bit-reference multiply, algebra laws, order certificate, kernel
   value-exactness) and optimized past the reference implementation.
2. On Apple silicon, **the "b127 ≈ 30 % faster than GHASH" claim is a
   statement about GHASH implementations with off-PMULL reduction**. Against
   a PMULL-fold GHASH, b127 wins ~30 % only on throughput-bound independent
   multiplies and loses latency-bound patterns; a wholesale field swap would
   move the F2Z prover by ~0–10 %, not 30 %.
3. The swap is in any case blocked by the ring-switch packing
   (`[K:F_2] = 2^7`) and flock's GHASH-native Ligerito; the prime-order
   generator-check simplification and the unchanged chunk geometry are real
   but only reachable via a non-Ligerito opener or a re-derived 64-packing
   ring-switch over a forked flock.
