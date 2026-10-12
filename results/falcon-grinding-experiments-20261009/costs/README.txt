Grinding kernel calibration, 2026-10-09

Source dependency: baseline d9b13bdb43cd2d3a0901b44f0939293f234661b5,
native release library from the previous accepted benchmark build. Exact
dependency/library and executable hashes are in build.json. No production
source was changed to make this probe.

Run: Ryzen 9 9950X3D, sequential 1/2/4/8/16-worker pinned Rayon pools.
Worker order 6,7,4,5,2,3,0,1,14,15,12,13,10,11,8,9. Main thread CPU31.
13,760 measured searches plus 735 warmups; all nonces checked against BLAKE3,
and every measured (kernel,bits,seed) returns the same minimum nonce across
thread counts. Run duration16.44s; maximum unused SMT-sibling utilization0.49%.

Spartan calls find_grinding_nonce on a32-byte seed. PCS calls the actual
ZincChallenger::grind_pow with a plain Blake3Transcript:16-byte seed extraction,
nonce scan, and nonce absorption are timed. Both select the same AVX-512
first_pow_nonce kernel on this host. Difficulty>=12 broadcasts across Rayon;
chunk size256 for bits12..16,1024 above16. All calls run inside pool.install;
pool setup, input preparation, nonce verification, and CSV formatting are
outside timed regions. No tracing subscriber is installed.

Inputs:64 independent deterministic seeds per difficulty<=18,32 per larger
difficulty, with three excluded warmups per cell. Bits1..24 measured for both
kernels; PCS also measures zero-bit wrapper overhead. raw.csv keeps every
sample including warmups(sample<0), actual minimum nonce, timing, CPU, worker.

curves.json: use curves_unconstrained[thread][spartan|pcs][difficulty] in ns.
For each cell, Huber robust regression estimates elapsed_ns=overhead+slope*
(nonce+1), with nonnegative coefficients; evaluate at expected attempts2^bits.
This removes luck from the empirical mean nonce distribution while retaining
dispatch/overscan costs. Difficulty25..32 extrapolates the fitted bits21..24
throughput, anchored at the measured bit24 estimate. Difficulty0 is modeled0
(no grinding); measured PCS zero-bit transcript overhead is in summary.json.

The actual curve is NOT monotone:8workers at Sparta11bits costs~7.11us but
12bits~3.99us; PCS~7.99us versus~3.56us. More bits cross the parallel threshold.
curves applies a monotone cumulative-maximum envelope only for clients that
require monotone inputs. It is not the actual estimate at each difficulty.
For a scheduler with Pareto pruning, prefer curves_unconstrained.

Caveats: nonce+1 is the required ordered prefix, not the total number of
redundant hashes performed by workers on in-flight chunks. The fitted
throughput therefore represents wall time per logical required attempt.
Searches inside a full proof can see different sleep/cache/clock conditions.
One-worker Spartan bits1..18 started slower(~4.2ns/attempt) than long searches
later in that pool(~3.4ns/attempt), consistent with clock warmup. This effect
is flagged and retained unadjusted. If one-worker schedules matter to the
next experiment, rerun that cell set with sustained warmup before using it.
