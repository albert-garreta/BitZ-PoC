# GKR sumcheck optimization qualification

**Adoption update:** the user approved merging the measured implementation
from `db82a93b` after reviewing the gains and regressions below. Coefficient
recovery and selective closing reductions are now enabled by default on
`gkr-optimizations`. Set `F2Z_GKR_RECOVER=0` or `F2Z_GKR_DIRECT_CLOSE=0` to
disable either path; controls are read once per process. This adoption decision
does not change the benchmark results or turn inconclusive gates into passes.

This campaign compares proving code with `4c03f2c190ccb1e54ef1e856e5330fdeb1651f5e`.
The earlier ownership and compact-coefficient optimizations are already in that
baseline. Their historical speedups are not gains from this campaign.

## Acceptance policy

The minimum improvement is 2% in warmed GKR on 1,024 SHA compressions plus
P-256, split outer mode, target 100, ten threads. It does not apply to total
proving. Whole-prover first-proof and warmed timings must not regress across
the covered workloads. First-proof GKR is also checked separately. Setup is
excluded. An interval crossing a gate is inconclusive, not a pass.

Exploration uses six paired blocks and seed 17. Confirmation uses fresh,
fixed 24-block campaigns, five warmed proofs per process, and seeds 31/47
where the fixture supports them. `scripts/qualify_gkr.py` requires the full
matrix and refuses incomplete or exploratory results.

The baseline and candidate use matching features, compiler flags, CPU affinity,
and measurement adapters. Binaries and their hashes are retained with each
campaign. Latency measurements exclude allocation instrumentation. Proof bytes
and transcript continuation must match, and every timed proof is verified.

## Candidate implementations

The existing prover already factors the current equality coordinate out of the
sumcheck polynomial. For its quadratic cofactor,

\[
P_j(X)=\operatorname{eq}_1(X,q_j)(a_0+a_1X+a_2X^2),
\qquad S_j=a_0+q_j(a_1+a_2).
\]

The new recovery experiment computes `a0` and `a2` in eligible bit-driven
kernels and reconstructs `a1` from the running claim. The claim includes the
analytic padding coefficient. With real-group constant coefficient `a0_real`
and analytic padding `C A_j`, the implemented reconstruction is

\[
a_1=(S_j-a_{0,\mathrm{real}}-C A_j)q_j^{-1}-a_2.
\]

Only the public coordinate is inverted. Zero coordinates or fields without
the optional inverse hook retain the original kernel. Two coordinate inverses are batched;
claim tracking stops after the last eligible round. This changes prover work,
not the proof encoding or verifier.

The closing-reduction experiment aggregates only coefficients needed that
round, uses a serial loop for at most 1,024 groups, and reduces chunk results
directly for larger group counts. It removes the temporary vector of chunk
partial sums. The first round still computes all coefficients for its claim.

The archived experiments use opt-in controls:
`F2Z_GKR_RECOVER=1` and `F2Z_GKR_DIRECT_CLOSE=1`.

## Rejected experiments

These are exploratory six-block results on the ten-thread primary fixture;
they are not final qualification. Ratios are candidate/baseline, with 95%
paired bootstrap intervals. Their baseline is the stated intermediate
candidate, so the numbers must not be added to earlier speedups.

| Experiment | Comparison | GKR before → after (ms) | GKR ratio interval | Decision |
| --- | --- | --- | --- | --- |
| Allocation-only closing reduction | Same binary, recovery off | 35.67 → 35.95 | [1.0115, 1.0347] | Replaced with selective-coefficient reduction |
| Three-round grid, revised fused fold/contraction | Recovery + selective closing | 35.51 → 40.37 | [1.1272, 1.1956] | Removed |
| Factored suffix, revised 1,024-entry tiles | Recovery + selective closing | 35.24 → 36.39 | [1.0205, 1.0600] | Removed |
| Intra-group parallel messages | Recovery enabled | 34.38 → 34.68 | [1.0103, 1.0217] | Removed; no demonstrated benefit |

