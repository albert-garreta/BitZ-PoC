# Candidate V1 diagnostics

V1 did not pass the fixed qualification. Native 100-bit batch 1 completed all 60 selected seeds. Total proving passed, but the verifier upper confidence bound exceeded 1.02:

| Metric | Baseline mean ms | Candidate mean ms | Ratio | One-sided 95% upper |
|---|---:|---:|---:|---:|
| Total prover | 22.726933 | 22.763522 | 1.001610 | 1.018556 |
| Verifier | 14.806591 | 15.093781 | 1.019396 | 1.040195 |

The verifier interval is inconclusive under the accepted criterion, so the campaign cannot qualify. Cooperative stopping preserved 78 complete pairs: 60 in Native 100-bit batch 1 and 18 in batch 3, totaling 624 verified proofs. The remaining samples were not run. Binary, source, and all 240 fixture hashes passed the final integrity checks. [Canonical report](qualification/REPORT.md), [raw summary](qualification/summary.json), [manifest](qualification/manifest.json).

The batch-1 verifier's mean paired difference was +0.287190 ms, its median difference was +0.184589 ms, and the candidate was faster on 24 of 60 seeds. Pair differences ranged from -3.352768 to +3.566893 ms. These observations remain in the qualification; none are excluded as noise.

The original instrumented seed-42 run reported verifier 15.704905 → 15.520225 ms at batch 1 and 16.278340 → 16.281116 ms at batch 3. Those stage spans expose the whole verifier and native-ring internals, but do not separately time compact terminal reconstruction, so they cannot attribute the qualification difference to that operation.

A separately authorized diagnostic ran baseline then V1 at Native 100-bit batch 1, seed 42, with one warmup, five measured proofs, and `LIG_VERIFY_TRACE=1`. Inputs, roots, security terms, and grinding diagnostics matched. Whole-verifier medians were 13.074180 → 12.337726 ms. Succinct Ligerito measured totals were baseline `[2.24, 1.73, 1.59, 1.36, 1.35]` ms and candidate `[1.36, 1.34, 1.35, 1.37, 1.37]` ms. The largest reported variation was in residual-basis evaluation. This trace starts after compact terminal reconstruction and therefore does not measure the proposed NTT-setup optimization directly. These diagnostics are not qualification samples. [Trace manifest](verify-trace/manifest.json), [parsed stages](verify-trace/analysis.json), [baseline trace](verify-trace/baseline.stderr), [candidate trace](verify-trace/candidate.stderr).

All 16 cold and stage cases verified with identical source roots, security ledgers, and grinding diagnostics. Every cold-case proof was smaller. SharedPrime batch 1,024 seed 42 changed from 700,466 to 659,002 bytes at 100 bits and from 867,546 to 814,810 bytes at 128 bits. These size results do not override the failed timing qualification.
