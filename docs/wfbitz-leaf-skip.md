# Univariate skips for the leaf sumcheck

Two explicit protocols replace the initial row-variable rounds of GKR
level zero. The other GKR levels and the subsequent inner-product
sumcheck, ring switch and PCS use their existing protocols.

| Public variant | Row variables packed | Polynomial degree | Coefficients sent | Extra transcript bytes |
|---|---:|---:|---:|---:|
| `LeafProtocol::Skip3` | 3 | 14 | 15 | 144 |
| `LeafProtocol::Skip4` | 4 | 30 | 31 | 368 |

Select the same variant on both `BitZProver` and `BitZVerifier` with
`.with_leaf_protocol(...)`. The default remains `LeafProtocol::Sequential`,
including its existing transcript compatibility. The byte counts above
compare the packed message with the ordinary rounds it replaces.

## Protocol

Let `k` be three or four and index the skipped row block by
`a = 0..2^k-1`, with the first GKR coordinate being its most significant
bit. Let `L_a(T)` be the Lagrange basis on those distinct polynomial-basis
encodings in GF(2^128). The nodes are not assumed to form a subfield.
Interpolate each child separately:

```text
E_hat(T,b) = sum_a L_a(T) E(a,b)
O_hat(T,b) = sum_a L_a(T) O(a,b)
Q(T)      = sum_b eq(z_suffix,b) E_hat(T,b) O_hat(T,b)
```

The prover absorbs `wfbitz/leaf-univariate-skip3/v1` or
`wfbitz/leaf-univariate-skip4/v1`, sends all 15 or 31 coefficients of `Q`,
and receives one full-field challenge `rho`. The
verifier checks `claim = sum_a eq(z_prefix,a) Q(node_a)` before continuing
from `Q(rho)` with the ordinary suffix rounds. There is no skipped-prefix
equality multiplier after this step. The fixed message length enforces
degree at most 14 or 30, giving a skipped-round consistency error bounded
by that degree divided by `|GF(2^128)|`, before accounting for the remaining
protocol. The four-variable variant retains its earlier domain and wire format.

At the terminal claim, the row weight at `(child,a,y)` is
`(image[row]-1) * eq(child,r_child) * L_a(rho) * eq(y,r_remaining)`.
Column weights retain their ordinary equality form. The prefix weights
sum to one, so the constant leaf contribution is still one; subtract it
before handing the factored linear claim to the existing opening.

## Computation

Both variants start with the cross sums
`C[a,b] = sum_suffix eq_suffix * E_a * O_b`. Contracting these with cached
products `L_a L_b` constructs the packed polynomial. This computation
keeps the original bit structure available until the challenge arrives.

Skip3 reuses the existing 8-by-8 cross-sum pass and builds one 256-entry
subset table per child and remaining row. Its weights are the eight
Lagrange weights rather than three multilinear equality weights. It then
uses the ordinary JIT continuation: round 4 reads the tables, and binding
round 4 shares a pass with computing round 5. The dense engine resumes
with round 5's challenge pending. The continuation starts with equality
factor one and records only the suffix challenges.

Skip4 accumulates a 16-by-16 matrix through sixteen 256-entry pattern
buckets. Its direct-scatter implementation extracts each column's eight
nibbles once and loads the column equality weight once for all sixteen
bucket updates. It avoids the reference pass's temporary pair-index
arrays. Marginal contraction consumes each 4-by-4 corner block directly,
without storing a full temporary matrix of bit-pair marginals. Portable
and NEON implementations compute the same matrix as the reference pass.

After the challenge, each Skip4 factor is an affine weighted sum of 16
bits. It supports two subset-table layouts:

| Layout | Components per factor | Entries per factor | Populated table data at n=28 |
|---|---:|---:|---:|
| Byte | Two 256-entry tables | 512 | 32 MiB |
| Nibble | Four 16-entry tables | 64 | 4 MiB |

The byte components select even and odd corners to match the existing
nibble-row storage. The nibble components select even-low, even-high,
odd-low and odd-high corners. The affine constant occurs in the first
component only. Nibble tables require more lookups and bucket updates per
column, in exchange for smaller construction and contraction costs.

The preceding GKR level's table allocation can be reused. Consequently,
the populated sizes above do not imply an equivalent reduction in reserved
capacity or process peak memory. Table construction can also be fused with
round 5: each task fills four disjoint position tables, uses them immediately,
and retains their values for the following binding pass.