The intra-group larger-u128 launch failed benchmark environment validation
before proving. It supplies no timing evidence. The harness now recognizes the
two retained candidate flags. Experimental patches and raw evidence remain in
`PerfRuns/gruen-20260917/`; rejected kernels are absent from production source.

## Clean-candidate exploration

On AMD Ryzen 9 9950X3D, Rust 1.98.1, `-C target-cpu=native`, six paired
blocks with tuning seed 17 give the following results against `4c03f2c1`.
Each process performs one separately reported first proof and five warmed
proofs. These exploratory measurements do not qualify a default.

| Threads | Metric | Baseline (ms) | Candidate (ms) | Paired ratio 95% interval |
| --- | --- | --- | --- | --- |
| 1 | Warm GKR | 227.859 | 219.048 | [0.9600, 0.9698] |
| 1 | Warm total proving | 429.944 | 422.235 | [0.9813, 0.9860] |
| 1 | First GKR | 237.931 | 222.785 | [0.9345, 0.9537] |
| 1 | First total proving | 442.716 | 426.920 | [0.9627, 0.9759] |
| 1 | Warm verifier | 21.562 | 21.603 | [0.9994, 1.0029] |
| 10 | Warm GKR | 34.913 | 34.044 | [0.9673, 1.0059] |
| 10 | Warm total proving | 70.113 | 69.591 | [0.9710, 1.0057] |
| 10 | First GKR | 48.733 | 48.640 | [0.9228, 1.0305] |
| 10 | First total proving | 85.173 | 85.677 | [0.9398, 1.0551] |
| 10 | Warm verifier | 7.332 | 7.358 | [0.9706, 1.0133] |

The serial improvement is demonstrated in this exploration. The ten-thread
result remains inconclusive, including the 2% warmed-GKR gate. Medians and
paired-ratio estimates are different statistics; do not infer an interval
from the ratio of the two displayed medians.

The clean candidate passes 479 library tests (five existing ignored tests),
20 transcript-pin tests, four virtual-opening tests, and the forest-grinding
test with both flags enabled. These include independent dense comparisons,
zero/one equality challenges, zero padding prefixes, mixed leaf types, and
multiple table sets. A separate test covers batched inverse fallback for
missing, zero, one, and nontrivial coordinates. The seven benchmark-harness
qualification tests also pass.

## Fixed primary confirmation

A fresh 24 paired blocks per seed, seeds 31 and 47, confirms a warmed-GKR
improvement on the primary ten-thread fixture. Its equal-seed-weighted ratio
interval is **[0.96739, 0.98301]**, corresponding to **1.70–3.26% less time**.
The required upper ratio bound is 0.98, so this is **inconclusive for the 2%
minimum**, despite showing a positive improvement. No extra blocks are added
after observing this fixed campaign.

| Seed | Metric | Baseline (ms) | Candidate (ms) | Paired ratio 95% interval |
| --- | --- | --- | --- | --- |
| 31 | Warm GKR | 34.926 | 34.153 | [0.9673, 0.9914] |
| 31 | Warm total proving | 69.693 | 68.828 | [0.9731, 0.9936] |
| 31 | First GKR | 48.463 | 48.176 | [0.9810, 1.0255] |
| 31 | First total proving | 85.008 | 84.571 | [0.9896, 1.0182] |
| 31 | Warm verifier | 7.170 | 7.167 | [0.9876, 0.9996] |
| 47 | Warm GKR | 35.035 | 34.153 | [0.9587, 0.9830] |
| 47 | Warm total proving | 69.905 | 68.840 | [0.9727, 0.9903] |
| 47 | First GKR | 49.036 | 47.973 | [0.9703, 0.9979] |
| 47 | First total proving | 84.952 | 83.918 | [0.9813, 1.0055] |
| 47 | Warm verifier | 7.074 | 7.057 | [0.9829, 1.0062] |

