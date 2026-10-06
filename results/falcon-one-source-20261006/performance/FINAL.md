# Falcon one-source performance qualification

**Final candidate V3 passes every timing and payload gate.** The canonical evidence is the [full V3 report](v3/qualification/REPORT.md), [machine-readable summary](v3/qualification/summary.json), [campaign manifest](v3/qualification/manifest.json), and [frozen build manifest](builds-v3.json). V3 here denotes the optimization candidate; protocol versions are Native V6 and SharedPrime V5.

The fixed matrix covered Native and SharedPrime, security targets 100/128, batches 1/3/32/1024, and 16 threads. Every cell used seeds 42–101, one warmup and three measured proofs per process, with balanced AB/BA/BA/AB baseline/candidate ordering. All **960 matched pairs, 1,920 processes, and 7,680 proof verifications** completed. Timings below are arithmetic means of per-seed process medians; the total prover includes witness commitment and proof generation.

Each of the 32 primary timing metrics passed its paired one-sided 95% bootstrap upper-bound requirement of 1.02. The largest upper bound was **0.97381787**, for SharedPrime/128/batch1024 total prover time. Each interval used 20,000 paired bootstrap draws. Every mean payload decreased, including reductions of at least 20% for both large SharedPrime cases.

| SharedPrime security | Prover ms, baseline → V3 | Verifier ms, baseline → V3 | Mean payload bytes, baseline → V3 | Size reduction |
|---|---:|---:|---:|---:|
| 100 bits | 727.27 → 689.79 | 58.82 → 55.30 | 887,891.60 → 699,228.13 | 21.25% |
| 128 bits | 968.66 → 931.33 | 62.54 → 59.22 | 1,100,810.00 → 866,417.47 | 21.29% |

Payload means are canonical stored proof bytes, excluding Falcon framing and the public statement. The baseline is revision `51405193878e15e1e4259216464f2bb856c8b592`; both builds used the same compiler, lockfile, release profile, `falcon-hybrid` feature set, and `-C target-cpu=native`.

Fresh-process, zero-warmup seed-42 RSS diagnostics for batch1024 are separate from the repeated latency campaign:

| Protocol | Security | Baseline peak MiB | V3 peak MiB | Reduction MiB |
|---|---:|---:|---:|---:|
| native | 100 | 2980.01 | 2725.24 | 254.77 |
| native | 128 | 3011.00 | 2723.60 | 287.40 |
| shared-prime | 100 | 3005.98 | 2753.26 | 252.72 |
| shared-prime | 128 | 3010.51 | 2751.58 | 258.93 |

RSS is the process-lifetime high-water mark, not a per-proof allocation count. See the [fresh cold report](v3/cold/REPORT.md) and [current stage diagnostics](v3/stages/INITIAL-COMMIT.md). Stage timings are diagnostic and were not used for latency acceptance. Candidate encoding timers include allocation/fill, whereas the old transform-only timers do not; they are not compared directly.

Final integrity checks revalidated both binaries, every frozen source file, the unchanged cache-reference manifest, and all 240 generated input fixtures. Each process checked workload/security metadata and input identity, and every measured and warmup proof verified. The [standard proof-equivalence checks](v3/proof-equivalence.json) and [extra batches 7/9 checks](v3/extra-equivalence/manifest.json) confirm V2/V3 equality of roots, proof Debug digests, payload, input digests, grinding diagnostics, and security headers in all 24 tested configurations.

[V1](v1/qualification/REPORT.md) stopped after 151 complete pairs and [V2](v2/qualification/REPORT.md) after 163 complete pairs because completed prover cells did not meet the confidence-bound gate. Both are explicitly incomplete and not qualified. Their raw evidence is retained, and none of their samples was combined with the fresh V3 qualification.
