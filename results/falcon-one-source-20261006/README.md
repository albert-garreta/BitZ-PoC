# Falcon one-source implementation and qualification

**Final V3 result: PASS.** NativeCarry and SharedPrime use one initial source
Merkle commitment and one source multiproof. Recursive PCS trees remain.
Inactive Keccak instances are zero; no dummy computations or old Falcon proof
compatibility path remain. The protocol domains are native-ring v6 and
shared-prime v5. Security targets and grinding requirements are unchanged.

The [protocol specification](../../src/piop/spartan/falcon1024_ct/ONE_SOURCE.md)
describes the projections, decoder transposes, activity constraints, transcript
binding, and soundness accounting.

## Measured SharedPrime results at batch 1024

Times include witness generation and commitment in total proving. Each timing
entry is the arithmetic mean of the 60 per-seed medians; payload entries are
the mean stored proof payload, excluding public inputs and transport framing.
All runs used 16 threads on the same machine and compiler.

| Target | Total prover, baseline → candidate | Verification, baseline → candidate | Payload, baseline → candidate |
| --- | --- | --- | --- |
| 100 bits | 727.27 → 689.79 ms (5.15% faster) | 58.82 → 55.30 ms (5.98% faster) | 887,891.60 → 699,228.13 bytes (21.25% smaller) |
| 128 bits | 968.66 → 931.33 ms (3.85% faster) | 62.54 → 59.22 ms (5.31% faster) | 1,100,810.00 → 866,417.47 bytes (21.29% smaller) |

## Acceptance evidence

The final campaign covers both protocols, both security targets, and batches
1, 3, 32, and 1024: 16 cases, 960 matched baseline/candidate pairs, 1,920 fresh
processes, and 7,680 verified proofs. Each process has one warmup and three
measured repetitions. Every case uses the fixed seeds 42 through 101 with
balanced execution order and real grinding.

All 32 runtime gates pass. The largest one-sided 95% bootstrap upper bound on
candidate/baseline runtime is **0.97381787**, below the required **1.02**.
All 16 mean-payload gates pass, including at least 20% reduction for both
batch-1024 SharedPrime targets. Earlier unsuccessful V1/V2 qualification runs
are retained separately; the final result uses the complete fresh V3 campaign.

- [Final performance analysis](performance/FINAL.md)
- [Complete qualification table](performance/v3/qualification/REPORT.md)
- [Machine-readable statistics](performance/v3/qualification/summary.json)
- [Execution and integrity manifest](performance/v3/qualification/manifest.json)
- [Build provenance](performance/builds-v3.json)
- [Correctness validation and limitations](validation/REPORT.md)
- [Final source and result audit](validation/v3/final-audit.json)
- [Detailed seed-42 proof accounting](size-analysis/REPORT.md)

The final root validation includes 133 Falcon tests and the explicit 28-proof
matrix for both protocols and targets across batches 1, 2, 3, 7, 8, 9, and
1024. Joint commitment, malformed authentication, projection, decoder, activity,
padding, independent Keccak reference, and generic-client checks also pass.
The validation report identifies the existing unrelated prototype and vendor
fixture failures; this is not a claim that every workspace target passes.

All 1,727 files in the frozen candidate source manifest match the final working
tree. The final build, source, and all 240 cached-input hashes were rechecked.
`git diff --check` passes.

Integer column sums remain the largest individual proof component: 262,144
bytes at 100 bits and 327,680 bytes at 128 bits, about 37–38% of the new proof.
This change meets the one-source plan but does not reach 0.4 MB.