First-proof nonregression also remains inconclusive in some cells. The
candidate therefore did not qualify for a new default under the original
automatic policy. Whole-matrix 24-block confirmation cannot change this unmet primary
gate and is not represented as completed. A separate six-block workload
screen and allocation measurements characterize the candidate further.

## Workload screen

All 34 workload/thread cases completed six paired blocks, five warmed proofs
per process, seed 17. This covers native u32/u64/u128 and full u32 W1/W8 at
exponents 15/19; SHA chains at exponents 7/10/12; MultiSwap batches 1/2/4/8;
and one/ten threads. MultiSwap does not use this fixture seed. All proofs
verify, with exact corresponding proof bytes and transcript fingerprints.

For warmed whole-prover time, 20 cells improve, 13 are inconclusive, and one
slows down under the six-block interval rule. First-proof whole-prover time
improves in 22 cells and is inconclusive in 12. These are exploratory results,
not the complete 24-block acceptance matrix.

The small ten-thread SHA chain (`sha-n7`) regresses from **33.998 to 35.843 ms**
whole-prover time, ratio interval **[1.00304, 1.06093]**. Its GKR phase also
regresses, **20.321 to 22.013 ms**, interval **[1.02449, 1.09763]**. A universal
default was therefore rejected by the original automatic policy independently
of the inconclusive 2% primary gate. The subsequent user approval accepts this
tradeoff; no fixture-specific dispatch was introduced to hide this result.

All intervals below are candidate/baseline. Full per-block ratios,
first-proof GKR, online-prover timings, verifier measurements, fingerprints,
and binary hashes are in [the qualification data](gkr-sumcheck-qualification.json).

### Warm proving

