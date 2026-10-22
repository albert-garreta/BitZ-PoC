# Towards a fixed recursion circuit: what the Falcon verifier computes from challenges

2026-10-22. Today the recursion circuit is a public-challenge circuit
(`src/recursion/val.rs`): every Fiat–Shamir challenge and query position of
the base proof is a constant, so a challenge times a GF value is an
F2-linear map and a Merkle child order is wiring, and everything that
depends only on public values runs natively through the protocol's hooks.
The circuit is re-recorded per proof, and the final verifier replays it.
Plonky2, SP1, RISC Zero and Stwo instead use one circuit per shape with the
challenges as witness. This note lists what has to move into such a fixed
circuit, and how to handle the exponentiations.

## Native work on challenges (configuration G, t = 16 bridge rows)

Sizes at batch 32,768 (s = 13 bridge column variables; 1,024: s = 8). The
prime q is 112 bits ([2^111, 2^112 − 2^100] at t = 16). mq = mod-q product,
gm = GF(2^128) product, gexp = GF(2^128) exponentiation.

| item | math | size | kind |
|---|---|---|---|
| row weights (`arith::bridge_claim` + `widen_bridge_claim`) | w_(j,ℓ) = eq_q(ρ_B[0..12], j)·d_ℓ mod q, j < 2^12, ℓ < 16; d_ℓ = 2^ℓ + ε (ℓ < 14), d_14 = −2^14 + ε, d_15 = ζ_m; ε = ζ_m² if B < cap, else 0 | 2^16 mq at every batch | mq |
| row images (`integer_bridge::recursion::binary_claim`) | low[r] = eq(ρ_row, r)·(g^(w_r) + 1) | **2^16 gexp** (≤ 112-bit exponents) + 2^17 gm | gexp |
| joint terminal coefficient (`joint_sumcheck` `terminal_coefficient`) | Σ_branch padding·(⟨low, eq(pt_lo)⟩·eq(high, pt_hi) + gathers) | ≈ 98K gm | gm |
| polylog read-off (`polylog::readoff_weights` + its bridge) | rows (k < 128, c_lo < 2^a), weights 2^k·eq_q(point[..a], c_lo) mod q, a = min(t − 7, s − 1) | 2^16 mq + **2^16 gexp** (a = 9) | gexp |
| standard root claim (`root_claim`) | Σ_c eq(ζ, c)·g^(S_c), S_c the public folds | 2^13 gexp, 128-bit exponents; gone with the polylog bridge (fold tree) | gexp |
| read-off, bind | Σ_c S_c·eq_q(point, c); folds absorbed | 2^13 mq; ≈ 3.4K SHA-256 | mq |
| statement (`bind_statement`, `keyed_claim`, `binder_terminal_weights`) | BLAKE3 over keys ‖ targets; every target t_i and key h_i evaluated at the two keyed points | 64 MiB BLAKE3; ≈ 34M + 34M small×F_q products | statement |
| instance polynomials, eq tables over instances, exceptions | BLAKE3 XOF; eq tables of size cap; 2^s + 3E gm | ≈ 12K BLAKE3; ≈ 4·2^15 mq | ch + data |
| per-round work (forests, sumchecks, Round 0, ring switch, residuals) | inversions, eq factors, powers | O(rounds); ≈ 0.3M gm in the opening | gm, mq |

Shape, besides arithmetic: the prime is sampled by rejection, with ≈ 97
Miller–Rabin modexps; mod-q challenges and distinct query positions are
also drawn by rejection; and the Merkle multi-proof topology depends on the
query positions.

## Cost model of today's circuit

The recursion relation has bits, AND gates and linear integer rows, but no
product constraint. In-circuit:

- `mul_mod_q`: a 112 × 112 schoolbook, 12,544 ANDs (≈ 0.6 SHA compressions);
- a GF product: a Karatsuba `clmul`, 2,187 ANDs;
- `pow_public_base`: a 112-bit exponent of a public base, 112 × 128 = 14.3K
  ANDs.