Round 5 uses three groups of component buckets: endpoint E, E-low
infinity and E-high infinity. Byte layout has six 256-entry buckets;
nibble layout has twelve 16-entry buckets. Each column contributes an
unreduced `eq_c * O_endpoint` or `eq_c * delta_O`, shared across the E
components. The buckets are contracted with the E tables after the row scan.

Binding round 5 and computing round 6 share a lookup pass. The portable
and NEON kernels specialize the same lookup, fold, optional column
weighting, arena-write and round-sum operations for either table layout.
Rows with at least four times the selected table length in columns
pre-scale the tables: 2,048 columns for byte tables or 256 for nibble tables.
Narrower rows fold in registers. When all column equality weights are
invertible, the arena retains those weights in E. The existing dense GKR
engine resumes with round 6's challenge pending and removes the weights
before its column rounds. Tiny `t=6` forests retain the separate bind path.

The Skip4 arithmetic options preserve its transcript byte for byte.
Phase timings expose cross sums, polynomial construction, table preparation
and round 5 (separately or fused), the fold/round 6 pass and the dense tail.

## Arithmetic options

These environment settings select Skip4 prover arithmetic only. They are
read once per process, so comparisons must use separate processes.

| Variable | Current default | Alternative |
|---|---|---|
| `WFBITZ_LEAF_CROSS` | `fast` | `reference` restores pair-index generation |
| `WFBITZ_LEAF_TABLES` | `byte` | `nibble` selects four smaller tables |
| `WFBITZ_LEAF_TABLE_FUSION` | `1` | `0` separates table construction from round 5 |
| `WFBITZ_LEAF_TABLE_REUSE` | `1` | `0` allocates new Skip4 position tables |

In the implementation, only the listed alternative strings change the
defaults. Byte won the level-zero comparisons at n=28 and n=30. Nibble
showed promise on the narrower n=24 one-thread case, but the size/thread
sweep was too noisy to select it automatically. Protocol selection remains
explicit through `LeafProtocol` or the benchmark's `--leaf-protocol` argument.

## Evaluation and extrapolation experiment

An experimental `cfg(test)` implementation computes the same degree-30
Skip4 polynomial without constructing the cross matrix. It evaluates
each child interpolant separately at 31 distinct polynomial-basis nodes
`0..30`, multiplies the factor evaluations, and accumulates the suffix
sum at each node. Newton divided differences then recover all 31 monomial
coefficients. The interpolation denominators are nonzero field differences.

This is an exact evaluation/interpolation alternative for `Q`. A compressed
multivariate product grid is not substituted for the separately interpolated
factors. Parity tests compare its coefficients with `polynomial_from_cross`.

The prototype performs 31 weighted field products per column and builds
31 sets of subset tables per row; the cross method primarily performs
sixteen bucket XOR updates per column. At `t=12, s=8`, one thread, the
prototype took 18.038 ms versus 0.605 ms for the direct cross pass plus
polynomial construction, about 30 times as long. Both produced the same
coefficients. This focused diagnostic used the release test build with
LTO disabled; full-prover comparisons below use the normal release build.
The experiment is retained in tests and is not selected by production
proof generation.

## Run

```sh
RUSTFLAGS='-C target-cpu=native' cargo build --locked --release --features bitz-parity --example wfbitz_bench
RAYON_NUM_THREADS=1 target/release/examples/wfbitz_bench 28 --reps 5 --ladder custom:1:4 --leaf-protocol sequential
RAYON_NUM_THREADS=1 target/release/examples/wfbitz_bench 28 --reps 5 --ladder custom:1:4 --leaf-protocol skip3
RAYON_NUM_THREADS=1 target/release/examples/wfbitz_bench 28 --reps 5 --ladder custom:1:4 --leaf-protocol skip4
WFBITZ_LEAF_TABLES=nibble RAYON_NUM_THREADS=1 target/release/examples/wfbitz_bench 28 --reps 5 --ladder custom:1:4 --leaf-protocol skip4
```

The sequential baseline retains the current one-pass bit-round optimization.
The benchmark verifies every measured proof and reports commitment separately.