| Workload | Threads | Total before → after (ms) | Total ratio interval | GKR before → after (ms) | GKR ratio interval |
| --- | --- | --- | --- | --- | --- |
| u32-mod32-n15 | 1 | 63.407 → 61.835 | [0.9747, 0.9783] | 47.391 → 45.797 | [0.9658, 0.9673] |
| u32-mod32-n15 | 10 | 15.599 → 15.442 | [0.9725, 1.0063] | 9.929 → 9.817 | [0.9676, 0.9963] |
| u32-mod32-n19 | 1 | 870.257 → 842.440 | [0.9470, 0.9880] | 748.155 → 719.053 | [0.9368, 0.9849] |
| u32-mod32-n19 | 10 | 133.664 → 130.619 | [0.9664, 0.9898] | 104.282 → 101.366 | [0.9596, 0.9836] |
| u64-n15 | 1 | 115.757 → 112.491 | [0.9713, 0.9727] | 90.649 → 87.373 | [0.9629, 0.9645] |
| u64-n15 | 10 | 26.310 → 25.971 | [0.9729, 1.0137] | 17.664 → 17.491 | [0.9538, 1.0289] |
| u64-n19 | 1 | 1667.239 → 1590.419 | [0.9460, 0.9995] | 1446.981 → 1368.916 | [0.9381, 0.9992] |
| u64-n19 | 10 | 302.961 → 299.529 | [0.9705, 1.0012] | 255.644 → 252.269 | [0.9683, 0.9996] |
| u128-n15 | 1 | 231.130 → 224.078 | [0.9688, 0.9713] | 185.356 → 178.207 | [0.9600, 0.9625] |
| u128-n15 | 10 | 44.688 → 44.497 | [0.9894, 1.0081] | 32.438 → 32.089 | [0.9857, 1.0035] |
| u128-n19 | 1 | 3330.665 → 3240.061 | [0.9631, 0.9768] | 2917.779 → 2826.833 | [0.9575, 0.9730] |
| u128-n19 | 10 | 608.519 → 594.799 | [0.9640, 0.9995] | 519.725 → 504.075 | [0.9545, 0.9933] |
| full-u32-w1-n15 | 1 | 63.079 → 61.489 | [0.9735, 0.9753] | 47.542 → 45.970 | [0.9648, 0.9670] |
| full-u32-w1-n15 | 10 | 15.586 → 15.523 | [0.9681, 1.0080] | 9.959 → 9.823 | [0.9658, 0.9973] |
| full-u32-w1-n19 | 1 | 843.948 → 843.076 | [0.9707, 1.0016] | 720.886 → 719.537 | [0.9656, 1.0013] |
| full-u32-w1-n19 | 10 | 135.106 → 134.112 | [0.9769, 1.0228] | 106.347 → 104.549 | [0.9622, 1.0151] |
| full-u32-w8-n15 | 1 | 58.126 → 56.454 | [0.9698, 0.9735] | 47.503 → 45.866 | [0.9642, 0.9679] |
| full-u32-w8-n15 | 10 | 14.549 → 14.332 | [0.9707, 1.0034] | 9.832 → 9.733 | [0.9657, 1.0049] |
| full-u32-w8-n19 | 1 | 829.289 → 771.011 | [0.9106, 0.9670] | 727.304 → 667.680 | [0.8974, 0.9610] |
| full-u32-w8-n19 | 10 | 131.445 → 128.967 | [0.9710, 1.0116] | 104.999 → 102.338 | [0.9599, 1.0049] |
| sha-n7 | 1 | 71.559 → 70.141 | [0.9780, 0.9832] | 43.266 → 41.631 | [0.9609, 0.9641] |
| sha-n7 | 10 | 33.998 → 35.843 | [1.0030, 1.0609] | 20.321 → 22.013 | [1.0245, 1.0976] |
| sha-n10 | 1 | 421.528 → 413.108 | [0.9796, 0.9820] | 255.460 → 245.722 | [0.9606, 0.9643] |
| sha-n10 | 10 | 107.515 → 108.545 | [0.9912, 1.0233] | 55.447 → 54.986 | [0.9664, 1.0138] |
| sha-n12 | 1 | 1639.311 → 1614.815 | [0.9751, 0.9872] | 1004.455 → 971.077 | [0.9531, 0.9685] |
| sha-n12 | 10 | 345.312 → 344.283 | [0.9884, 1.0046] | 171.657 → 168.233 | [0.9697, 0.9884] |
| multiswap-b1 | 1 | 476.063 → 457.702 | [0.9420, 0.9849] | 359.806 → 344.212 | [0.9292, 0.9892] |
| multiswap-b1 | 10 | 106.015 → 105.687 | [0.9835, 1.0005] | 52.971 → 51.497 | [0.9597, 0.9987] |
| multiswap-b2 | 1 | 975.806 → 955.437 | [0.9670, 0.9825] | 750.417 → 734.989 | [0.9643, 0.9819] |
| multiswap-b2 | 10 | 212.195 → 210.831 | [0.9529, 1.0024] | 121.889 → 119.097 | [0.9328, 0.9993] |
| multiswap-b4 | 1 | 1977.240 → 1927.170 | [0.9733, 0.9757] | 1530.467 → 1484.341 | [0.9655, 0.9718] |
| multiswap-b4 | 10 | 422.097 → 415.418 | [0.9762, 0.9982] | 247.537 → 242.497 | [0.9677, 1.0161] |
| multiswap-b8 | 1 | 4015.729 → 3899.083 | [0.9667, 0.9769] | 3086.140 → 2980.453 | [0.9596, 0.9721] |
| multiswap-b8 | 10 | 867.483 → 859.349 | [0.9790, 0.9969] | 510.910 → 495.682 | [0.9643, 0.9890] |

### First proof after setup

