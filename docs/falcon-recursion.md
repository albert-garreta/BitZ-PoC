# Recursion over algebraic Falcon-512 proofs

Branch `falcon-recursion` (from `falcon-aerie-experiments` fa341d3ad). One
recursion step: a batch proof of the algebraic Falcon-512 relation
(configuration G: per-site grinding, small prime, keyed ring, 16 bridge rows,
optimized word binder) is verified by a BitZ circuit, and that circuit is
proved with FlockRec. Campaign data: `bench_results/falcon-recursion-20261018/`
(ignored by git); results below.

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

## Results

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
   the circuit on the public stream before checking π.
6. The production proofs are byte-identical to configuration G's before the
   hooks (`proof_debug_digest` 964daab7… at 1K, dcb4c8cf… at 32K).

Levers not pursued: Johnson at λ=128 (a level-0 binding beyond GF(2^128):
a certified Round-0 sample with grinding, or GF(2^256) out-of-domain
samples) would bring π to roughly the λ=100 size; fewer bridge columns or a
bridge whose root claim does not take the folds as public exponents; sparse
exception counts in the base proof.

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
```

`campaign.zsh` writes to `bench_results/falcon-recursion-20261018/` and
expects the fn-dsa case cache there (`BITZ_FALCON_CASE_CACHE`; the 1K and 32K
caches were copied from the configuration-G campaign, 4K was generated).
