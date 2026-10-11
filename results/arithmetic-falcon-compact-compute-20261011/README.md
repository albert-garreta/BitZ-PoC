# Arithmetic Falcon witness and proving optimization

**R7 is qualified with no observed proving regression across 70 measured shapes.** Against the preserved v7 baseline, every shape improves in proof-only and total-prover medians. All 280 descriptive paired 95% intervals favor R7 (two metrics, median/geometric-mean estimators; no across-shape multiplicity correction). Native release results on AMD Ryzen 9 9950X3D, one/eight workers:

| Batch | Pooled median proof improvement | Paired median proof improvement |
|---:|---:|---:|
| 1 | 18.51–28.75% | 18.70–28.70% |
| 32 | 5.57–16.21% | 5.33–16.25% |
| 1024 | 14.61–25.45% | 14.52–25.51% |

The implementation trims padded arithmetic domains, chooses raw-binding strategies by public shape, and groups coefficient expansion on qualified prefix-4 paths. Batch-1 logical arithmetic bits fall by 96.875% for N512 and 93.75% for N1024; physical committed source allocation stays unchanged. Reduced-layout proof payloads shrink 1.24–4.63%. The tradeoff is up to 14.28% higher median process peak RSS in a grouped large-batch case. Arithmetic remains separate from HashToPoint/SHAKE, and exact integer norm arithmetic is preserved.

The [final R7 qualification report](r7-qualification/README.md) contains the complete 70-shape table, method, confidence intervals, payload/verifier/RSS results and limits. **All 7,552 proofs verified; all 2,496 unchanged-layout candidate proofs exactly matched v7.** All 1,280 reduced-layout candidate proofs used the expected v8 layout; physical bits, bytes and source roots matched v7 throughout. All 160 targeted correctness tests passed, including full Falcon. This is a measured arithmetic-performance qualification, not a universal guarantee across other hardware or worker settings.

[R7 provenance](candidate-r7-provenance.json), the [frozen source hashes](candidate-r7-source-sha256.json), and [post-build comment-only delta](candidate-r7-post-build-provenance.json) identify the qualified implementation. The final [fixed plan](r7-qualification-plan.json), [manifest](variants.r7-qualification.ready.json), [machine-readable summary](r7-qualification/qualification-summary.json) and [validation logs](validation/) are preserved.

## Preserved experiments and rejected candidates

The sequence below retains negative results and diagnostics. Earlier candidate verdicts remain historical evidence; only R7 is accepted. Experimental compact-binding/multiple-bridge computation failed the speed gate and remains disabled in production.

- Grouped pilots: [large](grouped-large/README.md), [boundaries](grouped-boundary/README.md), and [medium](grouped-medium/README.md). All 720 proofs verified; all 576 diagnostic candidate proofs exactly matched original v7. These supported the simple cap-16 policy subsequently qualified in R7.
- [R6 expanded production qualification](r6-qualification/README.md): all7,552 proofs verified and2,496 exact v7 comparisons passed across70 shapes. Six paired-interval concerns remain; no regression-free acceptance claim.
- [R6 focused check](r6-batch32/README.md): fresh seeds170–201 resolve the earlier batch32 regression before the broader run.
- [Broad same-binary strategy sweep](binding-sweep/README.md): all 64 shapes with two seeds, all 1,536 proofs verified and identical to earlier proofs; the [public-threshold policy proposal](binding-sweep/POLICY.md) selects actual computation gains for later production qualification.
- [Same-binary raw binding controls](binding-controls/README.md): a two-seed pilot finds real compute gains from the existing optimized binding strategy; all 192 proofs verify and exactly match original v7. It is not yet a production qualification.
- [Production proof-layout check](r5-batch32/README.md): fresh seeds 138–169, all 1,024 proofs verified and 512 exact v7 matches; all four proof/total paired intervals are positive. [R5 provenance](candidate-r5-provenance.json) preserves its binary/source snapshot.
- [Revised production focused check](revision-batch32/README.md): fresh seeds 106–137, all 1,024 proofs verified and 512 exact v7 matches. Three proof-time intervals remain positive; the full rerun was not started. [R4 provenance](candidate-r4-provenance.json) preserves its distinct binary/source snapshot.
- [Full 64-shape production qualification](qualification/README.md): all 7,168 proofs verified, all 2,304 unchanged proofs exactly matched v7, and logical-layout/physical-source audits passed. Paired analysis preserves every positive observation.
- [Independent unchanged-layout confirmation](confirmation-batch32/README.md): seeds 74–105, all 1,024 proofs verified and 512 exact v7 matches. The proof-time increase repeats in all four degree/security cases.
- [Selected production qualification plan](QUALIFICATION.md): retains the original 64 shapes/seeds and uses production binaries with only `falcon-hybrid`. The candidate enables padding reduction; experimental compact computation is disabled by default.
- [Revised 32-shape compact pilot](pilot-r2/README.md): all 768 proofs verified, 192 mode no-op comparisons and 240 exact v7 comparisons passed. Compact computation failed its speed gate.
- [Revised large-component diagnosis](pilot-r2-profiles/README.md): eight shapes, all 64 diagnostic proofs verified and matched the untraced pilot; binding and BitZ costs explain the compact failure.
- [Initial pilot](pilot/README.md) and [initial diagnosis](pilot-profiles/README.md): all earlier evidence remains intact.
- [Preserved v7/v5 controls](control-provenance.json), [initial candidate](candidate-pilot-provenance.json), [revised candidate](candidate-r2-provenance.json), and [production candidate](candidate-final-provenance.json).


Qualification uses 32 seeds for core batches 1/32 and eight for large/edge shapes, one warmup and three measured proofs per input, balanced sequential order, pinned one/eight workers, and no concurrent builds/tests/profiles. Paired intervals describe seed variability without across-shape multiplicity correction. Raw evidence, failed gates and unexecuted confirmation plans remain intact.

## Raw evidence archive

Raw per-run output, command records and sample tables are preserved in
[raw-evidence.tar.gz](raw-evidence.tar.gz), with counts, sizes and checksum in
[raw-evidence.json](raw-evidence.json). Every archived file was verified against
its original. From this results directory, run `tar -xzf raw-evidence.tar.gz`
to restore the raw paths referenced by the reports and analysis scripts.
Reports, aggregate summaries, validation logs and provenance remain directly
readable in Git. Existing unpacked data is retained locally and ignored by Git.
