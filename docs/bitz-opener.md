# BitZ opening API

BitZ is always enabled. There is no PCS backend feature, opener enum or
backend CLI option. Ligerito ladder selection remains independent.

The PCS module is `bitz::bitz` (formerly `bitz::wfbitz`). The u32
multiplication relation lives in `bitz::piop::spartan::u32_mul_relation`.
Transcript domains and security-stage labels keep their historical spelling;
the rename does not change proof bytes. New benchmark records identify the
opener as `bitz`; result readers also accept `wfbitz` in saved measurements.

- `src/bitz/`: parameters, direct/virtual proofs, optimized kernels,
  transcript, grinding schedule and Ligerito adapter.
- `src/piop/spartan/protocol/bitz_opener.rs`: shared relation/standalone
  adapters, statement binding, concrete proof and bounded codec.
- `src/ligerito_flock.rs`: commitment hints, configuration and OOD support.
- `src/ligerito.rs`: packing, ring-switch and residual-evaluation kernels.
- `src/pcs.rs`: shared layout, generator checks and field reference helpers.

## Relations and ladders

Use `PreparedRelation::new` or an explicit security profile, then shared
`commit`, `prove` and `verify`. SHA, SHA+ECDSA and hybrid keep their specialized
relation preparation and share the native opening primitives.

`BitZOpener::prepare` supports direct relations with an explicit
`BitZLigerito` choice. `Fast` uses Flock's embedded configuration;
`Selected(LigeritoSelection)` resolves validated Johnson or unique-decoding
configurations. `prove_standalone`, `verify_standalone` and
`standalone_evaluation` implement the standalone claim flow.

Johnson ladders bind Round 0 before the prime draw and batch its claim into
the final opening. Unique-decoding configurations reject an unexpected OOD
record. Native grinding and Flock's work are independently checked; see
[the design](DESIGN.md) for schedule and security accounting.

## The clear opening (`m ≲ 16`)

Below the ladders' window (`m < 20`) no Ligerito configuration exists, and
below `m ≈ 16` none would pay: at 100 bits every sampled opening of a
`2^{m−7}`-element message reveals more symbols than the message has, and
its Merkle paths alone cost more than the message (see
`examples/tiny_opening_probe.rs`). `BitZLigerito::Clear` (`--ladder clear`,
`Pcs::clear`, `OpeningMode::Clear`) opens such a commitment in the clear:
the GKR runs as always and reduces the claim to one factored functional of
the bits; the prover then sends the packed message itself as one hint string
(`2^{m−3}` bytes) in place of the inner-product sumcheck, the ring switch
and Ligerito; the verifier re-commits the message under the commitment's
geometry (rate 1/2, `min(4, m − 8)` lane bits, the same Merkle hash), checks
the root and evaluates the functional on the bits. No Round 0, no grinding
and no soundness term of its own: the binding is the Merkle hash. The
verifier reads the whole witness, so the opening is not succinct; it is the
right opener exactly where the message is smaller than any sampled opening.
The commitment is the same object whichever way it is opened (the
configurations the frames carry describe its geometry only).

Measured (`bitz_bench <n> --ladder clear [--t T]`, MacBook Air M5, 1 thread,
medians of 5, 2026-11-02; bytes are exact and thread-independent):

| n | t, s | prove | verify | proof | message / GKR+folds |
|---|---|---|---|---|---|
| 10 | 7, 3 | 0.43 ms | 0.20 ms | 1 845 B | 132 / 1 696 |
| 12 | 7, 5 | 0.54 ms | 0.32 ms | 3 061 B | 516 / 2 528 |
| 13 | 7, 6 (reference) | 0.49 ms | 0.31 ms | 4 309 B | 1 028 / 3 264 |
| 13 | 9, 4 | 0.84 ms | 0.31 ms | **3 893 B** | 1 028 / 2 848 |
| 13 | 13, 0 | 5.2 ms | 0.73 ms | 3 973 B | 1 028 / 2 928 |
| 14 | 8, 6 | 0.59 ms | 0.25 ms | 5 781 B | 2 052 / 3 712 |
| 15 | 8, 7 | 0.73 ms | 0.37 ms | 9 109 B | 4 100 / 4 992 |
| 16 | 9, 7 | 0.93 ms | 0.35 ms | 13 717 B | 8 196 / 5 504 |
| 17 | 10, 7 | 1.48 ms | 0.48 ms | 22 453 B | 16 388 / 6 048 |