| Workload | Threads | Total before → after (ms) | Total ratio interval | GKR before → after (ms) | GKR ratio interval |
| --- | --- | --- | --- | --- | --- |
| u32-mod32-n15 | 1 | 65.381 → 63.614 | [0.9716, 0.9774] | 48.714 → 46.927 | [0.9619, 0.9667] |
| u32-mod32-n15 | 10 | 17.475 → 16.658 | [0.9201, 1.0062] | 10.971 → 10.820 | [0.9611, 1.0139] |
| u32-mod32-n19 | 1 | 883.262 → 854.395 | [0.9641, 0.9708] | 752.823 → 724.373 | [0.9588, 0.9659] |
| u32-mod32-n19 | 10 | 139.720 → 138.027 | [0.9381, 1.0182] | 108.027 → 106.244 | [0.9227, 1.0111] |
| u64-n15 | 1 | 119.702 → 116.322 | [0.9667, 0.9734] | 93.220 → 89.745 | [0.9581, 0.9646] |
| u64-n15 | 10 | 30.728 → 30.602 | [0.9577, 1.0370] | 21.447 → 21.122 | [0.9447, 1.0344] |
| u64-n19 | 1 | 1770.222 → 1712.779 | [0.9669, 0.9684] | 1537.441 → 1479.656 | [0.9618, 0.9634] |
| u64-n19 | 10 | 306.782 → 300.430 | [0.9686, 0.9942] | 256.381 → 250.031 | [0.9609, 0.9922] |
| u128-n15 | 1 | 237.964 → 231.180 | [0.9674, 0.9723] | 189.849 → 182.983 | [0.9586, 0.9642] |
| u128-n15 | 10 | 49.780 → 51.923 | [0.9898, 1.0960] | 36.239 → 36.707 | [0.9862, 1.0735] |
| u128-n19 | 1 | 3449.124 → 3337.071 | [0.9656, 0.9689] | 3021.075 → 2906.836 | [0.9603, 0.9636] |
| u128-n19 | 10 | 611.766 → 604.148 | [0.9624, 1.0006] | 521.020 → 511.556 | [0.9548, 0.9962] |
| full-u32-w1-n15 | 1 | 65.090 → 63.355 | [0.9723, 0.9761] | 48.700 → 47.021 | [0.9638, 0.9682] |
| full-u32-w1-n15 | 10 | 18.170 → 18.289 | [0.9626, 1.0480] | 11.493 → 11.779 | [0.9760, 1.0492] |
| full-u32-w1-n19 | 1 | 888.337 → 862.908 | [0.9706, 0.9735] | 755.257 → 729.861 | [0.9652, 0.9685] |
| full-u32-w1-n19 | 10 | 160.800 → 162.797 | [0.9358, 1.0206] | 127.703 → 128.854 | [0.9264, 1.0183] |
| full-u32-w8-n15 | 1 | 59.795 → 58.176 | [0.9702, 0.9741] | 48.572 → 46.945 | [0.9634, 0.9680] |
| full-u32-w8-n15 | 10 | 17.574 → 16.729 | [0.9105, 0.9940] | 11.941 → 11.395 | [0.9265, 0.9813] |
| full-u32-w8-n19 | 1 | 867.767 → 840.917 | [0.9645, 0.9694] | 755.330 → 727.943 | [0.9604, 0.9642] |
| full-u32-w8-n19 | 10 | 162.909 → 156.517 | [0.9274, 0.9831] | 130.601 → 124.313 | [0.9103, 0.9786] |
| sha-n7 | 1 | 73.414 → 71.988 | [0.9687, 0.9907] | 44.976 → 43.373 | [0.9528, 0.9763] |
| sha-n7 | 10 | 37.038 → 36.700 | [0.9371, 1.0213] | 23.016 → 22.135 | [0.9102, 1.0076] |
| sha-n10 | 1 | 434.786 → 426.222 | [0.9700, 0.9930] | 266.956 → 256.978 | [0.9488, 0.9783] |
| sha-n10 | 10 | 123.343 → 121.095 | [0.9520, 0.9952] | 70.062 → 68.049 | [0.9263, 0.9981] |
| sha-n12 | 1 | 1658.198 → 1631.544 | [0.9804, 0.9911] | 1016.346 → 976.492 | [0.9577, 0.9632] |
| sha-n12 | 10 | 386.966 → 380.096 | [0.9558, 1.0241] | 202.906 → 197.951 | [0.9452, 1.0114] |
| multiswap-b1 | 1 | 488.995 → 472.901 | [0.9536, 0.9849] | 364.827 → 351.069 | [0.9537, 0.9671] |
| multiswap-b1 | 10 | 130.812 → 133.208 | [0.9573, 1.1552] | 74.493 → 74.955 | [0.9245, 1.0437] |
| multiswap-b2 | 1 | 1017.447 → 975.500 | [0.9521, 0.9665] | 775.633 → 740.141 | [0.9468, 0.9613] |
| multiswap-b2 | 10 | 231.794 → 228.885 | [0.8989, 1.0175] | 135.564 → 130.649 | [0.8831, 1.0083] |
| multiswap-b4 | 1 | 2029.946 → 1970.734 | [0.9631, 0.9768] | 1543.747 → 1491.052 | [0.9564, 0.9734] |
| multiswap-b4 | 10 | 460.258 → 452.337 | [0.9744, 1.0129] | 268.398 → 263.166 | [0.9619, 1.0247] |
| multiswap-b8 | 1 | 4060.596 → 3945.507 | [0.9654, 0.9783] | 3080.205 → 2979.211 | [0.9586, 0.9752] |
| multiswap-b8 | 10 | 948.020 → 938.272 | [0.9854, 0.9929] | 561.494 → 552.367 | [0.9732, 0.9882] |

