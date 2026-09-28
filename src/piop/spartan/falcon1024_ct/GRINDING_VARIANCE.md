# Why some Falcon batches fall below 1,000 signatures/sec

The repeatably slow case is caused by **more PCS proof-of-work nonce searches**,
with a separate source of execution variability in some other runs. The
additional diagnostics confirm the search work directly: seed 7 requires
**513,715,303 nonce candidates**, compared with **105,264,354** for seed 2026.
That is **4.88 times as much PCS search work** for the same circuit geometry
and security schedule.

This is BLAKE3 proof-of-work in the shared Ligerito PCS. The Falcon ring,
HashToPoint, norm and Keccak relations retain their existing sizes. In the
original cold profiles, PCS grinding explained **122.203 ms of a 126.405 ms**
batch-time difference (96.68%). All work excluding grinding differed by only
2.106 ms per 1,024-signature batch.

## Direct nonce evidence

The diagnostic build records the existing `lig:grind_pow` span's difficulty,
public transcript seed words and selected nonce. The benchmark subscriber now
retains span fields, including the trial number. Neither change samples or
absorbs transcript bytes or changes the nonce search.

Each case used B=1024, target 128, 16 workers, native release, one warmup and
three instrumented measured trials. **All 20 diagnostic proofs verified and
matched the saved baseline input/proof digests, commitment roots, stored
payloads and security reports.** Each seed repeated exactly the same 30 PCS
challenge seeds, difficulties and nonces in all four trials.

| Input seed | Required PCS nonce candidates | Median PCS grinding, ms/batch |
| --- | ---: | ---: |
| 7 | 513,715,303 | 147.511 |
| 2026 | 105,264,354 | 33.377 |
| 0 | 200,052,934 | 59.810 |
| 1 | 205,692,426 | 61.486 |
| 123 | 197,506,939 | 60.201 |

Candidate count is the sum of `nonce + 1` over the 30 positive-difficulty
boundaries. The prover selects the smallest valid nonce, so it must check those
ascending candidates. SIMD tails and parallel chunks already in flight add
some extra work; these counts do not claim to measure that overscan. Times here
are instrumented diagnostics, separate from the ordinary throughput results in
[SEED_VALIDATION.md](SEED_VALIDATION.md).

The three largest searches for seed 7 account for about **122.7 ms/batch**:

| Boundary | Difficulty | Selected nonce | Multiple of expected search work | Median time, ms/batch |
| --- | ---: | ---: | ---: | ---: |
| Level 0, fold 0 | 26 bits | 213,279,644 | 3.18× | 60.398 |
| Level 1, fold 0 | 25 bits | 141,619,462 | 4.22× | 40.400 |
| Level 0, fold 1 | 25 bits | 78,161,714 | 2.33× | 21.905 |

## Why equal security settings produce unequal work

A difficulty of `g` requires a hash with `g` leading zero bits. Under the hash
model, each candidate succeeds with probability `2^-g`. The number of attempts
has a geometric distribution with mean `2^g`; the difficulty does not set a
fixed amount of work. A 26-bit boundary averages 67,108,864 attempts, but seed 7
needs 213,279,645 at that boundary.

Different public batches produce different commitments and Fiat–Shamir seeds.
Their successful nonces therefore occur at different positions. Repeating one
batch produces the same transcript and the same nonce positions, which explains
why seed 7 repeatedly takes longer. Its original and five-trial repeat medians
were 1.050039 and 1.053647 ms/signature; all eight measured trials exceeded 1 ms.
Reaching 1 ms from that repeat median requires saving **54.9 ms per batch**, or
**5.1%** of total proving time.

The current 30-block PCS schedule allocates the same error budget to each block:

| Boundaries | Current difficulties |
| --- | --- |
| Level 0 folds | 26, 25, 25, 24 |
| Level 1 folds | 25, 24, 23 |
| Level 2 folds | 23, 22, 21 |
| Level 3 folds | 21, 20, 19 |
| Level 4 folds | 19, 18, 17 |
| Level 5 folds | 17, 16, 15 |
| Six query boundaries | 11 each |
| Five introduction boundaries | 7 each |

