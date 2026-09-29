# Wfbitz opening API

Wfbitz is always enabled. There is no PCS backend feature, opener enum or
backend CLI option. Ligerito ladder selection remains independent.

- `src/wfbitz/`: parameters, direct/virtual proofs, optimized kernels,
  transcript, grinding schedule and Ligerito adapter.
- `src/piop/spartan/protocol/wfbitz_opener.rs`: shared relation/standalone
  adapters, statement binding, concrete proof and bounded codec.
- `src/ligerito_flock.rs`: commitment hints, configuration and OOD support.
- `src/ligerito.rs`: packing, ring-switch and residual-evaluation kernels.
- `src/pcs.rs`: shared layout, generator checks and field reference helpers.

## Relations and ladders

Use `PreparedRelation::new` or an explicit security profile, then shared
`commit`, `prove` and `verify`. SHA, SHA+ECDSA and hybrid keep their specialized
relation preparation and share the native opening primitives.

`WfbitzOpener::prepare` supports direct relations with an explicit
`WfbitzLigerito` choice. `Fast` uses Flock's embedded configuration;
`Selected(LigeritoSelection)` resolves validated Johnson or unique-decoding
configurations. `prove_standalone`, `verify_standalone` and
`standalone_evaluation` implement the standalone claim flow.

Johnson ladders bind Round 0 before the prime draw and batch its claim into
the final opening. Unique-decoding configurations reject an unexpected OOD
record. Native grinding and Flock's work are independently checked; see
[the design](DESIGN.md) for schedule and security accounting.

## Layouts

The raw PCS default is `t = ceil(3n/5) - 1`, clamped to the packing width.
`MulLayout::new` uses this split, with at least 64 gates per packed lane.
`with_split_shift` adjusts it explicitly. SHA's relation-specific layouts and
prime rules remain. The standalone `bitz` CLI retains its published
`ceil(3n/5)` default. Relation cells are bits.

## Running

```sh
RUSTFLAGS="-C target-cpu=native" cargo run --release --example wfbitz_bench -- \
  23 --reps 5 --seed 1 --ladder custom:1:4

python3 scripts/run_multiplication_benchmarks.py bitz -- \
  proof --workload u64 --ligerito custom:1:4 --bitz-profile 100 \
  --log-n 15,17,19 --threads 1,10 --reps 5 --memory rss

cargo run --release --example wfbitz_reference -- /path/to/dump
```

The reference example compares an upstream dump's commitment, then proves and
verifies its claim locally. Kernel tests compare optimized paths with
references and retain small-shape fallbacks.

[Port measurements](history/wfbitz-port-measurements.md) are historical records.
Retired backend results retain their original labels.
