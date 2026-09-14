# Candidate regression fixes: final ARM confirmation

All eight targeted regressions have passing replacements in the isolated experiment. The final revision passes **32 single-thread and 3 ten-thread selected checks**, with **zero baseline retentions, zero inconclusive selections, and no retry needed**. All three follow-up requirements are complete: the tiny polynomial case passes, the exact measured signed-MAC executable passes instruction review, and ten-thread OOD passes on that same executable.

These are Apple M1 Max measurements using Rust 1.98.1 and `-C target-cpu=native`. They establish performance for the recorded workloads and this build. The candidates are not integrated into production dispatch; this is not an end-to-end prover speedup or an x86 performance claim.

## Timings for the original eight regressions

| Workload | Threads | Baseline µs | Replacement µs | Paired time ratio |
|---|---:|---:|---:|---:|
| Signed exact MAC, 9 limbs, 1,024 terms | 1 | 68.480 | 46.981 | 0.690× |
| Batch inversion, 128-bit modulus, mixed zeros, 1,024 elements | 1 | 21.104 | 18.058 | 0.858× |
| Batch inversion, 128-bit modulus, all zeros, 1,024 elements | 1 | 4.251 | 1.510 | 0.356× |
| Public polynomial dot, sparse 3×7 limbs, 1,024 terms | 1 | 3.566 | 3.011 | 0.847× |
| GF sumcheck grid, 1,024 rows | 1 | 53.270 | 51.386 | 0.965× |
| NTT, 256 points × 1 lane | 1 | 2.557 | 1.171 | 0.462× |
| OOD, 2¹⁶ points | 10 | 82.856 | 46.743 | 0.567× |
| OOD, 2¹⁸ points | 10 | 130.367 | 114.811 | 0.881× |

Baseline and replacement medians above come from fresh, matched measurements in the final run. Historical timings from earlier revisions are not mixed into these comparisons. Absolute times vary with inputs and host state. The paired ratio is the median of per-process candidate/baseline median ratios, so it need not equal the quotient of pooled medians. A ratio below one is faster.

## The three final acceptance requirements

1. **Tiny polynomial dot, 3×7 limbs, 16 terms, dense prefix:** baseline **89.4255 ns**, replacement **66.3210 ns**. Paired time ratio **0.7309**; median confidence upper bound **0.7638**, P95 upper bound **0.7730**, worst process median ratio **0.7644**. All are below 1.01. The same public-prefix implementation passes all six tested density classes at both 16 and 1,024 terms.
2. **Signed MAC timing behavior:** the final executable uses two `CSEL` mask constructions per signed-column kernel. Complete instruction/dataflow review finds no operand-sign-dependent branches or secret-derived memory addresses in the reviewed signed kernels. Remaining branches use public lengths and counters. See the [exact executable review](results/candidate-fixes-arm-03/arm-audit/REVIEW.md).
3. **Ten-thread OOD:** log10, log16 and log18 all pass the full gate. Both thread-count campaigns and the assembly review use executable SHA256 `4d16fca6e2611020cdaaf5d709e46dedf65af9889ed3414a3da620f897fe9b4d`.

## Implementation

| Area | Selected implementation and reason | Source |
|---|---|---|
| Exact signed MAC | Accumulate products and two's-complement corrections in signed wide columns; validate public capacity once and carry once at the end. Use `CtSelect` for sign masks. L4/L9 select `signed_columns`; L2 retains the existing faster candidate `fused_exact`, not the baseline. | [words.rs](src/campaign/candidates/words.rs) |
| Public batch inversion | Initialize configured zero outputs once, avoid prefix storage and inversion for all-zero input, compact raw prefix products, skip zero multiplications and unused end updates. The selected API remains explicitly variable-time for public inputs. | [prime.rs](src/campaign/candidates/prime.rs) |
| Public polynomial dot | Consume each dense/zero prefix row once; apply the fused sparse kernel to the remaining suffix and XOR its result into the prefix sum. No rereading dense prefixes, no assumption about unread rows, and no private-data dispatch. | [binary.rs](src/campaign/candidates/binary.rs) |
| GF grid | Fold into the final compacted prefix directly. Separate folding and accumulation to reduce register pressure, prepare multipliers once per pass, retain wide products and perform nine final reductions. | [grid.rs](src/campaign/candidates/grid.rs) |
| Tiny NTT | Incrementally update public twiddles through a stack prefix table in a breadth-first transform; keep existing half-width multiplication specializations. Larger transforms use the existing candidate schedule. | [composite.rs](src/campaign/candidates/composite.rs) |
| OOD | Use 1,024-element equality tiles. Run smaller domains serially; group four tiles per Rayon job for large domains, reducing scheduling and equality-table costs. | [composite.rs](src/campaign/candidates/composite.rs) |

