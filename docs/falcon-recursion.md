# Recursion over algebraic Falcon-512 proofs

Branch `falcon-recursion` (from `falcon-aerie-experiments` fa341d3ad). One
recursion step: a batch proof of the algebraic Falcon-512 relation
(configuration G: per-site grinding, small prime, keyed ring, 16 bridge rows,
optimized word binder) is verified by a BitZ circuit, and that circuit is
proved with FlockRec. Campaign data: `results/falcon-recursion-20261018/`
(baseline) and `results/falcon-recursion-20261020/` (after the prover and
verifier optimizations, A/B against the baseline binary); results below.

## The circuit

`src/recursion/falcon_verifier.rs` and `src/recursion/falcon_opening.rs`
arithmetize `PreparedFalconAlgebraic::verify` for proofs of the `sha256-fs`
variant (every transcript the SHA-256 compression duplex, SHA-256 proof of
work and Merkle trees). The design is the one of the SHA-256+ECDSA and
self-recursion circuits: every Fiat–Shamir challenge is public, proof data is
committed, and everything that depends only on public values is computed
natively through the protocol's own code (`recursion_hooks.rs` and the
`recursion` child modules of the keyed ring, the arithmetic and word binder,
the integer bridge, the joint sumcheck and the shared opening).

In the circuit:

- the keyed ring's quotient (521 signed coefficients, absorbed, range checked
  against the exact bound, and its term `Σ_k (Σ_p c_p q ξ_p^k) K_k` of the
  ring target);
- the norm (cubic) and binder (quadratic) sumchecks over committed round
  polynomials, the norm terminal's one modular square, the binder's linear
  terminal and the source-opening value;
- the bridge's product forest (gpgkr) over committed messages, with one
  GF(2^128) product per layer;
- the joint binary sumcheck;
- the ring switch (the bit transpose is free) and the Ligerito opening:
  level 0's multi-proof over joint rows (unoccupied lanes are public zeros),
  the recursive levels' multi-proofs, and the compact final level, whose
  complete message is re-encoded (Reed–Solomon as public linear maps,
  settled) and its whole Merkle tree rebuilt;
- every grinding nonce, with the proof-of-work predicate on an unpublished
  seed: block grinding (bridge, opening), per-round grinding (norm, binder,
  joint), single sites (point, norm instance, merge, terminal merge) and the
  opening's work plan (atomic frames, fold and query grinding).

Public next to the Falcon statement (keys and targets): the commitment root
(32 B), the exception lists (sparse encoding), the bridge's integer folds
(16 B per bridge column; they enter the forest's root claim as exponents),
and the circuit's public stream (challenges drawn after committed data,
chaining values published to hash long public runs natively, query bits).

