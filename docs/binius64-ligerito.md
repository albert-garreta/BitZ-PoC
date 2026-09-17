# Binius64 with the F2Z opener (`binius64-ligerito`)

A comparison scheme that answers "what if Binius64 used F2Z's binary-field
PCS?": Binius64's own circuits and PIOP, unchanged, with every oracle
committed and opened by the opener F2Z itself uses — default rate 1/2, the Johnson
(list-decoding) proximity regime, fold and query grinding, and Round 0 (the
out-of-domain sample). It is available in the supported benchmarks listed below. The composed SHA+P-256 ECDSA circuit is currently unsupported because it contains BMUL constraints.

| bench | scheme id | what it measures |
| --- | --- | --- |
| `benches/mul_e2e_compare` (u32, BabyBear, u64, u128) | `binius64-ligerito` | Binius64's native multiplication circuits, end to end |
| `benches/sha256_e2e_compare` | `binius64-ligerito` | Binius64's two-lane SHA-256 circuit, end to end |
| `benches/hybrid_u32_sha256` | mode `binius-ligerito` | the all-Binius circuit (four-limb mod-2^32 gadget + chained SHA) |
| `mul_compare pcs --workload u32-full,baby-bear` | `f2z-ligerito-binary` | PCS only: the Binius64 packed rows and the identical bit-MLE claim, opened by the F2Z opener instead of BaseFold |

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
   `F2Z_BINIUS_LOG_INV_RATE` select another; the rate is part of the
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
   (domain-separated by oracle index): the witness claim through F2Z's ring
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
(`F2Z_BINIUS_LOG_INV_RATE`, default 1 = rate 1/2; 3 = rate 1/8, through
`Prepared::with_rate`). The 2026-09-13 suite runs the opener rows at both
rates under the round-by-round accounting
(`F2Z_BINIUS_LIGERITO_ACCOUNTING=rbr`) — the model the paper's tables
render; the union bound stays available and is recorded alongside.

`BinaryPcs::with_log_inv_rate` and `Prepared::with_log_inv_rate` also accept
rates 1/2, 1/4, and 1/8. Native multiplication rows can override the campaign
rate with `F2Z_BINIUS_LIGERITO_LOG_INV_RATE=1|2|3`.

As everywhere in this repository the bound is algebraic/IOP-level under
BLAKE3 Fiat–Shamir and 256-bit Merkle hashing; it is not an unconditional
Fiat–Shamir theorem.

## Running

```sh
cargo bench --bench mul_compare --features native-mul-compare,bench-internals -- \
  proof --workload u32-mod32 --backends binius64,binius64-ligerito --log-n 15 --threads 8
cargo bench --bench mul_compare --features native-mul-compare,bench-internals -- \
  pcs --workload u32-full,baby-bear --backends binius64-basefold,f2z-ligerito-binary \
  --log-n 15 --threads 8 --out results/binary-pcs
python3 scripts/mul_report.py results/binary-pcs --out reports/binary-pcs
```

Multiplication configuration uses flags; see the [benchmark guide](native-mul-compare.md).