The bytes are `16·2^s` (the folds) `+ 32·(t·s + t(t+1)/2)` (two elements
per sumcheck round and two per layer) `+ 16·2^{n−7} + 4` (the message)
`+ 17` (framing). At `n = 13` every split from `t = 8` to `t = 13` lands
within 100 B of 3.9 KB; `t = 9` or `10` is the byte minimum and the
reference split proves fastest. Under 10 kB holds up to `n = 15`; from
`n = 16` the message dominates and a sampled opening takes over once a
ladder exists (`m ≥ 20`; the window between is open). Ten threads do not
help at these sizes (the 1-thread prove is already under a millisecond).

Why not a sampled opening at these sizes: `examples/tiny_opening_probe.rs`
prices the cheapest Johnson configuration flock's validator accepts at 100
bits (its own query counts, a simulated octopus multi-proof, a Ligerito
allowed to stop after its lane fold — a floor the crate does not run)
against the clear opening:

| m | clear | sampled total | of which Ligerito | cheapest L0 (rate, k, queries) |
|---|---|---|---|---|
| 10 | 1 845 B | 6 409 B | 2 168 B | 1/16, 1, 45 |
| 12 | 2 965 B | 8 937 B | 3 864 B | 1/8, 1, 60 |
| 13 | 3 893 B | 10 145 B | 4 608 B | 1/4, 1, 90 |
| 14 | 5 365 B | 13 449 B | 7 416 B | 1/16, 2, 45 |
| 15 | 7 893 B | 16 145 B | 9 584 B | 1/1024, 3, 20 |
| 16 | 12 501 B | 19 161 B | 12 040 B | 1/1024, 4, 20 |
| 17 | 21 237 B | 24 577 B | 16 864 B | 1/4096, 5, 18 |
| 18 | 38 197 B | 29 153 B | 20 816 B | 1/1024, 3, 20 (two levels) |

The sampled total adds the GKR at the same split, the inner-product
sumcheck and the ring switch (2 KB on its own). At `m = 13` its Ligerito
part alone exceeds the message, and its 90 opened columns of 2 elements
reveal the 64-element message anyway: no opening at this size and target
is succinct, so the clear one is strictly cheaper. The crossover is
`m ≈ 17–18`.

## Layouts

The raw PCS default is `t = ceil(3n/5) - 1`, clamped to the packing width.
`MulLayout::new` uses this split, with at least 64 gates per packed lane.
`with_split_shift` adjusts it explicitly. SHA's relation-specific layouts and
prime rules remain. The standalone `bitz` CLI (and its `--sweep` table) uses
the same default. Relation cells are bits.

## Running

```sh
RUSTFLAGS="-C target-cpu=native" cargo run --release --example bitz_bench -- \
  23 --reps 5 --seed 1 --ladder custom:1:4
# below the ladders: the clear opening, at an explicit split
RUSTFLAGS="-C target-cpu=native" cargo run --release --example bitz_bench -- \
  13 --reps 5 --ladder clear --t 9

python3 scripts/run_multiplication_benchmarks.py bitz -- \
  proof --workload u64 --ligerito custom:1:4 --bitz-profile 100 \
  --log-n 15,17,19 --threads 1,10 --reps 5 --memory rss

cargo run --release --example bitz_reference -- /path/to/dump
```

The reference example compares an upstream dump's commitment, then proves and
verifies its claim locally. Kernel tests compare optimized paths with
references and retain small-shape fallbacks.

[Port measurements](history/wfbitz-port-measurements.md) are historical records.
Retired backend results retain their original labels.