Validation (lib tests, ignored, need the configuration-G environment):
`falcon_circuit_matches_native_transcript` (every transcript operation equal
to the native verifier's: 9,996 at batch 16, 14,614 at batch 1,024),
`falcon_circuit_rejects_tampered_proofs` (19 altered parts, each rejected by
the circuit and the native verifier), `falcon_recursion_step_proves_and_verifies`.

## Prover and verifier optimizations (2026-10-20)

Three changes, none of which changes the relation's statement or the proof
format (π sizes move by at most 2.6 KB: the transcript differs, so the
queries do):

1. **Shared recorder terms** (`src/recursion/val.rs`). `G` and `Lc` hold
   `Arc<Bits>`: a value flows into many linear combinations, and a recorder
   bit is a support list, so copying 128 of them per term was most of the
   recording (0.50 → 0.37 s at 1,024 in a same-session A/B; the circuit
   digest unchanged).
2. **Structured FlockRec opening** (`src/piop/spartan/flockrec.rs`, the
   prover and verifier halves from the FlockRec-verifier line in
   `src/bitz`). The committed layout is *boxed*: each SHA-256 block's hints
   read from outside it (its 256 committed outputs, template locals
   6112..6375) move to a 2^9-row band at the top of its 2^13-row column, and
   the constant and the tail move to the last dyadic columns. The opening's
   committed weights are then per-class row vectors (two per column, one per
   region of the split derived layout) plus a row box (the band) and a
   column box (the tail), so the prover never forms the 2^m transposed
   weights, and the verifier evaluates their extension lazily. At 1,024
   (λ=128, dev build): "transpose 171 ms + dense sumcheck 185–284 ms" became
   "structured weights 36 ms + structured sumcheck 44 ms"; the verifier's
   "transpose 167 ms + opening 368 ms" became "lazy opening 28 ms". The early
   commitment lays out the same band (the recursion module records one
   SHA-256 compression once for the template's external hints);
   `FLOCKREC_NO_BAND=1` keeps the plain layout and the dense path.
3. **Forked recording** (`crates/circuit`). `Circuit::map_independent` runs
   self-contained sub-circuits; the compact recorder records them on forked
   recorders, shifts their indices (and the handles they return) past the
   forks before them and splices them in order, which is the sequential
   recording exactly (block templates are shared through `Arc`). The
   in-circuit Merkle multi-proofs and the final level's complete tree split
   into at most 64 subtrees, each with the siblings the proof spends inside
   it (assigned natively: query positions are public), so leaf hashes, the
   final codeword's rows and the paths record on the pool. At 1,024 the
   opening's recording went 0.19 → 0.09 s and the whole recording
   0.29 → 0.19 s.

### A/B campaign (2026-10-20)

The 10-18 binary (c2c14ae24, "old") against 5399485ae ("new"), interleaved
cell by cell with alternating order, one gated campaign on AC power
(20:36–20:55, clocks 3.7–5.0 GHz), same cells, selections and fn-dsa inputs
as below; `results/falcon-recursion-20261020/` holds the runs, `campaign.zsh`,
`summarize.py` and `summary.md`.

10 threads (rec step and final verify; π + public as before):

| λ | batch | rec step old → new | final verify old → new | π new | peak RSS old → new (GiB) |
|---|---|---|---|---|---|
| 128 | 1,024 | 1.42 → 0.85 s | 1.14 → 0.37 s | 308.5 KB | 9.0 → 3.6 |
| 128 | 4,096 | 1.36 → 0.85 s | 1.25 → 0.45 s | 305.9 KB | 9.3 → 4.1 |
| 128 | 32,768 | 5.00 → 1.56 s | 4.66 → 0.82 s | 352.0 KB | 9.9 → 6.4 |
| 100 | 1,024 | 1.13 → 0.79 s | 0.69 → 0.42 s | 91.5 KB | 6.3 → 3.7 |
| 100 | 4,096 | 1.69 → 0.96 s | 1.64 → 0.41 s | 99.5 KB | 9.5 → 5.2 |
| 100 | 32,768 | 2.71 → 1.67 s | 3.03 → 0.78 s | 108.4 KB | 9.4 → 7.5 |

1 thread:

| λ | batch | rec step old → new | final verify old → new |
|---|---|---|---|
| 128 | 1,024 | 3.09 → 3.20 s | 2.48 → 0.68 s |
| 128 | 4,096 | 3.50 → 2.71 s | 1.90 → 0.76 s |
| 128 | 32,768 | 8.30 → 4.38 s | 5.14 → 1.35 s |
| 100 | 1,024 | 2.42 → 1.94 s | 0.97 → 0.57 s |
| 100 | 4,096 | 3.82 → 3.03 s | 1.86 → 0.67 s |
| 100 | 32,768 | 6.11 → 4.26 s | 3.62 → 1.17 s |

Notes:

1. The old binary ran slower than in the 10-18 campaign (1.42 against 1.18 s
   at λ=128, 1K; 5.00 against 2.94 s at 32K): its dense opening faults about
   2 GiB in the prover's transpose and 4.5 GiB in the verifier's opening at
   1K, and its FlockRec prove swings with the machine's memory state
   (0.66–1.08 s across reps at 1K, 2.55–3.85 s at 32K) where the new one is
   steady (0.48–0.50 s at 1K). Against the 10-18 numbers, the step is 1.4×, 1.5×,
   1.9× faster at λ=128 (1K, 4K, 32K) and 1.2×, 1.3×, 1.1× at λ=100; the final
   verifier 2.2×, 2.0×, 4.2× and 1.5×, 2.0×, 1.8×.
2. Proof of work is one draw per grinding site, and the two binaries'
   transcripts differ. At 1 thread, λ=128, 1K the prime draw took 63 ms (old)
   against 808 ms (new) while the opening went 1.8 → 1.0 s
   (`trace1-{old,new}.out`), hence the unchanged step there.
3. Where the new step goes (λ=128, 1K, 10 threads): recording 0.21 s
   (concurrent with the 0.05 s witness pass), outer relation 0.14 s,
   FlockRec prove 0.50 s (GKR ≈ 0.16 s, inner sumcheck ≈ 0.10 s, structured
   opening ≈ 0.10 s, the rest batching and its proof of work). At λ=100 the
   early commitment at rate 1/64 takes 0.14–0.44 s and is on the critical
   path at 32K (passes 0.55 s against recording 0.37 s).

## Polylog BitZ at the reference split (2026-10-20)

`BITZ_REC_OPENER=log` (the Falcon step's default since 46592ddfb; `std`
restores the opener above) opens the outer proof with BitZ logarithmic
(`src/bitz/logarithmic.rs`, the virtual two-run path ported from the
SHA+ECDSA proof-size line): the derived grid takes the reference split
`t = ⌈0.6 n⌉ − 1` (ℓ₁ = 2^16–2^17 factors per grand product, ℓ₂ = 2^11–2^12
products, against 2^18–2^19 × 2^8–2^11 for the standard opener at T = 18/19),
the column folds are committed (127 live bit slots each) and proved by a
second product tree and one recursive BitZ instance instead of sent, `f` is
opened through the map as before, and the fold bits in a second Ligerito
run under `BITZ_REC_MU_LIG` (default: the outer selection). Every native
challenge, including the fold-bit tree's, is ground to the target, so it also
runs at 128 bits.

Proof bytes per part (one traced run per cell, `bytes/breakdown.md`):

| λ | batch | ℓ₁ × ℓ₂ | GKR on h | fold-bit tree | recursive instance | open f | open fold bits | π (std) |
|---|---|---|---|---|---|---|---|---|
| 128 | 1,024 | 2^16 × 2^12 | 13.1 KB | 4.5 KB | 8.5 KB | 275.7 KB | 110.4 KB | 414.0 KB (308.5) |
| 128 | 4,096 | 2^16 × 2^12 | 13.1 KB | 4.5 KB | 8.5 KB | 278.8 KB | 110.9 KB | 417.5 KB (305.9) |
| 128 | 32,768 | 2^17 × 2^12 | 14.3 KB | 4.5 KB | 8.5 KB | 302.6 KB | 110.5 KB | 442.1 KB (352.0) |
| 100 | 1,024 | 2^16 × 2^11 | 10.0 KB | 3.4 KB | 6.6 KB | 74.6 KB | 39.3 KB | 135.4 KB (91.5) |
| 100 | 4,096 | 2^16 × 2^12 | 10.5 KB | 3.6 KB | 7.2 KB | 77.5 KB | 39.3 KB | 139.6 KB (99.5) |
| 100 | 32,768 | 2^17 × 2^12 | 11.4 KB | 3.6 KB | 7.2 KB | 78.8 KB | 39.4 KB | 142.1 KB (108.4) |

Timings (one gated campaign, std and log interleaved on the same binary, AC
power, 3.7–4.6 GHz; `results/falcon-recursion-polylog-20261020/`), 10 threads:

| λ | batch | rec step std → log | FlockRec prove std → log | final verify std → log |
|---|---|---|---|---|
| 128 | 1,024 | 0.83 → 0.67 s | 0.50 → 0.33 s | 0.36 → 0.37 s |
| 128 | 4,096 | 0.83 → 0.75 s | 0.45 → 0.37 s | 0.42 → 0.41 s |
| 128 | 32,768 | 1.34 → 1.44 s | 0.67 → 0.71 s | 0.73 → 0.72 s |
| 100 | 1,024 | 0.63 → 0.56 s | 0.28 → 0.21 s | 0.33 → 0.32 s |
| 100 | 4,096 | 0.91 → 0.80 s | 0.41 → 0.32 s | 0.40 → 0.36 s |
| 100 | 32,768 | 1.45 → 1.30 s | 0.67 → 0.50 s | 0.67 → 0.65 s |

At 1 thread the step is 2.95 → 1.93, 2.62 → 2.24, 4.18 → 4.43 s (λ=128) and
1.84 → 1.61, 2.93 → 2.68, 4.15 → 3.95 s (λ=100); the two transcripts draw
different proof of work (one draw per site), which at 1 thread moves a step by
up to ~0.8 s (`results/falcon-recursion-20261020/trace1-*.out`).

Findings:

1. The proofs grow: +106–112 KB at λ=128 (+34–36 %), +34–44 KB at λ=100
   (+31–48 %). The grand-product part is about the standard one's size
   (26–27 against 30–48 KB at 128 bits, 20–22 against 15–29 KB at 100); the
   second commitment's own Ligerito run is the difference (110 KB under unique
   decoding at 128 bits, 39 KB at 100).
2. The recursion step is 10–19 % faster, except at λ=128, 32K (+7 %, where
   t = 17); FlockRec prove alone 18–34 % faster in those cells (0.50 → 0.33 s
   at λ=128, 1K), likely because the low-entropy tables have ℓ₁ + 1 = 2^16 + 1
   values instead of 2^18 + 1 (not profiled). The verifier is unchanged.
3. The polylog note's single merged Ligerito run (the fold bits joining a
   level of `f`'s ladder, a level-0 branch of ~12–16 KB instead of their own
   run) needs a Johnson ladder for `f`: unavailable at 128 bits over
   GF(2^128), and at 100 bits it would leave π ≈ 110 KB, still above the
   standard opener's. With Johnson over GF(2^256) (the proof-size lever above)
   both would apply at 128 bits.

