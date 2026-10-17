# Falcon prover flamegraphs: Aerie and BitZ at 32,768 signatures

Single-thread sampled profiles of the prover for Aerie v0.2.0 and two BitZ setups, with 10-thread prove, verify and memory figures and a proof-size breakdown. Every number is for one batch of 32,768 Falcon-512 signatures at 128-bit security on an Apple M5 (10 cores, AC power, campaigns serialized by `scripts/bench_gate.py`). The interactive page is `page/index.html`, also published at https://claude.ai/artifact/F2r1HKc15jQkHwVBCHAvPZ.

## Setups

| Name | Code | Settings |
|---|---|---|
| Aerie | a16z/aerie `49b6c212` (v0.2.0), plus `aerie-profiling.patch` | As published: cargo bench profile, no LTO, no target-cpu flag, Rust 1.95 |
| BitZ, as shipped | John's `falcon-poc` `e140a3f3`, with the bench-only switches of `17482040f` (all off) and `profiling-additions.patch` | Union-bound grinding, default prime, canonical ring check, 2^13 bridge rows. At 10 threads `BITZ_ARITHMETIC_BINDING_STRATEGY=optimized` |
| BitZ, experiment | `falcon-ring-succinct` `d0ed8cfc1` plus this branch's profiling hooks | `BITZ_FALCON_GRINDING_MODEL=per-site BITZ_FALCON_PRIME=small BITZ_ARITHMETIC_RING=keyed BITZ_FALCON_BRIDGE_ROWS=16`; at 10 threads also the optimized binder |

Both BitZ builds use `--profile release --features falcon-hybrid,bench-internals`, LTO and `-C target-cpu=native` (Rust 1.98.1).

## Results

One thread, milliseconds per prove by kind of work (unprofiled totals; the split comes from the profiles):

| Kind of work | Aerie | BitZ, as shipped | BitZ, experiment |
|---|---:|---:|---:|
| Witness preparation | 342 | 130 | 128 |
| Signature decoding | 136 | – | – |
| Commitment | 592 | 194 | 162 |
| Ring and norm arguments | 2,437 | 1,643 | 1,740 |
| Bit-to-integer bridge (GKR forest) | – | 1,696 | 879 |
| PCS opening | 641 | 179 | 175 |
| Proof-of-work grinding | 3 | 429 | 411 |
| Other | 17 | 79 | 96 |
| **Total** | **4,168** | **4,349** | **3,591** |

Ten threads, medians of five proves (`t10/`):

| | Prove | Verify | Peak memory | Proof |
|---|---:|---:|---:|---:|
| Aerie | 968 ms | 15.3 ms | 2.0 GiB | 1,378,672 B |
| BitZ, as shipped | 1,015 ms | 193 ms | 3.9 GiB | 1,767,336 B |
| BitZ, experiment | 747 ms | 20.1 ms | 2.4 GiB | 593,232 B |

BitZ as shipped enables its optimized binder only at 1, 8, 12 or 16 workers; with its default policy at 10 workers it proves in 1.31 s and verifies in 502 ms.

Proof sizes split by component are in `sizes/` and on the page. 95% of Aerie's proof is the per-signature 40-byte salts; with one shared salt it would be 67,992 B. In BitZ as shipped, three quarters of the proof is the bridge's clear column sums: 65,536 columns of 20 B. With 2^16 bridge rows and one limb that becomes 8,192 columns of 16 B. The experiment's largest part is then the Ligerito opening (374 kB, of which Merkle paths are 218 kB and opened rows 144 kB).

## Method

- samply 0.13.1 samples every thread at 250 Hz and records each thread's CPU time since its previous sample. `samply2folded.py` weights every stack by that CPU delta, so the sampler's own pauses do not bias the split.
- Each profile covers five proves. `flame.py` keeps the stacks under the bench's marker frames (`profile_prove_region`, and `profile_commit_region` for BitZ), removes Rayon and closure-call frames, shortens symbol names, and scales the total to the same setup's unprofiled single-thread time.
- At one thread Rayon would run parallel work on its worker, cut off from the caller's stack. `BITZ_PROFILE_IN_POOL=1` and `AERIE_PROFILE_IN_POOL=1` run the prover inside the pool, so nested parallel work runs inline. BitZ uses Flock's pool, which also receives Flock's own installs.
- Validation:
  - Sampled CPU time is at least 98.8% of wall time for all three provers.
  - BitZ's commit share agrees with its own timer to within 0.1 points: 7.59/7.58% as shipped, 9.15/9.09% experiment.
  - Span-traced single-thread BitZ runs (`trace/`, `compare_trace.py`) match the sampled bridge, arithmetic, ring and opening shares within one point.
  - Aerie's commit, opening and sumcheck-stage totals fall within 3% of its stage timers.
- The first campaign used macOS `sample` (`profile-t1.zsh`, `profile-rerun.zsh`). It mis-attributed phases by up to 18% (BitZ commit share 9.0% against 7.6%), so it was replaced; only its unprofiled clock checks (`samples/`) are kept.
- Kinds of work are assigned by frame name in `build_page.py`. A stack takes the kind of its outermost phase frame. A grinding or signature-decoding frame overrides that anywhere on the stack.

## Files

- `page/`: the built page (`index.html`), its template, `snap.swift` (offscreen WebKit snapshot used to check the page) and `wrap_preview.py`.
- `out-samply/`: per-profile cleaned folded stacks (ms per prove), inferno SVGs, JSON trees and `summary.json`. `bitz-onelimb` is the earlier experiment with 2^13 bridge rows and the canonical ring check (3.21 s per prove, 1.51 MB proof).
- `samply/`: bench output and CPU-weighted folded stacks of the samply runs. `*.clock.*` are the unprofiled single-thread runs used for scaling.
- `t10/`: the 10-thread campaign. `sizes/`: 10-thread proves that report `pcs_breakdown`. `trace/`: span-traced single-thread BitZ runs.
- Scripts:
  - Pipeline: `samply2folded.py` → `flame.py` → `build_page.py`.
  - Checks: `validate.py`, `compare_trace.py`, `show_tree.py`, `debug_other.py`, `test_short.py`.
  - Campaigns: the `*.zsh` files.
  - The scripts hold absolute paths to the campaign directory where they ran (`bench_results/flamegraph-20261017` in the `falcon-flamegraph` worktree). `collect.py` copied this subset from there.

## Reproduce

1. Build the BitZ bench at the commit for the setup:
   `CARGO_TARGET_DIR=target-native RUSTFLAGS='-C target-cpu=native' cargo bench --offline --no-run --profile release --features falcon-hybrid,bench-internals --bench falcon_algebraic`
2. Build Aerie at `49b6c212` with `aerie-profiling.patch` applied:
   `cargo +1.95 bench --locked -p aerie-bench --features falcon-bench --bench falcon --no-run`
   Then generate its fixtures with `--generate-honest-fixtures-only 32768`.
3. Profile one setup:
   `BITZ_PROFILE_IN_POOL=1 RAYON_NUM_THREADS=1 samply record --save-only --unstable-presymbolicate --rate 250 -o NAME.samply.json -- <bench> --degree 512 --batch 32768 --security 128 --threads 1 --warmup 0 --iterations 5`
   Export the setup's variables first: samply cannot inject into `/usr/bin/env`.
4. Process the profile:
   `python3 samply2folded.py NAME.samply.json NAME.samply.folded cpu`, then `flame.py` and `build_page.py`.
