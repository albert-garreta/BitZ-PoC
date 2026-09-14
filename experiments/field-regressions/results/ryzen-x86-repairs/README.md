# Ryzen x86 arithmetic repairs

Validated on the local **AMD Ryzen 9 9950X3D**, Rust 1.98.1, `-C target-cpu=native`, after updating `fast-arithmetic` to `2d67dc6` and preserving the x86 work. Changes remain in the benchmark experiment; production dispatch is unchanged.

**Final policy: 241 confirmed candidate choices and 61 production retentions across 302 operation/size/CPU-placement cases.** Every accepted candidate has a demonstrably faster median (95% CI upper bound below 1), passes the unchanged 1% median/P95/every-process limits and allocation checks against production, and clears its predeclared strongest alternative. Retention uses the actual production kernel and is not a speedup.

## Representative confirmed results

Speedups are baseline time divided by candidate time, using paired process medians. These are operation-level measurements on this host.

| Operation | CPU 0, 96 MiB L3 | CPU 8, 32 MiB L3 |
|---|---:|---:|
| GF products, 1,024 items | 3.65× | 3.65× |
| Single-pair GF round, 1,024 weights | 3.44× | 3.44× |
| Two-pair GF round, 1,024 weights | 3.29× | 3.29× |
| Two-limb wrapping MAC, 1,024 terms | 1.49× | 1.52× |
| Prime dot, q128, 1,024 terms | 1.04× | 1.04× |
| NTT, log8 / 32 lanes | 1.95× | 1.96× |

## Repairs and retained paths

- Packed GF products use fixed chunks and masked SIMD tails; weighted rounds keep products in SIMD registers and share traversal and final reductions across two pairs.
- Two-limb wrapping MACs accumulate low products and high cross products separately. Short batches retain production. One kernel must pass both random and carry-heavy fixtures for each public size and CPU placement.
- The borrowed prime-dot adapter removes materialization allocations. The original allocating adapter remains a measured control.
- The private Flock scalar substitute uses five-CLMUL Karatsuba/Barrett reduction. Its NTT sources remain byte-identical to production. A separate tiled schedule wins on small NTT shapes; it is rejected for larger shapes where it loses.
- The 33-term one-accumulator proposal was 4.5% slower in one retry process. Four accumulators passed fresh checks on both fixtures, taking about 9% less time than production.
- The scalar-substitution NTT passed fresh confirmation for log15/32 lanes on both single-core placements (about 1.4% and 1.2% less time), and log18/32 lanes with 16 threads (about 1.6% less time).
- The log17/32-lane substitution with 32 threads remains inconclusive on P95 after its prescribed retry; production is retained. CPU 0 also retains production for log17/32 lanes because its substitution passed the 1% gate but did not demonstrate a speedup.

For the integer MAC, with $a_i=a_{i,0}+2^{64}a_{i,1}$ and likewise for $b_i$:

$$\sum_i a_i b_i \equiv \sum_i a_{i,0}b_{i,0}+2^{64}\sum_i(a_{i,0}b_{i,1}+a_{i,1}b_{i,0})\pmod{2^{128}}.$$

The diagonal accumulator wraps modulo $2^{128}$; the cross-product accumulators wrap modulo $2^{64}$. Dispatch depends on public sizes and instruction support. Fixture labels and input values do not select kernels.

## Validation and acceptance

- Development covered 1,524 variant rows across CPU 0, CPU 8, CPUs 0–7, CPUs 8–15, CPUs 0–15, and CPUs 0–31. All arithmetic correctness checks passed.
- Primary confirmation used five fresh processes and 32 paired samples per case. Inconclusive cases received at most the one prescribed five-process/64-sample retry. Six replacement cases then received fresh confirmation; five were accepted.
- All 302 final choices have complete confirmation evidence. The final policy retains production when an alternative fails, remains uncertain, or does not demonstrate an improvement. All retained optimizations pass the original production and strongest-alternative limits.
- Allocation auditing takes the maximum for one call across 64 untimed calls. The periodic 1,520-byte Rayon queue allocation was observed in both x86 NTT implementations; it is included in both, with no exemption or relaxed limit.
- Correctness checks cover zero/full-width/random data, SIMD boundaries and tails, packed-buffer offsets and sentinels, integer cutoff neighbors and carry-heavy inputs, and complete NTT outputs with equal reset copies.
- The frozen development assembly contains masked loads/stores for packed-product tails and no integer division in the inspected packed-product, weighted-round, and split-cross integer kernels. This is a bounded source/assembly review.
- The 36 benchmark-policy tests and four tests for consistent integer fixture choices pass. Native release builds and `git diff --check` pass. Production source, dependencies, and dispatch remain unchanged by these repairs.

The original proposal gates contain rejected or inconclusive selections and remain unchanged in the archive. The final accepted policy is recorded separately in [verified-choices.json](verified-choices.json), including the original verdicts and refinement verdicts. Rejected alternatives can still be slower; they are not part of the final policy. These results do not establish an end-to-end prover speedup or performance outside the measured host and workloads.

## Evidence and replay

- [All 302 choices](verified-choices.md), [machine-readable choices and original gate issues](verified-choices.json), and [raw-evidence hashes](evidence-sha256.json).
- [Exact confirmation executables, source archives, manifests, and raw measurements](frozen-runs.tar.xz), with [archive size and SHA-256](archive.json).
- [Fresh-replay selections](confirmed-selections/) include every used seed, so the runner rejects reuse of development or confirmation seeds.
- [Choice finalizer](finalize.py), [public-shape freezing rule](freeze-x86-shapes.py), [focused-confirmation driver](refine.py), and [fixture-policy tests](test_shape_policy.py).

From the repository root, replay the final policy with a new output directory and fresh seeds:

```sh
python3 experiments/field-regressions/ryzen.py --scope regressions \
  --streaming --scaling --phase confirm \
  --exploration experiments/field-regressions/results/ryzen-x86-repairs/confirmed-selections \
  --out /tmp/x86-confirmed-policy-replay --seed-offset 97182817
```

The runner checks source hashes, host, flags, CPU placement, and thread count before confirmation. Full campaign methodology is in [X86.md](../../X86.md).

## Earlier attempts

The out-of-line and then inlined scalar remainder in seven-element packed products was unstable in earlier confirmation attempts (`confirm01` and `confirm02`). Masked SIMD tails replaced it.

The `explore04` / partial `confirm03` campaign used the older source and three-call allocation audit. Confirmation was stopped when upstream `2d67dc6` arrived; its metadata is marked aborted and its raw data remain available under `/tmp`. It is superseded for current selections.

The current evidence comes from `/tmp/bitz-x86-repair-explore05`, `/tmp/bitz-x86-repair-confirm04`, and `/tmp/bitz-x86-repair-refine02`. The accepted policy is more conservative than the original proposals: the one candidate with no demonstrated median improvement is explicitly replaced by production, without rewriting its passing original gate result.
