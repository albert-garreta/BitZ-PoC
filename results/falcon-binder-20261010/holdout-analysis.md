# Falcon binder v2 holdout

All 120 processes verified. Falcon-512, batch 1024, security 128; fresh seeds 45–49; 1/4/8/16 workers on AMD Ryzen 9 9950X3D. Each process has one warmup and five measured trials. Timing uses one measured-trial median per process, paired by seed. Five seed pairs are the sampling units. Intervals below are descriptive 95% exact paired bootstrap intervals.

| Policy | Workers | Total prover reduction [95% CI] | Verifier change | Lifetime peak RSS change |
|:--|--:|--:|--:|--:|
| arithmetic word | 1 | 35.18% [34.83, 35.47] | +0.65% | -12.87% |
| arithmetic word | 4 | 40.72% [40.17, 41.12] | -39.50% | -11.98% |
| arithmetic word | 8 | 22.37% [21.65, 22.97] | +3.10% | -0.42% |
| arithmetic word | 16 | 33.66% [33.24, 34.07] | -46.48% | -11.72% |
| full replay-on | 1 | 0.57% [0.41, 0.70] | -0.76% | -0.02% |
| full replay-on | 4 | 5.82% [5.56, 6.13] | -0.32% | -3.12% |
| full replay-on | 8 | 5.84% [5.59, 6.07] | -0.14% | +0.05% |
| full replay-on | 16 | 3.63% [2.96, 4.44] | -0.15% | +0.24% |

Positive reductions mean faster proving; positive verifier/RSS changes mean more cost. All baseline comparisons include the v2 commit-stage fix as well as the selected binder policy.

Word verification costs increase at one worker by 0.65% (ratio CI 1.0020–1.0110), about 0.289 ms, and at eight workers by 3.10% (1.0193–1.0442), about 0.624 ms. These modest costs are reproducible across this holdout; universal verifier nonregression is unsupported. At 4/16 workers verification is substantially faster than the baseline.

The complete word proof is 360 bytes larger for seed 45 (+0.137%) and 200 bytes larger for seed 49 (+0.076%); it is smaller for the other three seeds. The mean change is −664 bytes, but proof-size nonregression does not hold for every fixture. Sizes are identical across worker counts for each seed.

The previous eight-worker word-memory regression is resolved in the observed lifetime peaks: ratio 0.9958 [0.9935, 0.9986]. Full replay RSS ratios at 1/8/16 workers have intervals crossing parity; their small deltas cannot establish strict memory nonregression. Full four-worker lifetime RSS improves by 3.12% [2.27%, 3.92%].

RSS is the maximum sampled process high-water mark, including warmup, fixture construction, allocator retention, and proof-digest formatting. The runner originally takes the median sampled high-water mark, so its RSS summary may differ. Full one-worker baseline RSS grows from roughly 1.66 GiB after sample one to 2.24 GiB after sample five; these measurements do not establish a steady-state memory bound or diagnose a leak.

| Workers | All bit kernels: total reduction | Replay alone: reduction versus r011 | Word: reduction versus r111 |
|--:|--:|--:|--:|
| 1 | 4.05% | 2.45% | 32.44% |
| 4 | 21.75% | 4.31% | 24.25% |
| 8 | 5.24% | 2.81% | 18.08% |
| 16 | 26.68% | 1.53% | 9.51% |

At one worker, all-bit-kernel prove-only time is effectively unchanged versus baseline; its 4.06% total gain comes from faster commitment. Full one-worker replay was already enabled in the baseline; prove-only time grows 0.19%, while faster commitment gives the 0.57% total gain.

Recommendation: enable the tested arithmetic kernels and full replay for the demonstrated prover gains, with the existing public shape guards. Word supports a prover-focused default only with the explicit verifier/payload tradeoffs above; retain opt-in status if strict verifier and complete-proof-size nonregression are requirements. Select the word protocol from public N=512, batch=1024, security=128, independently of worker count. V2 two-worker behavior still requires the separate production-default validation.

All identities are stable across warmup/measured trials. Bit/full proofs match across implementations, kernel choices, and worker counts for each seed. Word proofs have their separate protocol identity, with matching input and source commitments. These functional checks do not replace protocol soundness review.

The JSON report contains paired seed ratios, exact-bootstrap intervals, conservative paired-log Student-t sensitivity intervals, lifetime RSS statistics, proof sizes, and binary hashes. Five seeds on one host limit generalization; intervals are descriptive and are not adjusted for multiple comparisons.
