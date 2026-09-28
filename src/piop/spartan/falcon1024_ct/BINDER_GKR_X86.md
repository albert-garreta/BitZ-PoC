# Falcon arithmetic binder and x86 GKR optimization

This change optimizes arithmetic coefficient processing and the wfbitz product
forest used by the Falcon bridge. The proof protocol, verifier, commitments,
field parameters, challenge order, and grinding schedule remain unchanged.
The matched ten-seed sweep improved aggregate throughput from **1,061.1 to
1,111.6 signatures/second** (+4.75%). Mean proving time fell from **0.942398 to
0.899637 ms/signature** (-4.54%). All 30 optimized measured trials exceeded
1,000 signatures/second, compared with 27/30 baseline trials. These are measured
batch results, not a minimum-rate guarantee.

The baseline is commit `41a54c83` (`refactor(falcon): retire legacy proving
backends`). Its saved release executable has SHA-256
`41bc2cd6a9a67f8e69a354a2f9deb67584487135662f4902f659d1878ce55835`.
Artifacts are under `bench_results/falcon-gkr-binder-20260927/`.

## Arithmetic binding

The three-variable prefix previously expanded compact word coefficients into
individual bit coefficients, accumulated physical eight-bit blocks, and then
grouped those blocks by their witness byte. The new path sends compact words
directly to those byte buckets. The resulting prefix sumcheck is identical.

A word is split into runs that occupy one physical byte and have a common
sign. For a run starting at lane `i`, with length `len` and first coefficient
`a`, the desired coefficients are `a, 2a, ..., 2^(len-1)a`. Accumulate:

```text
difference[i]       += a
difference[i + len] -= 2^len * a    // only when i + len < 8

value[-1] = 0
value[j]  = 2 * value[j - 1] + difference[j]
```

All operations are in the existing arithmetic field. Signed encoded words
split at their sign bit and physical-byte boundaries. Overlapping words and
corrections combine by linearity, so the bounded decoder and public-signature
encoding retain their original coefficients.

Boundary contributions use exact field-by-`u64` accumulators and reduce once
per occupied bucket lane. `FpLinearAcc<2, 1>` has four 64-bit limbs, including
the accumulator's extra limb. Accumulators belong to a single signature
partition; the fixed word/run counts stay within the library's fewer-than-
`2^64`-terms capacity contract. Negative contributions use the canonical field
negation, followed by exact unsigned integer multiplication and reduction.

Word geometry is shared through the existing compiled coefficient template.
Each partition still reads its own authenticated witness bits. A zero witness
byte contributes zero at every prefix evaluation and can be omitted.
Canonical coefficient checks happen before this omission. The checked byte
reader enforces alignment, partition bounds, and the occupied coefficient
lanes of a partial final byte. Bucket emissions must be canonical, unique,
and ordered. Other prefix widths and unsupported sources retain the existing
block or additive streaming paths.

## GKR column weights

Let `e[i] = eq(z, i)` be the equality table for the `s` column coordinates,
and let `n = 2^s`. Complementary entries satisfy:

```text
e[i] * e[n - 1 - i] = c
c = product(j = 0..s-1, z[j] * (1 - z[j]))
```

The existing guard excludes column coordinates equal to zero or one before
carrying weights, so `c` is invertible. At the first column round, the left
table contains `e[i] * left[i]`. Instead of computing every inverse `1/e[i]`,
the prover multiplies by the reversed equality table:

```text
weighted_left[i] *= e[n - 1 - i]    // now c * left[i]
message_factor  *= 1/c
terminal_left  *= 1/c              // immediately before the terminal pair
```

The common scale commutes with subsequent linear folds. Adjusting the round
message factor and normalizing the terminal left value therefore preserve
the messages, challenges, and outgoing claim. This replaces a serial batch
inversion and pointwise division with independent pointwise products and one
scalar inversion.

Starting to carry column weights now uses a per-layer cost heuristic. With
`r` subsequent tree rounds, the estimated saving is
`2 * n * (2^r - 1)` field multiplications. Carrying starts only if that exceeds
`n + 256`: one multiply per column to remove the weights, plus a conservative
scalar allowance for inversion and bookkeeping. The existing invertibility
guard still applies. This selects an implementation path; it does not alter
any proof obligation or security parameter.

## x86 kernels and fallbacks

- Nibble selection uses AVX2 byte-shuffle tables. Each vector handles 32
  sixteen-bit entries; decoding is shared across selectors. Paired selection
  combines its outputs before storing them. Interleaving also uses AVX2.
- Four-bucket accumulation loads each field weight once and applies it to
  four independent bucket streams. Updates remain sequential within each
  stream, preserving XOR accumulation when indices collide.
- Reversed equality-table multiplication uses four-lane field products with
  the 128-bit lanes reversed, followed by a scalar tail.

The nibble backend is compiled only for `x86_64` with `avx2`. The field-kernel
backend requires `x86_64`, `pclmulqdq`, `sse4.1`, `avx512f`, `avx512bw`, and
`vpclmulqdq`. Portable and existing ARM paths remain available when these
compile-time requirements are absent. SIMD accesses are unaligned and
bounded by complete input/output elements.

## Protocol preservation and validation

No security-bound term, transcript domain separator, challenge boundary,
grinding difficulty, or PCS opening configuration changes. The work remains
within the existing protocol's reported security convention. These kernel
changes do not independently establish a new cryptographic security claim.

