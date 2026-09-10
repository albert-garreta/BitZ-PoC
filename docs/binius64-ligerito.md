# Binius64 with the F2Z opener (`binius64-ligerito`)

A comparison scheme that answers "what if Binius64 used F2Z's binary-field
PCS?": Binius64's own circuits and PIOP, unchanged, with every oracle
committed and opened by the opener F2Z itself uses — rate 1/8, the Johnson
(list-decoding) proximity regime, fold and query grinding, and Round 0 (the
out-of-domain sample). It sits in every benchmark that measures Binius64.

| bench | scheme id | what it measures |
| --- | --- | --- |
| `benches/mul_e2e_compare` (u32, BabyBear, u64, u128) | `binius64-ligerito` | Binius64's native multiplication circuits, end to end |
| `benches/sha256_e2e_compare` | `binius64-ligerito` | Binius64's two-lane SHA-256 circuit, end to end |
| `benches/hybrid_u32_sha256` | mode `binius-ligerito` | the all-Binius circuit (four-limb mod-2^32 gadget + chained SHA) |
| `benches/u32_pcs_compare`, `benches/baby_bear_pcs_compare` | `f2z-ligerito-binary` | PCS only: the Binius64 packed rows and the identical bit-MLE claim, opened by the F2Z opener instead of BaseFold |

Code: `src/binary_pcs.rs` (the opener as a stand-alone binary PCS) and
`src/binius_ligerito/` (the Binius64 PIOP adapter; feature `binius64-bench`).

## Protocol

1. **Statement.** The transcript is seeded with a digest of the constraint
   system's shape, the opener configuration of every oracle, and the public
   words.
2. **PIOP prefix.** `IOPProver::prove_to_evaluation` — the fork's exposure of
   Binius64's prover up to the witness evaluation claim its ring switch would
   consume: the IntMul, BinMul (rejected here), BitAnd, zero and shift
   reductions, exactly as upstream, on a BLAKE3 transcript
   (`src/binius_ligerito/channel.rs`).
3. **Oracles.** Every `send_oracle` the PIOP makes — the packed witness, and
   the IntMul reduction's logup* pushforward (2^16 words) when the circuit
   multiplies — is committed by `BinaryPcs`: an interleaved Reed–Solomon
   codeword at rate 1/8, 32 lanes (512-byte leaves), BLAKE3 Merkle tree. The
   root is bound into the transcript and **Round 0 is taken immediately**:
   the prover grinds, the verifier draws `ζ`, the prover sends
   `y = MLE[oracle](ζ, ζ², ζ⁴, …)`. This pins the committed word to one
   element of its Johnson list before the next challenge, which is what the
   paper's theorem requires of a Johnson-regime opener.
4. **Relations.** Oracle linear relations the PIOP queues
   (`prove_oracle_relations`) are recorded, not opened — the same deferral
   Binius64's BaseFold channel performs at `finish()`. The witness evaluation
   claim is a bit-MLE claim; the pushforward relation is
   `⟨transparent, oracle⟩ = claim` with a transparent basis the verifier can
   evaluate anywhere (the closure Binius64 hands its verifier channel).
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

`Prepared::security()` is a whole-protocol union bound, gated at 100 bits, in
the style of `hybrid::security::account`:

- Binius64 PIOP: the AND/zero/shift overcount `4096 · (log witness words +
  log AND + log zero + 64) / 2^128`, and, when the circuit multiplies, an
  IntMul overcount `4096 · (log IMUL + 64 + 16 + 8) / 2^128` over the 64-layer
  GKR step, the Frobenius/product sumchecks, the limb product check and the
  logup* lookup over the 2^16-row generator table;
- per oracle: Round 0 (`C(L_δ,2)·(2^{m_p}−1)/|K|` at level 0's Johnson
  parameters, topped up to 108 bits by proof of work), every Ligerito level's
  proximity folds (with fold grinding), queries (with query grinding) and the
  deeper levels' out-of-domain samples, the field rounds and the `η_ood` draw;
- the ring switch (128/|K|).

The opener's round-by-round target is the **smallest** in 100..=112 whose
union clears 100 bits (the hybrid hard-codes 106; here 104 at 2^10 SHA
compressions, for example), and the achieved bits are reported. This is the
yardstick of the `f2z` rows; Binius64's own "100 bits" is its FRI query-phase
target only (`calculate_n_test_queries`), which counts neither its folding
phase nor its PIOP. The rate is fixed at 1/8 for this scheme; the Binius64
rate knobs do not apply.

As everywhere in this repository the bound is algebraic/IOP-level under
BLAKE3 Fiat–Shamir and 256-bit Merkle hashing; it is not an unconditional
Fiat–Shamir theorem.

## Running

```sh
# Native multiplication tables (adds the binius64-ligerito rows; rate 1/8 fixed).
F2Z_BENCH_SHAPES="15 16" F2Z_BENCH_REPS=5 F2Z_MUL_COMPARE_WORKLOADS="u32" \
F2Z_MUL_COMPARE_BACKENDS="f2z binius64 binius64-ligerito" \
RAYON_NUM_THREADS=8 bash scripts/run_native_mul_compare.sh
python3 scripts/native_mul_table.py PerfRuns/<run> --workload u32

# SHA-256 comparison.
F2Z_SHA_COMPARE_BACKENDS=binius64,binius64-ligerito F2Z_SHA_COMPARE_LOG_INV_RATE=3 \
  bash scripts/run_native_sha256_compare.sh

# Hybrid table: the all-Binius circuit with the F2Z opener.
RUSTFLAGS="-C target-cpu=native" RAYON_NUM_THREADS=8 \
  cargo bench --bench hybrid_u32_sha256 --features hybrid -- \
  --sweep --mode binius-ligerito --iterations 11

# PCS-only rows.
F2Z_PCS_COMPARE_BACKENDS="binius64-basefold f2z-ligerito-binary" F2Z_BINIUS_LOG_INV_RATE=3 \
  cargo bench --bench u32_pcs_compare --features bench-internals,plonky3-whir-bench,binius64-bench
```

Unit tests: `cargo test --release --lib --features binius64-bench -- binary_pcs binius_ligerito`
(a multiplication circuit with the two-oracle path, an AND-only circuit, the
opener's bit-MLE and arbitrary-basis openings, tamper rejection).

## First measurements (2026-09-10, Apple M5 24 GB, 8 threads, smoke runs)

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
| | f2z-ligerito-binary | commit 1.07, open 9.48 | — | 70,040 | 62 queries, 16 query + 14 fold grinding bits, target 104 → 100.8 bits |
| BabyBear PCS compare, 2^15 rows (1 sample) | binius64-basefold | commit 1.11, open 0.95 | — | 128,128 | 121 queries |
| | f2z-ligerito-binary | commit 1.55, open 10.4 | — | 70,136 | 62 queries, target 104 → 100.8 bits |

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

Reading: the proof-size win of the F2Z opener over BaseFold at rate 1/8 is
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
