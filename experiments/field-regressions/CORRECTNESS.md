# Arithmetic correctness audit

This report records the earlier 37-test audit. The subsequent
[candidate regression fixes](CANDIDATE_FIX_RESULTS.md) include new runtime
kernels and a final 39-test audit in three build configurations.

The audit of commit `2d67dc6` found one existing boundary bug in vendored Flock:
`AdditiveNttF128::new(&[])` (and `standard(0)`) inserted an empty evaluation row,
then indexed its first element during normalization. The expanded test first
failed with an out-of-bounds panic at that access. The constructor now returns
an empty layer table for the zero-dimensional domain. Its one-point forward
and inverse transforms are identity operations, including interleaved lanes.
The fix and a focused regression test are in
[additive_ntt_f128.rs](../../vendor/flock-mod/crates/flock-core/src/ntt/additive_ntt_f128.rs).

No arithmetic mismatch was found in the optimized kernels for the tested input
contracts. Their runtime bodies are unchanged. All additional candidate checks
are compiled only under `cfg(test)`; they neither enter timed loops nor alter
the recorded benchmark selections. The vendored zero-dimensional constructor
fix is the only production behavior change in this audit. Frozen performance
reports remain evidence for their original source snapshots, not new timings.

Final results on the Apple M1 Max with Rust 1.98.1:

| Run | Result |
|---|---|
| Native debug, overflow checks enabled | 37 passed, 0 failed |
| Native release | 37 passed, 0 failed |
| Generic ARM with AES/PMULL feature selection disabled | 37 passed, 0 failed |
| Vendored additive NTT suite | 12 passed, 0 failed |
| Python benchmark-harness tests | 17 passed, 0 failed |

## Independent checks

There are 28 new tests, plus nine existing tests of the included production
delayed reducer. Deterministic seeds make failures reproducible. Boundary
fixtures complement random values rather than relying only on random coverage.

| Operations | Independent oracle and coverage |
|---|---|
| Wrapping and checked unsigned/signed add/sub | BigInt arithmetic and range checks; every bit boundary for 1/2/4/9 limbs, carry/borrow chains, signed minimum/maximum, full returned values and validity flags |
| Widening multiplication | BigUint products through 64-by-64 limbs, both orientations of 2-by-9 limbs, maximum words, zero and dense inputs |
| Exact unsigned/signed MAC | BigUint/BigInt full sums; 1/2/4/9 limbs, maximum carries, sign extension, negative-times-negative, mixed signs, empty batches and tails through 1,025 terms |
| Wrapping MAC | BigUint sum modulo the declared storage width; independent accumulator chains and tail lengths |
| Signed projection | BigInt Euclidean remainder; odd moduli from just above 2^64 through 2^128−1, signed extrema and random full-width integers |
| Reciprocal division | BigUint quotient and remainder; small, power-of-two and near-maximum divisors, through 64 limbs |
| Montgomery arithmetic and delayed sums | BigUint raw-domain calculations with an extended-Euclid inverse of the Montgomery radix; separate product and linear scaling, full carries and multiple accumulator counts |
| Batch inversion and public powers | Extended Euclid and BigUint modular exponentiation; empty/all-zero/mixed batches, 0/1/q−1, exponent 0 and the full 128-bit exponent range boundaries |
| GF(2^128) multiply and dot | Independent bit-serial quotient-ring oracle; all 16,384 prepared-multiply basis pairs, all 8,192 half-width basis pairs, dense values and incomplete SIMD tails |
| GF(2^8) | Polynomial long division for all 65,536 products, exhaustive inverses, vector paths and short/tail batches |
| GF(2^127) | Bit-serial trinomial reduction for all 16,129 basis pairs and dense random operands, including both ARM multiplication alternatives |
| Polynomial MAC over GF(2) | Bit-by-bit convolution independent of carryless instructions; every limb crossing, highest monomials, zero/sparse/dense/misleading-prefix distributions, duplicate-row XOR cancellation and output padding |
| Sumcheck and fused folding | Direct evaluation of weighted affine products using bit-serial field multiplication; zero/one/random challenges, all written prefix values and all nine grid outputs, including 127/128/129/257-row chunk boundaries |
| OOD/equality tables | Direct adjacent affine elimination and tensor-product weights; Boolean-point table selection, the 12-bit block boundary and scratch reuse |
| Packing | Independent index-to-source mapping for every pair of lane widths, every padding slot, serial and parallel thresholds |
| NTT | Independent scalar breadth schedule with bit-serial multiplication and the unchanged twiddle plan; multiple bases, smaller-than-prepared domains, zero-dimensional transforms, isolated lanes, complete outputs and per-lane inverse roundtrips |