Expected PCS work is **229,225,088 candidates**, with **65.9% in the four
level-zero folds**. Under the same independent-hash model, its standard
deviation is about **92.8 million candidates**, or 40.5% of the mean PCS work.
This is a distribution of search work, not a whole-prover timing deviation.

The allocation is in [hybrid.rs](hybrid.rs), using the raw errors and native
minimum difficulties in [grinding.rs](../../../ligerito_flock/grinding.rs).
At target 128 it assigns target 135 to each of the 30 blocks. Larger raw-error
folds consequently receive much larger search requirements.

## The other misses are not all the same problem

Eight primary trials missed 1 ms/signature: two for seed 0, two for seed 1,
all three for seed 7, and one for seed 123. Seed 1's five-trial repeat improved
from 1.046113 to 0.931136 ms/signature with the same proof and required nonce
work. Seed 0 ranged from 0.931356 to 1.154635 within the original process.

These within-seed changes are execution variability. Existing observations
cannot identify whether OS scheduling, CPU frequency, allocation/page behavior
or another runtime effect caused those particular outliers. The nonce evidence
rules out different required search work as their explanation. The additional
instrumented runs are diagnostic observations and do not replace those original
measurements.

## A concrete next optimization, not yet implemented

The PCS error budget can potentially be distributed to minimize total search
work. An exact-rational analysis of the existing 30 raw-error terms found this
feasible illustrative schedule:

| Boundaries | Candidate difficulties |
| --- | --- |
| Level 0 folds | 24, 23, 23, 23 |
| Level 1 folds | 23, 23, 22 |
| Level 2 folds | 22, 22, 21 |
| Level 3 folds | 21, 21, 20 |
| Level 4 folds | 20, 20, 19 |
| Level 5 folds | 19, 19, 18 |
| Query boundaries | 16 each |
| Introduction boundaries | 14 each |

Within the repository's existing work-normalized security accounting:

- Expected PCS candidates decrease from **229.225 million to 83.050 million**,
  a **63.77% reduction**.
- The summed PCS error is **3.88% smaller**, with every native minimum retained.
  The PCS component bound increases from 130.359032 to 130.416072 bits.
- Search-work standard deviation decreases from **92.8 million to 26.5 million**.

These are analytical results, **not measured prover speedups** or an independent
soundness proof. The candidate has only been analyzed for this B=1024,
target-128 configuration. Implementing it requires binding/versioning the new
schedule, recomputing the complete protocol bound for all supported batches and
targets, and validating adversarial proofs and performance across new seeds.
No grinding difficulty or protocol rule was changed in this investigation.

Choosing a different valid nonce or restarting the transcript would change the
proof, but would not eliminate geometric search variability. The existing
minimum-nonce choice preserves deterministic proofs; it is not a verifier
requirement at positive difficulty. Optimizing hash throughput, reducing other
stages and reallocating the budget can provide more margin, but finite-rate
nonce search has a tail and does not guarantee a fixed latency for every input.

## Evidence and reproduction

`bench_results/falcon-under1000-20260927/` retains the diagnostic executable,
build log and source hashes; the five profiles and complete proof comparisons;
all 30 nonces and timings for each seed; and the original-data analysis.

`analyze_nonces.py` checks the actual difficulty list against the reconstructed
schedule and exact proof equivalence against the earlier measurements.
`analyze_pcs_schedule.py` uses exact rational arithmetic for raw errors, candidate
allocation, expected-work/variance sums and feasibility. It reconstructs the
existing PCS security term to the saved report's floating-point precision.

```sh
python3 bench_results/falcon-under1000-20260927/analyze_pcs_schedule.py
python3 bench_results/falcon-under1000-20260927/analyze_nonces.py
```