The completed validation covers:

- Dense GKR transcript parity for ordinary and carried starts, several entry
  offsets, non-unit incoming factors, zero/one coordinates, and carry-gate
  boundaries.
- Equality-table complement identities; SIMD reverse multiplication tails;
  exhaustive sixteen-bit selector inputs and interleave boundaries; bucket
  collisions, padding, and guard elements.
- Direct byte buckets against dense coefficients for unsigned and signed
  words, overlaps, all witness-byte patterns, and several runtime primes.
- Rejection of invalid byte reads, coefficient padding, noncanonical values,
  and duplicate or descending buckets before transcript changes.
- Full arithmetic-binding transcript parity for prefix widths three and four;
  applicable native and portable tests; verified small and padded batches.
- Matched full-proof digests, roots, statements, proof sizes, witness counts,
  and complete reported security terms across the ten-seed benchmark.

**Validation results:** the complete portable library suite passed **678 tests**
with **7 ignored**, using `RAYON_NUM_THREADS=16 cargo test --offline --features
falcon-hybrid --lib -- --test-threads=1`. The native build additionally passed
**172 targeted tests**: 42 wfbitz, 37 packed inner sumcheck, and 93 Falcon tests.
The native test build used `CARGO_TARGET_DIR=target/falcon-native` and
`RUSTFLAGS='-C target-cpu=native'`; test execution used 16 Rayon threads and
one test thread. Test logs are saved in the artifact directory.

All eight initial profile proofs verified with identical complete Debug proof
digests, roots, payload sizes, and security metadata. The digest is over the
proof's Debug representation, not a canonical serialization. The unit tests
separately compare exact sumcheck/GKR transcript messages.

## Measurement method

Both executables use the release profile with
`RUSTFLAGS='-C target-cpu=native'`, batch size 1,024, 16 threads, target security
128, one warmup, and three measured trials. The fixed input seeds are:

```text
42, 7, 2026, 0, 1, 2, 3, 17, 99, 123
```

Run each seed as a paired baseline/optimized comparison, alternating which
executable runs first. Benchmarks run sequentially without concurrent builds
or tests. Keep preprocessing exclusions identical, and include witness
generation, commitments, HashToPoint, SHAKE, all proof phases, and the shared
opening in full proving time. Every warmup and measured proof must verify.

Acceptance uses uninstrumented runs. Separate stage-instrumented runs report
arithmetic binding and its prefix, GKR forest, bridge, binary authentication,
and shared-opening timings. Nested spans are not additive, and stage medians
are not a decomposition of the full-prover median. Process peak RSS includes
the whole benchmark process rather than one isolated phase.

The artifact directory records the executable hashes, source revision,
settings, exact commands, stdout JSONL, stderr, process resource measurements,
and scripts for validating and summarizing the paired runs. Report per-seed
medians alongside the aggregate rate:

```text
aggregate signatures/second = 30 * 1024 * 1000 / sum(measured prover ms)
```

Count seed medians and individual measured trials exceeding 1,000
signatures/second separately; an aggregate above 1,000 does not establish
that every run meets the threshold.

## Measured results

| Metric | Baseline | Optimized |
| --- | ---: | ---: |
| Aggregate signatures/second | 1,061.12 | 1,111.56 |
| Mean proving ms/signature | 0.942398 | 0.899637 |
| Seed medians above 1,000 signatures/second | 9/10 | 10/10 |
| Measured trials above 1,000 signatures/second | 27/30 | 30/30 |
| Verification median, ms/batch | 73.668 | 74.715 |
| Median process peak RSS, GiB | 3.317 | 3.333 |

Nine of ten paired seed medians improved. Seed 99 was 0.60% slower; the
optimized measured-trial range was 0.856335–0.988341 ms/signature. Proof payload
sizes were unchanged for every seed (2,439,588–2,447,332 bytes across the sweep).
This fixed sample supports the measured gain without establishing a universal
speedup or a latency guarantee.

Separate instrumented seed-42 profiles measured:

| Stage | Baseline ms/batch | Optimized ms/batch | Reduction |
| --- | ---: | ---: | ---: |
| Arithmetic binder | 154.306 | 128.755 | 16.56% |
| Binder prefix accumulation | 69.938 | 40.149 | 42.59% |
| wfbitz GKR forest | 190.933 | 176.655 | 7.48% |

The prefix row is included in the binder row. Unchanged stages also varied
between these profiles, so the uninstrumented ten-seed sweep determines the
whole-prover result.

All **80 sweep proofs** verified; every optimized proof matched its baseline
complete Debug digest, roots, input digest, and payload size. The complete
reported security terms and **129.343036-bit** work-normalized bound under the
existing convention were unchanged at target 128. Batches **1, 3, and 32** added
**24 verified proofs**, also matching baseline digests and security metadata.

The CPU was an AMD Ryzen 9 9950X3D. The optimized executable SHA-256 is
`49046c3668d9809f40a24b87779b9ff6baaa46d93b0e01b05d21c8298d4397a6`.
The artifact directory contains `REPORT.md` with all ten seed rows,
`results.json`, `profiles.json`, `small-batch-results.json`, test logs, raw
samples, and the exact changed-source hash manifest. The benchmark executables
include the current uncommitted Rust implementation; unrelated pre-existing
documentation changes were left intact.