OOD, packing and NTT tests explicitly run with one and ten Rayon workers.
The separate vendored NTT suite also checks nonzero domains, parallel/scalar
agreement, cache-blocked transforms and the new identity case.

## Arithmetic invariants reviewed

- A schoolbook multiply step fits `u128`: with base B = 2^64, its maximum is
  `(B−1)^2 + (B−1) + (B−1) = B^2−1`.
- Exact delayed columns receive at most `2L` word contributions per term. The
  enforced term bound `n <= (B−1)/(2L+1)` leaves capacity for the carry from the
  preceding column. The `2L+1` output words hold the full unsigned or signed
  batch result under the supported slice-size bound.
- Signed exact multiplication subtracts each negative operand's unsigned
  counterpart shifted by `64L` and restores `2^(128L)` when both signs are set.
  This preserves the full two's-complement product before accumulation.
- Product Montgomery reduction removes one radix factor; linear reduction does
  not. Zero masking substitutes Montgomery one before batch inversion and
  restores zero outputs afterward.
- The half-width GF128 fold uses `x^128 = x^7+x^2+x+1`; its remaining folded
  polynomial fits in 128 bits. The public polynomial density choices change
  work performed but preserve every coefficient.
- Packing's spare-capacity writes initialize every value and padding slot
  before setting the vector length. Worker failure leaves length zero. Slice
  partitioning makes workers' destinations disjoint.

## Reproduce

Run from the repository root with dependencies cached:

```sh
RUSTFLAGS='-C target-cpu=native' cargo test --offline \
  --manifest-path experiments/field-regressions/Cargo.toml \
  --features arithmetic-campaign -- --test-threads=1
RUSTFLAGS='-C target-cpu=native' cargo test --offline --release \
  --manifest-path experiments/field-regressions/Cargo.toml \
  --features arithmetic-campaign -- --test-threads=1
RUSTFLAGS='-C target-cpu=generic -C target-feature=-aes' cargo test --offline \
  --manifest-path experiments/field-regressions/Cargo.toml \
  --features arithmetic-campaign -- --test-threads=1
RUSTFLAGS='-C target-cpu=native' cargo test --offline \
  --manifest-path vendor/flock-mod/Cargo.toml -p flock-core --lib \
  ntt::additive_ntt_f128::tests -- --test-threads=1
python3 -m unittest discover -s experiments/field-regressions -p 'test_*.py'
```

The third build disables the AES/PMULL feature selection on this ARM host,
exercising the shared field's portable implementation and the corresponding
candidate fallbacks. ARM NEON remains enabled; this is not an x86 test or a
claim that every dependency's SIMD implementation is disabled.

Logs, source/binary hashes, compiler flags and exact test outcomes are preserved
in [the audit evidence](results/correctness-arm-01/metadata.json).

## Scope and remaining contracts

These tests strengthen correctness evidence; they are not a formal proof or a
whole-prover soundness audit. NTT twiddle construction is reused from the
unchanged plan rather than independently reimplemented. No x86 validation was
performed.

Internal kernels still require their documented nonzero widths/accumulator
counts, matching slices, supported shapes, canonical residues and shared
modulus contexts. Batch inversion additionally requires a prime-field context
(or invertible nonzero inputs). Benchmark callers establish these conditions;
future public APIs must establish them at their outer boundary. Some private
zipped loops assume matching output lengths and are not general checked APIs.

Public sparse-polynomial paths and public-exponent dispatch retain their
variable-time public-input contracts. They must not replace private-input
fixed-schedule entry points. Arithmetic correctness tests do not establish
constant-time behavior; the prior bounded ARM instruction review remains
separate evidence for the frozen performance build.
