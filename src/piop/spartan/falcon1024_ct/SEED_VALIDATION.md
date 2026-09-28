# Falcon throughput validation across additional seeds, 2026-09-27

The [subsequent nonce investigation](GRINDING_VARIANCE.md) directly confirms
the slow seed's extra PCS work: 513.7 million nonce candidates versus 105.3
million for the fastest seed, with identical challenge counts and security.

The broader validation **does not establish consistent 1,000 signatures/sec**.
Nine of twelve new seed medians exceeded that target. Aggregate throughput over
all 36 optimized measured trials was **1,025.9 signatures/sec**, but three seed
medians and eight individual trials fell below 1,000/sec.

The optimized prover was faster than the saved baseline on **11 of 12** seed
medians, with a median paired time reduction of **12.1%**. The one regression
and every slower trial are retained below. This extends the earlier successful
three-seed measurements in [SIMD_THROUGHPUT.md](SIMD_THROUGHPUT.md).

## Conditions and validation

Predeclared seeds: `0, 1, 2, 3, 7, 17, 99, 123, 256, 1024, 2026, 65537`.
Each produces a batch of 1,024 distinct original Falcon-1024 signatures.
AMD Ryzen 9 9950X3D, 16 Rayon workers, native release, target 128, one warmup and
three measured trials per executable/seed. Baseline and optimized order
alternates between seeds. All runs are sequential; this task launched no builds,
tests or other timed workloads concurrently with the sweep.

The exact saved executables from the previous comparison are reused, and the
optimized binary's recorded source hashes match the current production source.
No Rust code, verifier, grinding difficulty or security parameter changed.
Full prover time includes witness generation, commitments, SHAKE, HashToPoint,
native ring proof, all sumchecks, grinding and the shared opening. Input
creation/validation, reusable preparation, proof verification and Debug hashing
remain excluded identically for both binaries. These are batch-amortized times,
not single-signature latency.

All **96 primary benchmark proofs verified**, including the warmups. Every pair
has the same input digest, commitment roots, complete proof Debug digest, stored
payload and reported security bound. Repeated proofs for each seed also match.
The reported work-normalized bound remains **129.375910 bits** under the existing
computational grinding/Fiat–Shamir convention; this validation does not change
that convention or constitute a new soundness proof.

## All primary measurements

Times are milliseconds per signature. The trial range excludes the warmup.
Negative reduction means the optimized median was slower in that comparison.

| Seed | Baseline median | Optimized median | Optimized signatures/sec | Less prover time | Optimized trial range |
| --- | ---: | ---: | ---: | ---: | ---: |
| 0 | 1.059391 | 1.024394 | 976.2 | 3.3% | 0.931356–1.154635 |
| 1 | 1.025135 | 1.046113 | 955.9 | -2.0% | 0.990018–1.061226 |
| 2 | 1.077114 | 0.943229 | 1,060.2 | 12.4% | 0.942901–0.962466 |
| 3 | 1.072364 | 0.946159 | 1,056.9 | 11.8% | 0.930049–0.951341 |
| 7 | 1.221832 | 1.050039 | 952.3 | 14.1% | 1.049500–1.056250 |
| 17 | 1.121018 | 0.980864 | 1,019.5 | 12.5% | 0.949073–0.999745 |
| 99 | 1.086655 | 0.947258 | 1,055.7 | 12.8% | 0.943611–0.950118 |
| 123 | 1.111053 | 0.968246 | 1,032.8 | 12.9% | 0.952202–1.048446 |
| 256 | 1.025805 | 0.944521 | 1,058.7 | 7.9% | 0.916034–0.969237 |
| 1024 | 1.064445 | 0.941951 | 1,061.6 | 11.5% | 0.941339–0.962499 |
| 2026 | 1.105287 | 0.925801 | 1,080.1 | 16.2% | 0.918159–0.939575 |
| 65537 | 1.013279 | 0.937643 | 1,066.5 | 7.5% | 0.924823–0.990974 |

- Seed median range: **0.925801–1.050039 ms/signature**, or **952.3–1,080.1/sec**.
- Median of the twelve seed medians: **0.946708 ms/signature**.
- Combined throughput: **1,025.9/sec**, computed as total signatures divided by
  total prover time across all 36 measured optimized trials.