So the 2^16 row images cost ≈ 0.94G ANDs, about 6× the whole 32K circuit
(158M cells), and the 2^16 row weights, one product each, another 0.82G. With a native
product family (integer products by an F_q sumcheck, as the Falcon norm
does for squares; GF products by a binary zerocheck), a product would cost
its operands' bits, ≈ 400 committed bits (estimate). Most O(2^16) tables
need no products at all: a sumcheck takes them down to closed-form eq
evaluations. The row images are the exception: they have no closed form,
and 2^16 × 112 conditional products are too many even at 400 bits each.

## Exponentiations: product tree over committed weight bits

This is the polylog bridge's fold-tree trick, applied to the row weights:

1. After ρ_B, ζ_m and q, the base prover commits the weights' bits
   W (2^16 × 112).
2. A product forest with public images g^(2^k) (rows k < 128, columns
   r < 2^16, rooted at the point τ where the verifier needs
   I(τ) = Σ_r eq(τ, r)·g^(W_r)) replaces the 2^16 exponentiations: ≈ 140
   GKR rounds for the verifier. Its exit is a binary claim on W, batched
   into the joint sumcheck. The binary claim's eq(ρ_row, ·)·eq(σ, ·)
   weights need no extra sumcheck. Coordinate-wise,
   eq(ρ_i, ·)·eq(σ_i, ·) = d_i·eq(ρ''_i, ·), with d_i = 1 + σ_i + ρ_i and
   ρ''_i = σ_i ρ_i / d_i (`merge_points`), so τ = ρ''. This tree is the
   `(⋆)` tree of branch bitz-logarithmic (2026-10-22, `StarMode::Gkr`).
   That branch leaves W uncommitted: its verifier evaluates the exit claim
   from every row, and step 3 is what replaces that. In Falcon's split
   joint geometry the images reach the terminal through three branches:
   lanes 0–7; lanes 8–10 and 11 + … + 14; lane 15's slack gathers. The tree
   therefore starts from one combined root claim.
3. W's correctness without a second integer claim. Checking W by its MLE
   at a fresh point would need a bridge on W, with its own challenge-
   dependent row images: a regress. Instead:
   - lanes 1–14 form a doubling chain, W_(j,ℓ+1) = 2·W_(j,ℓ) − c·q with
     c ∈ {0, 1}, which is a binary relation (shift, conditional
     subtraction, borrow chain; ≈ 340 committed bits per row, plus a shift
     argument); an accepted W is ≡ the weight mod q and below 2^112, which
     is all the fold bound needs;
   - lane 0 (E_j = eq_q(ρ_B, j), 4,096 entries) and lane 15 (ζ_m·E_j) are
     mod-q products the verifier computes itself (≈ 8K + 4K mq; another 4K
     when ε ≠ 0), linked to W by one binary claim.
4. The read-off bridge's images (weights 2^k·eq_q(point[..a], c_lo)) get
   the same treatment: 2^a base values and a 128-step doubling chain each.

Cost: +15–22M committed bits per bridge in the base proof (+3–4 % of the
32K base commitment, but about +100 % at 1K, where fewer bridge rows would
be better); ≈ 300 GKR and zerocheck rounds and ≈ 12–16K mod-q products in
the verifier. The products need the native product family; as schoolbooks
they would cost ≈ 0.2G ANDs.

## One recursion level does not need any of this in-circuit

A fixed circuit can export the challenge points the tables depend on:
ρ_B, ζ_m, q, the forest and joint exit points, the instance, norm and
binder points, the keyed points and the instance seed. That is ≈ 150
field elements, ≈ 3 KB. The final verifier then computes the images and
tables natively, as it already does for the statement terms (which stay
native in every design), and passes the results in as public inputs. The
circuit is fixed, and its public stream shrinks from 24 KB (polylog bridge)
to about 3 KB. The final verifier is left with the statement terms and the
outer verification, with no replay. In-circuit exponentiation is needed
once the outer verifier must itself be a circuit (multi-level recursion).

## Staged plan

1. A product constraint family in the recursion relation: mod-q products
   by an F_q sumcheck, GF products by a binary zerocheck.
2. The fixed circuit: challenges as witness, fixed-attempt sampling (or
   exported prime and positions), per-query Merkle paths; exponentiations,
   tables and statement terms exported.
3. For multi-level recursion, the exponentiation sub-protocol in the base
   proof: W, the product tree, the doubling chain.