### Verifier tradeoffs

Verifier arithmetic and proof encoding are unchanged, but measured verifier
time is not identical. The screen flags 14 slower cells, two faster cells,
and 18 inconclusive cells. The flagged slowdowns below are reported tradeoffs;
the cause was not isolated, and they must not be called protocol changes.

| Workload | Threads | Verifier before → after (ms) | Ratio interval |
| --- | --- | --- | --- |
| u32-mod32-n15 | 1 | 4.409 → 4.422 | [1.0023, 1.0038] |
| u32-mod32-n15 | 10 | 2.198 → 2.210 | [1.0002, 1.0097] |
| u64-n15 | 1 | 6.974 → 6.994 | [1.0024, 1.0033] |
| u64-n19 | 1 | 24.867 → 24.918 | [1.0019, 1.0029] |
| u64-n19 | 10 | 5.894 → 6.069 | [1.0097, 1.0492] |
| u128-n15 | 1 | 12.902 → 12.926 | [1.0015, 1.0031] |
| u128-n15 | 10 | 3.535 → 3.640 | [1.0188, 1.0448] |
| u128-n19 | 1 | 48.173 → 48.323 | [1.0018, 1.0038] |
| full-u32-w1-n15 | 1 | 4.437 → 4.467 | [1.0063, 1.0066] |
| full-u32-w1-n15 | 10 | 2.216 → 2.264 | [1.0085, 1.0278] |
| full-u32-w1-n19 | 1 | 13.141 → 13.183 | [1.0013, 1.0046] |
| full-u32-w1-n19 | 10 | 3.811 → 3.870 | [1.0080, 1.0357] |
| full-u32-w8-n15 | 1 | 2.267 → 2.284 | [1.0060, 1.0139] |
| full-u32-w8-n15 | 10 | 1.939 → 1.949 | [1.0000, 1.0124] |

## Allocation and memory measurements

Separate allocation-instrumented binaries use `span-metrics,ecdsa,bench-peak-memory`
and `-C target-cpu=native`. Each cell is one verified proof in a fresh process,
using the fixed fixture of `gkr_capture`. These timings are excluded from the
latency campaign. Counters measure requested live Rust heap, including owned
cached capacity and instrumentation, with nested phases counted inclusively;
phase totals must not be added. RSS is process high-water RSS including setup.
These single-run RSS values have no uncertainty interval and do not establish
an RSS improvement.