- Individual measured trial range: **0.916034–1.154635 ms/signature**, or
  approximately **866–1,092/sec**; **28 of 36** trials were below 1 ms/signature.
- Median within-seed sample coefficient of variation: **1.95%**. There are only
  three primary measured trials per seed, so these describe the observations
  rather than a population guarantee.

Repeated trials use the same proofs and therefore the same grinding nonces.
Their within-seed variation is execution variability. Across seeds, transcript
challenges and grinding work can also change. For example, seed 0 varied from
0.931356 to 1.154635 ms/signature, while seed 7 stayed between 1.049500 and
1.056250 in all three original trials.

## Five-trial repeats of the two slowest seed medians

After completing the predeclared sweep, seeds 7 and 1 were selected because they
had the two largest optimized medians. Each repeat used one warmup and five
measured trials with the same binary, input and settings. Original observations
above remain the primary results.

| Seed | Original median, ms/signature | Repeat median, ms/signature | Repeat signatures/sec | Repeat trial range, ms/signature |
| --- | ---: | ---: | ---: | ---: |
| 7 | 1.050039 | 1.053647 | 949.1 | 1.049782–1.070071 |
| 1 | 1.046113 | 0.931136 | 1,074.0 | 0.924248–0.944951 |

Seed 1's original shortfall did not persist. Seed 7's did: all eight measured
trials across its original and repeated runs exceeded 1 ms/signature. Both
execution variability and seed-dependent proof cost matter for consistency.

## Diagnostic profiles of the fastest and slowest seeds

Additional one-trial cold profiles used seeds 2026 and 7, selected from their
primary medians. These instrumented measurements have no warmup and are separate
from the throughput results above. Times below are milliseconds per signature.

| Stage | Seed 2026 | Seed 7 |
| --- | ---: | ---: |
| Full prover | 1.017231 | 1.140673 |
| Arithmetic binder | 0.154432 | 0.153661 |
| Integer-to-binary bridge | 0.202373 | 0.203468 |
| Joint binary sumcheck | 0.052901 | 0.055481 |
| Shared PCS opening | 0.120434 | 0.243704 |
| PCS grinding (nested within opening) | 0.031538 | 0.150877 |
| All grinding, excluding nested duplicates | 0.074333 | 0.195719 |

PCS grinding accounts for approximately **0.119 ms/signature** of the
**0.123 ms/signature** difference between these profiles. This points to
seed-dependent nonce-search work as the main cause of the persistent slow case.
It does not imply that every timing outlier has that cause; seed 1's repeat and
seed 0's original spread demonstrate execution variability as well. Nested rows
must not be added to their parent stages.

The twelve repeat proofs and two diagnostic proofs also verified and matched
their original baseline proof/input digests, roots, payloads and security
reports: **110 verified proofs in this validation task**. No security settings
were reduced to meet the throughput target.

The implementation is faster on most tested inputs and exceeds 1,000/sec in
aggregate, but it needs more performance margin to sustain that rate across
these seeds. Further work should account for PCS grinding cost as well as the
remaining binder and bridge cost.

## Reproduction

All original observations are retained in
`bench_results/falcon-seed-validation-20260927/`. `plan.json` records the seed
list, order, executable hashes and source hashes before the sweep. Per-case
files include commands, all trial timings, proof digests and peak RSS.
`results.json` and `analysis.txt` contain the complete checked comparison.
`followup-plan.json` records the repeat/profile selection; the corresponding
results, full phase timings and verification checks are in
`followup-results.json` and `followup-analysis.txt`.

```sh
python3 bench_results/falcon-seed-validation-20260927/sweep.py
python3 bench_results/falcon-seed-validation-20260927/analyze.py
python3 bench_results/falcon-seed-validation-20260927/followup.py
python3 bench_results/falcon-seed-validation-20260927/analyze_followup.py
```

The saved binaries are the baseline at commit `8a924fac` and the optimized binary
with SHA-256 `b6da10c3bbcef854b4b1dee812b202049a644221178dee9832fd5199d2a779cb`.
The sweep runner writes its case files; copy the runner and plan to a fresh
artifact directory to preserve prior evidence when rerunning.