The integer column bound leaves signed i128 headroom for both per-column sums and the final carry. Polynomial prefix/suffix ranges are disjoint and cover every term. Grid folding loads each full source block before storing its compacted output, preserving unread inputs. Independent source reviews found no remaining correctness blocker in these changes.

## Validation and coverage

- **39/39 tests pass** on frozen sources in native debug, native release, and generic ARM with AES-gated candidate selection disabled. The latter exercises the candidate fallback; it does not claim every dependency disables SIMD.
- **17/17 benchmark-harness tests pass.** [Hashed test records and logs](results/candidate-fixes-arm-03/correctness/results.json).
- Independent BigInt/BigUint, extended-Euclid, bit-convolution and bit-serial field oracles cover signed extrema, carries, cancellation, zero/mixed inversion inputs, sparse/dense transitions, nine grid outputs, compacted prefixes, NTT lanes/custom bases, and OOD Boolean/one-hot indexing at tile boundaries.
- The final signed and grid assembly, including the out-of-line folding helper, is retained with hashes. Instruction review is specific to this executable and compiler, not a universal constant-time certificate.
- Performance acceptance remains the unchanged **1% maximum slowdown** policy: five fresh processes, 32 paired samples, shuffled execution order, median and P95 confidence bounds, every process median, and allocation checks. Each inconclusive selected workload may receive exactly one five-process, 64-sample retry; none was required in the final revision.
- Coverage includes signed MAC L2/L4/L9 at 16/1,024 terms; inversion at 100/128-bit moduli with nonzero/mixed/zero inputs at 1,024 elements; all six public polynomial density classes at 16/1,024 terms; grid 16/1,024 rows; NTT log8/1 lane, log8/32 lanes and log12/8 lanes; OOD log10/log16/log18 at both thread counts. Small inversion batches were development/correctness cases, not part of this focused performance gate.

## Evidence and rejected attempts

The first confirmation's performance and numerical checks passed, but its signed masks compiled into operand-sign branches. That executable was [rejected for private arithmetic](results/candidate-fixes-arm-01/arm-audit/REVIEW.md). Revision 2 fixed the masks but [remained blocked on tiny dense-prefix polynomial latency after its one retry](results/candidate-fixes-arm-02/gate.txt). Revision 3 changes the polynomial kernel and is independently frozen and confirmed. Earlier results and rejected diagnostic kernels remain available; they are not counted as accepted speedups.

- [Final single-thread measurements](results/candidate-fixes-arm-03/measurements.md) and [gate](results/candidate-fixes-arm-03/gate.txt).
- [Final ten-thread measurements](results/candidate-fixes-arm-10cores-03/measurements.md) and [gate](results/candidate-fixes-arm-10cores-03/gate.txt).
- [All 35 selected comparisons](candidate_fix_decisions.json), including exact medians, ratios, bounds and allocations.
- Frozen manifests: [one thread](candidate_fix_cases_v3_arm_1.json), [ten threads](candidate_fix_cases_v3_arm_10.json).
- [Source and executable archive](results/candidate-fixes-arm-03/source-and-binary.tar.gz), with [archive hashes](results/candidate-fixes-arm-03/archive.json). All 527 source files and the executable were checked before and after archiving.

## Replay

From the repository root, verify the recorded results:

```sh
python3 experiments/field-regressions/gate.py experiments/field-regressions/results/candidate-fixes-arm-03
python3 experiments/field-regressions/gate.py experiments/field-regressions/results/candidate-fixes-arm-10cores-03
python3 -m unittest discover -s experiments/field-regressions -p 'test_*.py'
```

For a new ARM confirmation with cached dependencies, use a new output directory, then reuse the resulting executable for ten-thread OOD:

```sh
python3 experiments/field-regressions/run.py --suite optimization \
  --manifest experiments/field-regressions/candidate_fix_cases_v3_arm_1.json \
  --out experiments/field-regressions/results/my-candidate-fix-arm --threads 1
python3 experiments/field-regressions/run_frozen.py \
  --from-run experiments/field-regressions/results/my-candidate-fix-arm \
  --manifest experiments/field-regressions/candidate_fix_cases_v3_arm_10.json \
  --out experiments/field-regressions/results/my-candidate-fix-arm-10 --threads 10 \
  --families opt_ood
```

For an exact-binary replay, extract the archive into a temporary directory and pass `--binary <directory>/snapshot/field-regressions` to `run_frozen.py` with `--from-run` pointing at the recorded final run. The runner verifies the binary hash.