| SHA compressions | Threads | Proving peak heap before → after (MiB) | GKR allocation calls before → after | GKR cumulative allocation before → after (bytes) | Process RSS before → after (MiB) |
| --- | --- | --- | --- | --- | --- |
| 128 | 1 | 151.720 → 151.720 | 38,934 → 41,266 | 62,249,453 → 62,343,454 | 672.793 → 672.812 |
| 128 | 10 | 151.782 → 151.782 | 38,934 → 41,266 | 62,249,453 → 62,343,454 | 662.941 → 662.254 |
| 1,024 | 1 | 219.041 → 219.041 | 155,179 → 158,265 | 306,060,613 → 306,184,890 | 672.801 → 672.820 |
| 1,024 | 10 | 219.102 → 219.102 | 155,194 → 158,280 | 308,137,301 → 308,261,578 | 662.809 → 662.664 |

There is **no meaningful proving-peak reduction**. On the main ten-thread
fixture, GKR allocation calls increase by 3,086 and cumulative requested GKR
allocation increases by 124,277 bytes (about 121.4 KiB). The instrumentation
includes the new `eqf:recover` and `eqf:direct_close` spans; their contribution
has not been isolated. Removed partial-sum vectors alone must not be advertised
as a demonstrated whole-GKR allocation reduction. No new workspace cache is
introduced by this candidate.

The approximately 799 MiB whole-process requested-heap peak occurs during
setup, outside the proving peak reported above. It is not interchangeable with proving heap
or resident memory. Detailed per-phase counters, including witness generation,
commitment, proving, and GKR, are retained in the qualification JSON.

## Final correctness and source disposition

- Release library: 479 passed, five existing ignored tests; transcript pins:
  20 passed; virtual openings: four passed; forest grinding: one passed.
- Nonparallel library: 471 passed, five existing ignored tests.
- Eight separate-process override configurations pass: baseline fallback,
  flat/per-tree storage, generic kernels, no fusion, single-round grids,
  L2, and L8. The forest differential tests also exercise L2/L4/L8 directly.
- Both forced quad variants produce baseline-identical proofs and transcript
  continuation. Their concurrent-build run timings are correctness artifacts
  and are excluded from performance results.
- Vendor field tests: five binary-provider tests and three fused-sumcheck
  tests pass. The seven qualification-harness tests pass.
- The explicit ECDSA regression test passes for split/all-row outer modes
  and security targets 100/128. Existing proof and continuation pins are unchanged.

The exact measured implementation is preserved in **`db82a93b`**, on
**`gkr-cofactor-experiment-20260917`**. That archived commit has opt-in controls;
all reported candidate measurements enabled both controls explicitly.
The implementation is now merged into **`gkr-optimizations`**, with both controls
on by default. Explicit `0` selects the corresponding baseline path.

The original automatic acceptance policy was not satisfied: the fixed primary
confirmation does not establish the minimum 2% GKR gain, several first-proof
gates remain inconclusive, and the screen flags a small-SHA whole-prover
regression. The complete 24-block workload/outer-mode/security matrix was not
run. The user subsequently approved adoption with these measured tradeoffs.
The default-setting change does not introduce a new arithmetic implementation
or claim a new benchmark result.

With both environment variables unset, the enabled defaults pass nine sumcheck
tests, four forest-equivalence tests, 20 transcript pins, four virtual openings,
the grinding test, and the pinned ECDSA regression across both outer modes and
security targets 100/128. A separate process with both controls set to `0`
also passes the nine sumcheck tests.

Raw builds, hashes, paired samples, allocation counters, compatibility checks,
and rejected patches are retained under `PerfRuns/gruen-20260917/`. The
committed [qualification data](gkr-sumcheck-qualification.json) preserves the
measured results and their uncertainty independently of those local binaries.