## Results (2026-10-18 baseline)

M5 (10 cores, 24 GiB), AC power, one gated campaign (2026-10-18 15:54–16:07,
clocks 3.9–4.7 GHz); `results/falcon-recursion-20261018/` holds the raw runs,
`campaign.zsh` and `summarize.py`. Falcon-512, distinct fn-dsa inputs (seed
42), configuration G. Medians of 3 runs at 10 threads (2 at 32K, where the
harness's median is the slower one), 2 (1K) or 1 run at 1 thread.

- *base*: the production proof (`falcon_algebraic`, Blake3 transcript);
  *base\**: the `sha256-fs` proof the circuit verifies (same relation).
- *rec step*: witness pass and recording (concurrent), early commitment,
  outer relation, outer witness, FlockRec prove.
- *final verify*: the recording replayed on the public stream, the relation,
  the FlockRec verifier.
- *recursive*: π (FlockRec proof and commitment) + public (stream + root +
  sparse exceptions). Proof sizes are payload bytes.

Outer opening: λ=128 `udrg:3:4` (unique decoding, fold grinding), opening
rows T=18; λ=100 `custom:6:4` (Johnson), T=19 (the SHA-256+ECDSA recursion's
choice). Johnson cannot reach 128 bits here: flock rejects it at level 0
("expected_eps_ood_bits 116 < 128"), the post-commit binding being capped by
GF(2^128).

10 threads:

| λ | batch | base proof | recursive (π + public) | base prove | base\* prove | rec step | base verify | final verify |
|---|---|---|---|---|---|---|---|---|
| 128 | 1,024 | 226.8 KB | 330.2 KB (309.3 + 20.9) | 118 ms | 81 ms | 1.18 s | 5.5 ms | 0.82 s |
| 128 | 4,096 | 313.0 KB | 342.8 KB (307.3 + 35.5) | 162 ms | 148 ms | 1.29 s | 7.4 ms | 0.91 s |
| 128 | 32,768 | 593.2 KB | 503.1 KB (349.4 + 153.7) | 800 ms | 799 ms | 2.94 s | 20.0 ms | 3.48 s |
| 100 | 1,024 | 186.0 KB | 102.6 KB (91.1 + 11.5) | 39 ms | 39 ms | 0.93 s | 4.9 ms | 0.62 s |
| 100 | 4,096 | 259.4 KB | 123.4 KB (98.8 + 24.6) | 101 ms | 105 ms | 1.26 s | 6.8 ms | 0.81 s |
| 100 | 32,768 | 518.7 KB | 249.4 KB (108.5 + 140.9) | 712 ms | 748 ms | 1.90 s | 19.3 ms | 1.42 s |

1 thread:

| λ | batch | base prove | base\* prove | rec step | base verify | final verify |
|---|---|---|---|---|---|---|
| 128 | 1,024 | 519 ms | 382 ms | 2.99 s | 6.1 ms | 1.24 s |
| 128 | 4,096 | 740 ms | 711 ms | 3.32 s | 13.1 ms | 1.38 s |
| 128 | 32,768 | 3.57 s | 3.49 s | 5.88 s | 77.7 ms | 3.50 s |
| 100 | 1,024 | 123 ms | 127 ms | 2.30 s | 5.6 ms | 0.92 s |
| 100 | 4,096 | 407 ms | 414 ms | 3.70 s | 12.3 ms | 1.28 s |
| 100 | 32,768 | 3.17 s | 3.17 s | 5.16 s | 76.7 ms | 1.91 s |

Circuit sizes (SHA-256 blocks / committed bits / cells; outer committed and
derived layout bits):

| λ | 1,024 | 4,096 | 32,768 |
|---|---|---|---|
| 128 | 8,952 / 59.2 M / 184 M; 27/28 | 12,273 / 80.9 M / 252 M; 27/28 | 17,897 / 117.9 M / 367 M; 28/29 |
| 100 | 6,488 / 43.1 M / 133 M; 26/27 | 9,266 / 61.3 M / 190 M; 27/28 | 14,059 / 92.8 M / 289 M; 27/29 |

At 1,024 signatures and λ=128 the blocks are the opening's Merkle hashing
(6,109, 68 %), the transcript (2,457, 27 %) and proof of work (386, 4 %).
The recursion cells peak at 6–12.5 GB RSS (including an untimed
`CheckingWitgen` pass), the base at 0.1–2.5 GB.

Findings:

1. At λ=128 the outer proof is 307–349 KB at every batch (unique-decoding
   queries), so recursion is larger than the base at 1K and 4K and 15 %
   smaller at 32K (5 % against a base with sparse exception counts: the base
   spends 2 B per signature on them, 65.5 KB at 32K).
2. At λ=100 recursion compresses 1.8× (1K) and 2.1× (4K, 32K): π is
   91–109 KB.
3. The public bridge folds (16 B per bridge column) are 4, 16 and 131 KB; at
   32K and λ=100 they outweigh π.
4. The `sha256-fs` base proof is no slower than the production one, and
   faster at λ=128 (proof of work and Merkle hashing on the SHA-256
   extensions).
5. The final verifier takes 0.6–3.5 s against 5–78 ms natively: it re-records
   the circuit on the public stream before checking π (0.37–0.82 s after the
   10-20 changes).
6. The production proofs are byte-identical to configuration G's before the
   hooks (`proof_debug_digest` 964daab7… at 1K, dcb4c8cf… at 32K).

Levers not pursued: Johnson at λ=128 (a level-0 binding beyond GF(2^128):
a certified Round-0 sample with grinding, or GF(2^256) out-of-domain
samples); fewer bridge columns or a bridge whose root claim does not take the
folds as public exponents; sparse exception counts in the base proof.

Why π is ~310 KB at λ=128: about 85 % of it is the outer Ligerito opening's
query data (in a 1K run at rate 1/4, 112 KB of opened rows and 154 KB of
Merkle siblings out of 312 KB).
Unique decoding gives under one bit per query (≈0.83 at rate 1/8, ~155
level-0 queries, never fewer than ~130 at any rate), against ~3 bits per
query for Johnson at rate 1/64 (~30 queries at λ=100). Johnson over
GF(2^256) is built for the BitZ opener in `~/f2z-pcs-ecdsa-size`
(uncommitted) and for the base Falcon opening on `origin/falcon-pcs-levers`:
its Round 0 reaches ~225 bits and λ=128 needs 79, 60, 48 queries at rates
1/8, 1/16, 1/32. On SHA+ECDSA its λ=128 proofs were 0.48–0.52× the best
unique-decoding ones and 1.4–1.6× the λ=100 Johnson ones, which suggests
π ≈ 130–150 KB at 1K and 150–175 KB at 32K here (an estimate; porting it to
the virtual opening, structured prover and lazy verifier included, is not
done).

## Reproduce

```sh
# lib tests (validation)
CARGO_TARGET_DIR=$PWD/target-dev cargo test --release \
  --features recursion,falcon-hybrid,bench-internals,unchecked --lib --no-run \
  --config profile.release.lto=false --config profile.release.codegen-units=16
export BITZ_FALCON_GRINDING_MODEL=per-site BITZ_FALCON_PRIME=small BITZ_ARITHMETIC_RING=keyed \
  BITZ_FALCON_BRIDGE_ROWS=16 BITZ_ARITHMETIC_BINDING_STRATEGY=optimized
target-dev/release/deps/bitz-* falcon_circuit --ignored --nocapture

# benchmark binaries
RUSTFLAGS="-C target-cpu=native" CARGO_TARGET_DIR=$PWD/target-bench cargo bench \
  --features recursion,falcon-hybrid,bench-internals,unchecked --bench falcon_recursion --no-run
RUSTFLAGS="-C target-cpu=native" CARGO_TARGET_DIR=$PWD/target-blake3 cargo bench \
  --features falcon-hybrid,bench-internals,unchecked --bench falcon_algebraic --no-run
python3 scripts/bench_gate.py run --label falcon-recursion -- zsh results/falcon-recursion-20261018/campaign.zsh

# the A/B campaign (old binary copied to target-bench/ab/ before rebuilding);
# waits for AC power, then takes the gate
zsh results/falcon-recursion-20261020/launch.zsh
python3 results/falcon-recursion-20261020/summarize.py bench_results/falcon-recursion-20261020/runs

# standard against polylog BitZ (one binary, BITZ_REC_OPENER=std|log)
zsh results/falcon-recursion-polylog-20261020/launch.zsh
python3 results/falcon-recursion-polylog-20261020/summarize.py bench_results/falcon-recursion-polylog-20261020/runs
```

`campaign.zsh` writes to `bench_results/falcon-recursion-20261018/` and
expects the fn-dsa case cache there (`BITZ_FALCON_CASE_CACHE`; the 1K and 32K
caches were copied from the configuration-G campaign, 4K was generated). The
A/B campaign reads the same cache and writes to
`bench_results/falcon-recursion-20261020/`.
