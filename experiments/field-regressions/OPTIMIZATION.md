# Isolated ARM optimization candidates

This implements candidate kernels and source-linked comparisons inside this
experiment. Production arithmetic, callers, dependencies and enum dispatch are
unchanged. No x86 kernels or x86 measurements are part of this work.

Run the complete single-thread campaign from the repository root:

```sh
python3 experiments/field-regressions/run.py --suite optimization \
  --arch aarch64 --threads 1 \
  --out experiments/field-regressions/results/optimization-arm-01
```

The manifest is [optimization_cases.json](optimization_cases.json). Selection is
frozen before five fresh processes with 32 paired samples; selected inconclusive
cases receive at most one five-process retry with 64 samples. All rejected
candidates remain visible. A baseline self-comparison is never a new speedup.
Results include median, P95, confidence intervals, and warmed allocation counts
and bytes. The runner records the frozen source/executable paths and hashes.
The completed campaign also includes a source-and-executable archive.

Read the [confirmed results and per-workload recommendations](OPTIMIZATION_RESULTS.md).
The ten-core comparison reuses the identical executable:

```sh
python3 experiments/field-regressions/run_frozen.py \
  --from-run experiments/field-regressions/results/optimization-arm-01 \
  --out experiments/field-regressions/results/my-ten-core-run --threads 10 \
  --families opt_ood,opt_ood_reuse,opt_pack,opt_packed_ood,opt_ntt
```

If the temporary snapshot has been removed, extract the saved archive and pass
its executable using `--binary`. The runner validates the original binary hash.

## Implemented experiments

| Module | Implementations and controls |
|---|---|
| [integer.rs](src/campaign/candidates/integer.rs) | Wrapping MAC at 1/2/4/9 limbs; prior two-limb and nine-limb incumbents; deferred column carries; four-limb multiplication using two native u128 halves; fixed-width signed Horner projection at q100/q128; existing public-divisor and prepared-reciprocal division controls through 4096 bits. |
| [words.rs](src/campaign/candidates/words.rs) | Wrapping and checked signed/unsigned add/sub; full output schoolbook products including asymmetric and 2048/4096-bit controls; exact signed/unsigned MAC with an extra accumulator limb, compared with full product then add. |
| [prime.rs](src/campaign/candidates/prime.rs) | Public-length selection for delayed prime MAC; native linear MAC; actual one-inversion batch baseline, compact allocated and scratch-reuse alternatives, zero handling, fixed-iteration inverse control; sparse/dense public-exponent powers. |
| [binary.rs](src/campaign/candidates/binary.rs) | Actual private FixedGfMul baseline; ARM half-width fixed multiplication and butterflies; weighted single/two-pair rounds, fold-round and grid hooks; existing GF8 vector multiplication with a vector inverse chain; B127 controls; fixed-address embedding; fixed-schedule packed polynomial dot products. |
| [composite.rs](src/campaign/candidates/composite.rs) | OOD equality products reused across both children, preallocated tables, removal of the intermediate inner vector, reusable scratch; complete depth-first NTT with matching reset copies; single-write packing and packing plus OOD with the returned packed buffer retained. |

`Projection<L>` owns its modulus context and radix/sign correction constants.
`Case` and the shared `pass` helper own a distinct output buffer for each variant.
These are benchmark implementations, not a published unified library API. The
planned public traits and structs remain in [UNIFIED_LIBRARY_API.md](UNIFIED_LIBRARY_API.md).

## Boundaries that matter

- The four-limb MAC result is modulo 2^256. Exact MAC has a separate full-width
  product and accumulator contract. Both include full-width random and signed
  boundary tests; small-value timing fixtures do not authorize narrower math.
- Horner projection returns canonical residues of signed fixed-width integers.
  Its public modulus must be odd and greater than 2^64. Preparation and one-shot
  execution have separate rows. Full-width random projection is not a proxy for
  a workload dominated by tiny coefficients.
- GF scalar specialization and prime exponent branches use explicitly public
  pass constants. Fixed-address phi8 and zero-masked inversion have stronger
  input-timing contracts than their table/zero-skipping controls. Their cost is
  reported separately; a mathematical match alone does not certify timing safety.
- GF8 batching reuses Flock's existing vector multiplication kernel. Its gain
  over scalar F8 loops must not be described as beating that existing kernel.
- Prepared reciprocal division reuses crypto-bigint's existing primitive. Both
  quotient and full-width remainder writes are included, with public-divisor
  division retained as the stronger general control.
- Allocating batch inversion returns the same configured field representation
  as production. Conversion is timed. Scratch reuse has a separate comparison.
  All-zero and mixed-zero fixtures remain visible, including regressions.
- OOD and packing baselines are extracted from the real private functions.
  The benchmark explicitly routes their parallel iteration through Rayon at
  the requested thread count. It does not time transcript grinding or the later
  opening proof. The packing-plus-OOD case returns the packed Vec and result,
  preventing an apparent saving from silently discarding a later-required buffer.
- NTT/fold/grid reset copies are included equally. Setup remains outside the
  transform. Large NTT rows and a separate physical-core campaign test memory
  and parallel behavior; scalar microbenchmarks cannot establish prover speedup.

## Baseline extraction and audit

[build.rs](build.rs) extracts private production bodies with marker checks and
minimal representation/visibility or iteration-macro adapters. It does not edit
production source visibility. Frozen source and generated executable hashes are
recorded in each result directory. [BASELINES.md](BASELINES.md) contains the source
locations and contracts of the production and dependency controls.

Correctness runs before timing. BigInt/BigUint, bit-serial binary multiplication,
canonical residue equations, exhaustive GF8 pairs, and production fused outputs
cover the arithmetic changes. The timing audit is an ARM source/assembly review,
not a certification of the entire prover's constant-time behavior.

Operations without a new selected kernel retain the current implementation.
The earlier [operation metrics](OPERATION_METRICS.md) remain the evidence for
unchanged scalar GF add/square/inversion, checked integer multiplication and
prime codecs. This campaign does not implement general even-modulus rings,
signed division, private-exponent fixed-base tables or production migration.
