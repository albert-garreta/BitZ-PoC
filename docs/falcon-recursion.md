# Recursion over algebraic Falcon-512 proofs

Branch `falcon-recursion` (from `falcon-aerie-experiments` fa341d3ad). One
recursion step: a batch proof of the algebraic Falcon-512 relation
(configuration G: per-site grinding, small prime, keyed ring, 16 bridge rows,
optimized word binder) is verified by a BitZ circuit, and that circuit is
proved with FlockRec. Campaign data: `results/falcon-recursion-20261018/`
(baseline), `results/falcon-recursion-20261020/` (after the prover and
verifier optimizations, A/B against the baseline binary),
`results/falcon-recursion-polylog-20261020/` (polylog BitZ for the outer
proof), `results/falcon-recursion-sha-breakdown-20261021/` (the circuit's
SHA-256 compressions per stage), `results/falcon-recursion-ext-20261021/`
(Johnson over GF(2^256) for the outer proof at λ=128) and
`results/falcon-recursion-polylog-bridge-20261022/` (the base proof with the
logarithmic BitZ bridge); results below.

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

### Where the SHA-256 compressions come from (2026-10-21)

A hash over proof data is an in-circuit SHA-256 compression (one over public
data only is computed natively and enters as a public value). Three sources:
the Fiat–Shamir duplex (*transcript*: proof messages absorbed, challenges
squeezed), the proof-of-work predicates (*proof of work*, one compression per
nonce check), and the Ligerito opening's Merkle hashing (*Merkle*: the opened
rows' leaves, the multi-proofs' nodes, the final level's complete tree).
`BITZ_REC_STAGES=1` prints them per verifier stage, and per opening level the
Merkle split into leaves and nodes (d3eb249de). One recording per cell of the
test fixture (`falcon_record_profile`; the bench instance has 8,952 blocks at
λ=128 and 1K against the fixture's 9,028),
`results/falcon-recursion-sha-breakdown-20261021/`:

| stage | λ128 1K | λ128 4K | λ128 32K | λ100 1K | λ100 4K | λ100 32K |
|---|---|---|---|---|---|---|
| keyed ring | 79 | 76 | 79 | 73 | 73 | 73 |
| norm sumcheck | 270 | 249 | 267 | 119 | 116 | 131 |
| binder sumcheck | 258 | 275 | 293 | 133 | 135 | 153 |
| bridge (product forest) | 1,325 | 1,485 | 1,725 | 529 | 593 | 689 |
| joint sumcheck | 218 | 236 | 263 | 74 | 80 | 89 |
| opening: ring switch + L0 folds | 104 | 104 | 104 | 53 | 53 | 53 |
| opening: level 0 | 3,243 | 3,784 | 4,777 | 2,631 | 3,073 | 3,796 |
| opening: recursive levels | 2,373 | 3,892 | 6,274 | 1,879 | 3,067 | 4,908 |
| opening: final level | 1,158 | 2,160 | 4,204 | 1,112 | 2,121 | 4,168 |
| **total** | **9,028** | **12,261** | **17,986** | **6,603** | **9,311** | **14,060** |
| of which transcript | 2,485 (28 %) | 2,714 (22 %) | 3,069 (17 %) | 1,376 (21 %) | 1,503 (16 %) | 1,710 (12 %) |
| of which proof of work | 386 (4 %) | 434 (4 %) | 504 (3 %) | 0 | 0 | 0 |
| of which Merkle | 6,157 (68 %) | 9,113 (74 %) | 14,413 (80 %) | 5,227 (79 %) | 7,808 (84 %) | 12,350 (88 %) |

The Merkle hashing per opening level: opened rows × elements per row, then
leaf + node compressions (a compression block holds four GF(2^128) elements;
a node is one block):

| level | λ128 1K | λ128 4K | λ128 32K | λ100 1K | λ100 4K | λ100 32K |
|---|---|---|---|---|---|---|
| 0 | 309×16: 1,236 + 1,838 | 309×16: 1,236 + 2,379 | 309×16: 1,236 + 3,372 | 242×16: 968 + 1,536 | 242×16: 968 + 1,979 | 241×16: 964 + 2,706 |
| 1 | 190×8: 380 + 897 | 189×8: 378 + 1,261 | 189×8: 378 + 1,820 | 148×8: 296 + 750 | 148×8: 296 + 1,038 | 148×8: 296 + 1,447 |
| 2 | 156×8: 312 + 471 | 155×8: 310 + 761 | 155×8: 310 + 1,248 | 122×8: 244 + 410 | 121×8: 242 + 627 | 121×8: 242 + 995 |
| 3 | – | 142×8: 284 + 457 | 141×8: 282 + 858 | – | 111×8: 222 + 389 | 110×8: 220 + 684 |
| 4 | – | – | 135×8: 270 + 544 | – | – | 106×8: 212 + 489 |
| final (complete tree) | 512×4: 512 + 511 | 1,024×2: 1,024 + 1,023 | 2,048×2: 2,048 + 2,047 | 512×4: 512 + 511 | 1,024×2: 1,024 + 1,023 | 2,048×2: 2,048 + 2,047 |

1. The base proof's opening is 76–85 % of the compressions at λ=128 and
   86–92 % at λ=100; its Merkle hashing alone is 68–80 % and 79–88 %, the
   nodes 60–69 % of it.
2. The query counts are unique decoding's: level 0 at rate 1/2 opens 309 rows
   at λ=128 (0.415 bits per query) and 242 at λ=100, and the recursive levels
   (rates 1/4 to 1/32) 135–190 and 106–148. A level-0 query costs four leaf
   compressions (16 elements) and its share of the path, 6–11 nodes as the tree
   deepens with the batch.
3. The opening carries the growth with the batch: at λ=128 it goes from
   6,878 to 15,359 compressions (1K → 32K) while the stages before it go from
   2,150 to 2,627. Each step (1K → 4K → 32K) adds a recursive level (880–950
   compressions when added), deepens the existing trees, and doubles the final
   level's complete tree (1,023 → 2,047 → 4,095 compressions): the compact
   final level ships the message, and the circuit re-encodes it and rebuilds
   the whole tree.
4. Outside the opening the bridge's product forest is the largest stage
   (1,325–1,725 at λ=128, 265–345 of them proof of work). At λ=100 no site
   grinds, and the transcript is 55 % of the λ=128 count.

Levers on the circuit (not done): Johnson over GF(2^256) for the base
opening as well (`origin/falcon-pcs-levers`: 79 level-0 queries at rate 1/8
instead of 309; the circuit would need GF(2^256) arithmetic for the extension
levels), and a final level opened at its 131–141 queried rows instead of
rebuilt (an estimate from the expected multi-proof: ~455 instead of 1,023
compressions at 1K, ~670 instead of 4,095 at 32K, for ~15 KB more base
proof). The base opening is `MATCHED_UDR` (`udrg:1:4`).

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
   standard opener's. Johnson over GF(2^256) (next section) makes `f`'s
   Johnson ladder available at 128 bits, but the polylog opener's fold-bit run
   is GF(2^128) only, so neither the merged run nor polylog with an `ext:`
   ladder is wired.

## Johnson over GF(2^256) for the outer proof (2026-10-21)

`BITZ_REC_LIG=ext:R:K` (5872fe8e7) opens the outer proof with a Johnson
ladder at rate 2^−R whose Round 0 and levels past the first run over
GF(2^256) = GF(2^128)[Y]/(Y² + Y + c); level 0's rows stay GF(2^128)
elements. The GF(2^128) Round 0 reaches 116 bits, this one ~225, so 128-bit
Johnson becomes available. Ported pieces: flock's GF(2^256) field, extension
Ligerito (prover, verifier, proof type) and the `ext:R:K` selections, applied
from 411977004 (`falcon-pcs-levers`); the BitZ PCS extension path
(`Pcs::with_extension`, `prove_mle_ext` / `verify_mle_ext`, the Round-0 claim
as `OodClaim::{Base, Ext}` through the virtual, chained and logarithmic
openers) from the SHA+ECDSA proof-size line. FlockRec switches the PCS and
Round 0 to the extension when the selection asks for it, and the BitZ opening
codec carries `y_ext`. An `ext:` ladder runs with the standard opener (the
step's default opener, polylog, refuses it: see above). Test:
`extension_johnson_opening_proves_and_verifies` (codec roundtrip; altered
narg, hint and `y_ext` rejected).

λ=128, standard opener at T = 18, one gated campaign with the four ladders
interleaved per cell on one binary (built at 5872fe8e7), AC power,
3.8–4.0 GHz at 10 threads and 4.4–4.55 GHz at 1;
`results/falcon-recursion-ext-20261021/`. Medians of 3 runs at 10 threads (2
at 32K, where the median is the slower one), 2 (1K) or 1 at 1 thread. base\*
is the `sha256-fs` base proof the circuit verifies (223.8 / 313.6 / 593.1 KB;
the production Blake3 proof is 226.8 / 313.0 / 593.2 KB).

10 threads:

| batch | outer ladder | π | π + public | ÷ base\* | rec step | FlockRec prove | early commit | final verify | RSS |
|---|---|---|---|---|---|---|---|---|---|
| 1,024 | udrg:3:4 | 308.5 KB | 329.5 KB | 1.47 | 0.83 s | 0.49 s | 0.04 s | 0.37 s | 3.6 GiB |
| 1,024 | ext:3:4 | 187.6 KB | 208.5 KB | 0.93 | 0.76 s | 0.42 s | 0.04 s | 0.39 s | 3.6 GiB |
| 1,024 | ext:5:4 | 157.5 KB | 178.5 KB | 0.80 | 0.83 s | 0.44 s | 0.11 s | 0.37 s | 4.2 GiB |
| 1,024 | ext:6:4 | 150.1 KB | 171.1 KB | 0.76 | 1.01 s | 0.53 s | 0.23 s | 0.37 s | 5.0 GiB |
| 4,096 | udrg:3:4 | 305.9 KB | 341.3 KB | 1.09 | 0.82 s | 0.43 s | 0.04 s | 0.41 s | 4.1 GiB |
| 4,096 | ext:3:4 | 188.2 KB | 223.6 KB | 0.71 | 0.82 s | 0.44 s | 0.04 s | 0.41 s | 4.1 GiB |
| 4,096 | ext:5:4 | 157.9 KB | 193.3 KB | 0.62 | 0.89 s | 0.43 s | 0.13 s | 0.41 s | 4.6 GiB |
| 4,096 | ext:6:4 | 151.2 KB | 186.6 KB | 0.60 | 0.96 s | 0.45 s | 0.23 s | 0.41 s | 5.5 GiB |
| 32,768 | udrg:3:4 | 352.0 KB | 505.7 KB | 0.85 | 1.34 s | 0.66 s | 0.10 s | 0.73 s | 6.9 GiB |
| 32,768 | ext:3:4 | 214.5 KB | 368.1 KB | 0.62 | 1.31 s | 0.64 s | 0.09 s | 0.71 s | 6.9 GiB |
| 32,768 | ext:5:4 | 180.5 KB | 334.1 KB | 0.56 | 1.58 s | 0.78 s | 0.34 s | 0.71 s | 8.1 GiB |
| 32,768 | ext:6:4 | 173.0 KB | 326.7 KB | 0.55 | 1.78 s | 0.77 s | 0.55 s | 0.72 s | 9.6 GiB |

1 thread (rec step / FlockRec prove / early commit / final verify):

| batch | udrg:3:4 | ext:3:4 | ext:5:4 | ext:6:4 |
|---|---|---|---|---|
| 1,024 | 2.92 / 2.24 / 0.13 / 0.61 s | 2.38 / 1.71 / 0.13 / 0.61 s | 2.81 / 1.77 / 0.49 / 0.61 s | 3.70 / 2.17 / 0.98 / 0.61 s |
| 4,096 | 2.60 / 1.79 / 0.13 / 0.72 s | 2.57 / 1.76 / 0.13 / 0.72 s | 2.82 / 1.65 / 0.50 / 0.72 s | 3.40 / 1.74 / 0.98 / 0.73 s |
| 32,768 | 4.18 / 2.70 / 0.27 / 1.24 s | 3.91 / 2.43 / 0.27 / 1.23 s | 5.66 / 3.42 / 1.03 / 1.24 s | 6.49 / 3.24 / 2.03 / 1.24 s |

Findings:

1. π drops 38–39 % at rate 1/8 (ext:3:4: 187.6–214.5 KB against
   305.9–352.0 KB) and 48–51 % at rates 1/32 and 1/64; rate 1/64 saves only
   7–8 KB more than 1/32. With the public part, recursion is now smaller than
   the base proof at every batch at λ=128: 0.80 / 0.62 / 0.56 of base\* with
   ext:5:4 (unique decoding: 1.47 / 1.09 / 0.85). At 32K the public bridge
   folds (131 KB) are now about 40 % of the recursive bytes.
2. ext:3:4 has unique decoding's rate, so the early commitment is the same,
   and the step is no slower: 0.76 / 0.82 / 1.31 s against
   0.83 / 0.82 / 1.34 s at 10 threads (FlockRec prove 0.42 against 0.49 s at
   1K: fewer queries). The 1-thread gap (2.38 against 2.92 s at 1K) includes
   different proof-of-work draws (one per site per transcript).
3. Lower rates move cost into the early commitment: 0.11–0.34 s at rate 1/32
   and 0.23–0.55 s at 1/64 (10 threads; 0.49–1.03 and 0.98–2.03 s at
   1 thread). At 10 threads it runs beside the recording and slows it
   (0.19–0.21 → 0.31–0.35 s at 1K with ext:6:4), and at 32K it is the
   critical path (passes 0.62–0.67 s). Against unique decoding the step costs
   +0–18 % with ext:5:4 and +17–33 % with ext:6:4 (10 threads), RSS +14–18 %
   and +34–41 %.
4. The final verifier is unchanged: the replay of the circuit dominates it.
5. Where ext:6:4's bytes go (dev build, 1K): narg 36.5 KB (folds 16.4 KB,
   GKR 14 KB), opened rows 42 KB, Merkle siblings 65.7 KB.

The defaults are unchanged (`udrg:3:4` with the polylog opener at 128 bits).
ext:3:4 costs nothing and ext:5:4 is the size sweet spot.

## Cost against the standalone SHA-256 prover (2026-10-21)

The circuit's hashes are CM instances. Every compression is
`circuit::sha256::compress_block`, with words held as F2 bits and as lifted Z
bits (doubled Ch and Maj): 6,376 committed hint bits, 20,456 cells and 184
integer rows. FlockRec proves committed bits `f`, `h = M·f` over F2 and
`C·h = 0` over Z, and the SHA blocks are 96–97 % of the committed bits and
99.5 % of the cells. Same features (`recursion`: SHA-256 transcripts and
Merkle trees), λ=128, 10 threads, AC, gated
(`results/falcon-recursion-vs-sha-20261021/`):

| prover | compressions | prove | per compression |
|---|---|---|---|
| `sha256_compressions` (repeated Spartan + virtual BitZ, `udrg:1:4`) | 8,192 | 378 ms | 46 µs |
| same | 16,384 | 794 ms | 48 µs |
| FlockRec on the Falcon circuit at 1K (`ext:3:4`, T = 18) | 8,952 | 437 ms | 49 µs |
| FlockRec on the Falcon circuit at 32K | 17,897 | 652 ms | 36 µs |

The standalone excludes its witness generation (751 ms at 2^13); the step's
recording (0.20 s at 1K) is the equivalent. FlockRec at 1K: GKR on `h`
166–179 ms, opening (structured sumcheck, ring switch, Ligerito) 54–63 ms,
structured weights ~30 ms, folds ~10 ms; the remaining ~150 ms is the inner
sumcheck, prime, batching and proof of work (untraced).

The outer proof therefore costs what this repository's generic CM path costs
per compression. The slide-16 prover (85 ms at 2^13 and λ=100, ~10 µs per
compression) is the `sha-compressions` line (14d63cec, not on this branch):
Ch-only product slots (16,361 cells instead of 20,456), the Id ⊗ M block
opener and private I/O. The generic path takes 346 ms for the same 2^13 at
λ=100 on this binary. The Falcon circuit is almost entirely identical SHA
blocks, so that machinery is the main lever on the outer prover (not ported).

## SHA batch + glue on one commitment, and the opening rows (2026-10-21)

The BitZ paper's §2.4.1 is implemented for this circuit
(`docs/falcon-recursion-split.md`, `src/piop/spartan/split.rs`,
`src/bitz/shared.rs`, `c07b28ebc`): the compressions as one 2^13 batch of
the Ch-only relation with the block opener, the rest as FlockRec on a stub
circuit whose committed bits live in the batch's grid (the compressions'
inputs and outputs are the batch's own cells; no linking), one Round 0,
two PIOPs and one shared opening. It proves and verifies (tests, the
Falcon step at 16 and 1K), but it does not beat FlockRec
(`results/falcon-recursion-split-20261021/summary.md`, λ=128, ext:3:4
base and outer):

| batch | FlockRec t18 | FlockRec t17 | FlockRec t16 | FlockRec t15 | split (best) |
|---|---|---|---|---|---|
| 1,024 | 0.47 s / 170.6 KB | 0.39 s / 178.4 KB | 0.37 s / 192.8 KB | 0.34 s / 226.0 KB | 0.47 s / 208.4 KB (+0.55 s witness) |
| 32,768 | 0.66 s / 178.1 KB | 0.64 s / 195.0 KB | 0.56 s / 226.3 KB | 0.54 s / 291.5 KB | 0.69 s / 213.6 KB (+0.62 s witness) |

(rec step / π.) FlockRec's structured opening already prices a CM block like
the block opener prices a compression; the batch pads to 2^13 and the split
pays a second PIOP and a second fold + GKR. The cheap lever is FlockRec's
opening rows T: at λ=128 every PIOP round grinds ≈ T + 2 bits (the prime is
at most min(113, 128 − T) bits), so T = 18 spends most of its PIOP time on
proof of work; T = 17 (1K/4K) or T = 16 (32K) is 15–17 % faster per step for
5–27 % more proof.

## The logarithmic BitZ bridge in the base proof (2026-10-22)

With `BITZ_FALCON_BRIDGE=polylog` the base proof commits its bridge folds
instead of sending them (`integer_bridge::polylog`, `8b5c534d3`), and the
circuit verifies that bridge (`polylog_bridge`, `92d258f2b`). The fold
commitment's root is public and gets its own Round 0. The root value R
is committed. The source forest runs from R, and a fold tree over the
folds' bits runs from g·R: it proves the exponentiations g^T[c] as a
product tree with public images g^(2^k). The read-off on the reshaped
folds is an unsplit bridge with few public sums. Both fold claims are
batched, reduced by their own joint sumcheck and opened by ext:5:4. The
tests (transcript match, fourteen polylog tamper parts, the recursion
step) pass at 1K. The polylog bridge needs t ≥ 8 and s ≥ 7, so batches
of ≥ 512 at 16 bridge rows.
(`results/falcon-recursion-polylog-bridge-20261022/summary.md`, λ=128,
ext:3:4 main and outer):

| batch | standard t18 | polylog t18 | polylog t16 |
|---|---|---|---|
| 1,024 | 0.47 s / 190.9 KB | 0.70 s / 204.8 KB | 0.60 s / 252.1 KB |
| 4,096 | 0.50 s / 204.6 KB | 0.72 s / 206.4 KB | 0.67 s / 254.4 KB |
| 32,768 | 0.66 s / 330.6 KB | 0.93 s / 211.5 KB | 0.78 s / 259.8 KB |

(rec step / π + public stream.) At 32K the folds' 131 KB leave the public
stream: −36 % total proof for a 63 % larger circuit (7,681 → 12,549
compressions) and a final verifier 0.34 → 0.48 s. At 1K and 4K it does not
pay off. Of the extra compressions, the two forests cost 3,711 against
the standard forest's 1,725, and the read-off (1,004), the fold sumcheck
(190) and the fold opening (1,704) are new. The forests pay 7 compressions
per round against the standard bridge's 4, because of the grinding
boundary: the standard bridge's 58-byte domain makes each round's public
label run 4 blocks long (`PUBLISH_RUN`), so the chaining value is published
and the labels are hashed natively; the polylog's 38-byte domain gives
3-block runs, hashed in-circuit.

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

Levers not pursued on 10-18: Johnson at λ=128 (a level-0 binding beyond
GF(2^128): a certified Round-0 sample with grinding, or GF(2^256)
out-of-domain samples; the GF(2^256) one is done since 10-21, above); fewer
bridge columns or a bridge whose root claim does not take the
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
unique-decoding ones and 1.4–1.6× the λ=100 Johnson ones, which suggested
π ≈ 130–150 KB at 1K and 150–175 KB at 32K here. Measured after the port
(above): 150–158 KB at 1K and 173–181 KB at 32K at rates 1/64 and 1/32.

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

# unique decoding against Johnson over GF(2^256) at λ=128 (one binary,
# BITZ_REC_LIG=udrg:3:4|ext:3:4|ext:5:4|ext:6:4, standard opener at T = 18)
zsh results/falcon-recursion-ext-20261021/launch.zsh
python3 results/falcon-recursion-ext-20261021/summarize.py bench_results/falcon-recursion-ext-20261021/runs

# FlockRec against the standalone SHA-256 prover (sha256_compressions built
# with --features recursion,falcon-hybrid,bench-internals,unchecked,span-metrics)
python3 scripts/bench_gate.py run --label falcon-vs-sha -- zsh results/falcon-recursion-vs-sha-20261021/run.zsh

# the circuit's SHA-256 compressions per stage (the dev test binary above)
zsh results/falcon-recursion-sha-breakdown-20261021/shabreak.zsh /tmp/shabreak
python3 results/falcon-recursion-sha-breakdown-20261021/shabreak_table.py /tmp/shabreak

# standard against the logarithmic bridge in the base proof (one binary,
# BITZ_FALCON_BRIDGE=standard|polylog), and its per-stage compressions
python3 scripts/bench_gate.py run --label falcon-polylog -- zsh results/falcon-recursion-polylog-bridge-20261022/campaign.zsh
python3 results/falcon-recursion-polylog-bridge-20261022/summarize.py results/falcon-recursion-polylog-bridge-20261022/runs
zsh results/falcon-recursion-polylog-bridge-20261022/stages.sh
python3 results/falcon-recursion-polylog-bridge-20261022/stages_table.py results/falcon-recursion-polylog-bridge-20261022/stages
```

`campaign.zsh` writes to `bench_results/falcon-recursion-20261018/` and
expects the fn-dsa case cache there (`BITZ_FALCON_CASE_CACHE`; the 1K and 32K
caches were copied from the configuration-G campaign, 4K was generated). The
A/B campaign reads the same cache and writes to
`bench_results/falcon-recursion-20261020/`.
