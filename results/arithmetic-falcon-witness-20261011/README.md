# Arithmetic Falcon witness optimization

The first compact implementation added packing and decoder work that outweighed its smaller commitments in some configurations. The revised implementation keeps the original signed15 bits at the minimum PCS allocation, where compression saves no physical storage. Larger batches use compact planes, parallel packing and carry accumulation, and a fused ring-weight recurrence. Binary marginal scheduling uses otherwise-idle workers, and tensor preparation avoids redundant copies and multiplication by one.

**All 24 final configurations pass both observed-median proving-speed gates.** Proof generation is 0.213–4.074% faster and total proving is 0.184–3.051% faster than the original baseline. All 4,608 measured and warmup proofs verified. These measurements cover both degrees, security targets 100/128, batches 1/32/1024, and one/eight threads; they do not guarantee faster execution for every input or machine. Payloads are 2.448–21.168% smaller. Verification remains mixed, ranging from 1.327% faster to 2.434% slower.

The final fixed benchmark plan and measured speed results are recorded in [the performance report](performance-first-report.md). Earlier failures remain in [the original v6 report](v6-summary.md) and [the pre-fallback qualification](v7-qualified-report.md); they are not measurements of the final implementation.

The final release correctness checks pass **118 tests**, including negative arithmetic cases, parallel reductions, large batches, and full Falcon roundtrip/tampering. See [validation](VALIDATION.md) for logs, coverage and the separate portable/native field checks.

## Witness sizes

| Quantity | Falcon-512 | Falcon-1024 |
|---|---:|---:|
| Live bits at the PCS floor | 15,386, unchanged | 30,747, unchanged |
| Maximum live bits above the floor, before → after | 15,386 → 12,338 (−19.81%) | 30,747 → 24,651 (−19.83%) |
| Physical source at batch 32 | 64 → 64 KiB | 128 → 104 KiB |
| Physical source at batch 1024 | 2 → 1.625 MiB | 4 → 3.25 MiB |
| Removed batched ring certificate | 11,242 bytes | 22,506 bytes |

The compact layout starts at batch 33 for Falcon-512 and batch 17 for Falcon-1024. It reduces physical committed source bits by **18.75%**, including source padding and excluding Reed–Solomon expansion. Live counts include maximum exception capacity and norm slack. Compact exception indices add `2 + 2*count` proof bytes per signature; the original-Falcon benchmark inputs used no exceptions. The minimum-size layout has no exception metadata. These are committed witness sizes, not total prover memory: the regular aligned arithmetic view remains in use.

Both layouts preserve every supplied coefficient exactly, including valid noncentered values. The arithmetic-only API still proves the ring relation and the integer norm bound for public `(h,t)` and private `(s1,s2)`. It does not compute SHAKE-256 or HashToPoint. The full Falcon path remains available. The arithmetic protocol domain is v7; older arithmetic proofs are incompatible.

## Measurement method

- Original v5 baseline and final candidate use locked/offline native release builds with `falcon-hybrid`, the same compiler and profile, and the same deterministic Falcon inputs.
- AMD Ryzen 9 9950X3D. One worker is pinned to CPU 6; eight workers to CPUs 6,7,4,5,2,3,0,1. Both Rayon pools use the configured count.
- Both degrees and security levels 100/128 are measured. Batches 1 and 32 use 32 independent seeds (42–73); batch 1024 uses eight (42–49). Each variant/input has one warmup and three measured proofs. Variant order alternates by seed. Processes run sequentially, with no concurrent builds or tests.
- The strict observed-median gate rejects any positive delta in either total proving or proof-only time. Seed-paired ratios and bootstrap intervals supplement that gate. Protocol changes alter deterministic prime search and grinding; repetitions of one seed do not provide independent coverage of this variation.
- Total proving includes witness derivation, packing, commitment and proof generation. Input generation, decoding and external hashing are excluded. Diagnostic tracing is disabled for timing runs.
- Payload accounting includes stored proof values and PCS payload, excludes public inputs and transport framing, and does not specify a wire codec or Rust heap usage.

See [validation](VALIDATION.md) for correctness checks, [the final small matrix](performance-first-small/) and [large matrix](performance-first-large/) for raw timings and gates, and [the profile diagnosis](no-regression-profiles/README.md) for the initial overhead. Per-run command records, input digests, verification results, machine data and binary hashes are retained. Source provenance is recorded in `baseline-source-sha256.json` and `candidate-performance-first-source-sha256.json`; the final build log is `performance-first-build.log`.

## Raw evidence archive

Raw per-run output, command records and sample tables are preserved in
[raw-evidence.tar.gz](raw-evidence.tar.gz), with counts, sizes and checksum in
[raw-evidence.json](raw-evidence.json). Every archived file was verified against
its original. From this results directory, run `tar -xzf raw-evidence.tar.gz`
to restore the raw paths referenced by the reports and analysis scripts.
Reports, aggregate summaries, validation logs and provenance remain directly
readable in Git. Existing unpacked data is retained locally and ignored by Git.