The ignored `leaf_cross_cost_probe` test compares matrix construction plus
polynomial conversion against the evaluation experiment. Select
`WFBITZ_CROSS_PROBE=all|reference|fast|evaluation`, with dimensions controlled
by `WFBITZ_CROSS_T` and `WFBITZ_CROSS_S`, and repetitions by
`WFBITZ_CROSS_REPS`. Defaults are `all`, `t=12`, `s=8`, and three measured
repetitions after one warm-up. The diagnostic also reports summed worker
time for clearing, pattern extraction/scattering and contraction; those
phase sums are not parallel wall time.

```sh
WFBITZ_CROSS_T=16 WFBITZ_CROSS_S=12 WFBITZ_CROSS_PROBE=all RAYON_NUM_THREADS=1 \
  RUSTFLAGS='-C target-cpu=native' cargo test --locked --release --features bitz-parity \
  --lib leaf_cross_cost_probe -- --ignored --nocapture --test-threads=1
```

## Initial local measurement (2026-09-27)

On an M1 Max at n=28, one thread, the stable comparison measured
784.2 ms for the existing one-pass prover and 794.5 ms for Skip4 (1.3% slower).
Level zero itself increased from 119.1 to 133.4 ms. A noisier ten-thread
comparison measured 163.9 versus 169.7 ms. The sequential protocol remains
the default; Skip4 is an explicit experimental replacement.

Full results, raw logs and source hashes are in
`bench_results/wfbitz-skip4-confirm-20260927-155747-ke25xzt0/REPORT.md`.

## Optimized continuation measurement (2026-09-27)

After the fused transition, earlier weight carry, bucketed round 5 and
specialized split-byte kernels, a fresh confirmation run measured:

| Threads | Previous Skip4 | Optimized Skip4 | One-pass baseline |
|---|---:|---:|---:|
| 1 | 784.1 ms | 777.5 ms | 773.5 ms |
| 10 | 151.7 ms | 152.7 ms | 155.8 ms |

Level zero improved from 131.3 to 123.7 ms on one thread and 20.8 to
19.7 ms on ten threads. End-to-end ten-thread results varied between
campaigns, so they do not establish a speedup over previous Skip4.
Single-thread total proving improved by 0.8% over previous Skip4 but
remains 0.5% slower than the one-pass baseline. The default stays unchanged.

All 42 selected wfbitz tests passed, including exact transcript parity
with the unfused schedule, and all 120 measured proofs verified. Results,
both campaigns, source snapshots and logs are in
`bench_results/wfbitz-skip4-opt-20260927-162123/REPORT.md`.

## Cross-pass and table-layout measurement (2026-09-27)

The remaining candidates are implemented: direct cross scatter, reusable
table storage, fused construction, Skip3, nibble tables, and the test-only
evaluation/interpolation alternative. Skip4 selects direct cross, byte
tables, reuse and fused construction by default.

The n=28 confirmation used five timed proofs per process and three
processes per mode, with reordered variants. The medians of the process
medians were:

| Threads | Existing one-pass | Previous Skip4 (1–4) | Current Skip4 | Previous/current level zero |
|---|---:|---:|---:|---:|
| 1 | 773.75 ms | 777.51 ms | 764.49 ms | 123.45 / 111.33 ms |
| 10 | 153.73 ms | 153.90 ms | 152.79 ms | 19.51 / 17.92 ms |

At one thread, current Skip4 reduces total proving time by 1.67% versus
previous Skip4 and 1.20% versus the existing one-pass path. Its level-zero
time drops 9.82%. At n=30, one thread, existing one-pass/current Skip4
measured 2,999.30/2,934.89 ms overall and 446.58/399.54 ms for level zero.
Ten-thread overall timings varied substantially between runs; the small
overall differences there are not established speedups.

Direct cross is the clear isolated improvement: its one-thread n=28
phase took 42.70–43.52 ms in the screen versus 55.50–56.96 ms for the
reference pass. Separate measurements did not isolate a speedup from
allocation reuse or construction fusion. Nibble saves preparation time
but adds lookup and bucket work; Skip3 did not show a clear advantage
over the existing one-pass path.

All 51 selected wfbitz tests and all 324 timed proof verifications passed.
Raw timings, outliers, source hashes, snapshots, measured binaries,
ablation and size sweeps, and reproduction scripts are in
`bench_results/wfbitz-skip-all-20260927-164329/REPORT.md`.
