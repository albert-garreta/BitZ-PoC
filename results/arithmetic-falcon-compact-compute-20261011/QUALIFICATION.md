# Selected production qualification

The R1/R2 diagnostic pilots rejected compact binding/bridge computation because several configurations became slower. The shipping candidate therefore enables arithmetic-padding reduction and retains the original binding/bridge computation. Compact computation remains an explicitly selected benchmark experiment. This selection was fixed before this qualification; failed pilot results remain intact.

The [qualification plan](qualification-plan.json) preserves all 64 shapes and every seed in the original [fixed plan](fixed-plan.json). The only experimental change is reducing four variants to the preserved `v7` control and the selected `padding` production candidate. Both binaries use only `falcon-hybrid`, native release settings, locked/offline dependencies, and `RUSTFLAGS=-C target-cpu=native`. No benchmark phase-control environment variable selects the production path.

Each variant covers 896 input cases, with one warmup and three measured proofs per case: 3,584 proofs per variant, 7,168 total. Core-small shapes use seeds 42–73; core-large and boundary/tail shapes use 42–49. Variant order alternates forward/reverse by seed, balancing timing order within every shape. CPU affinity and timing isolation are unchanged from [PLAN.md](PLAN.md).

The fixed identity expectation is 2,304 exact candidate/control proof comparisons. These cover every shape whose logical capacity already meets the original floor. The other 1,280 candidate proofs use reduced arithmetic capacity and the new protocol, so equality to v7 is not required. Matching input hashes and successful verification are mandatory for every proof, including warmups. Identity checks compare the full Debug proof digest, source root, payload, input, protocol, trial, iteration, and committed physical source size.

Primary acceptance metrics are proof-only and total proving time, reported independently for each of the 64 shapes. Reports also preserve payload, verification time, process peak RSS, physical source size, and logical arithmetic extent. Any positive median is retained in the zero-threshold observed gate; seed-paired bootstrap intervals and, if necessary, separately fixed confirmation runs distinguish repeatable changes from noise. A smaller witness alone does not qualify a slower implementation.

The [pending manifest](variants.qualification.pending.json) cannot run until root supplies and confirms the frozen production binary. Timings also wait for explicit confirmation that correctness tests/builds have finished. Once ready, the whole plan runs in a fresh `qualification/` directory with the existing harness:

```sh
python3 results/arithmetic-falcon-compact-compute-20261011/benchmark.py \
  --plan results/arithmetic-falcon-compact-compute-20261011/qualification-plan.json \
  --variants results/arithmetic-falcon-compact-compute-20261011/variants.qualification.ready.json \
  --output results/arithmetic-falcon-compact-compute-20261011/qualification \
  --max-prover-regression-percent 0
```

Paired-seed analysis runs only after all timings finish:

```sh
python3 results/arithmetic-falcon-compact-compute-20261011/analyze_paired_seeds.py \
  results/arithmetic-falcon-compact-compute-20261011/qualification/comparisons/padding-vs-v7
```

The production example's timing boundaries were audited before qualification. Reusable preparation is separate. External signature generation, decoding, HashToPoint for benchmark input construction, and input hashing finish before the proof trials. Per-trial statement/coefficient clones precede the total-prover timer, matching the preceding example convention. Total proving covers `commit_from_s2` plus `prove`; verification has its own timer. Full proof Debug hashing, RSS reads, layout/size diagnostics and JSON output occur after all timed phases. The example's external HashToPoint input construction does not add HashToPoint to the arithmetic relation.

The first production qualification is complete: 7,168 verified proofs and 2,304 exact unchanged-v7 comparisons passed. Sixteen reduced-layout shapes consistently improve. Twenty unchanged shapes had a positive lower endpoint in either paired-median or paired-geometric-mean 95% intervals for at least one proving metric, so [confirmation-plan.json](confirmation-plan.json) fixes fresh seeds 74–105 for that union. Before spending the full confirmation cost, root approved the fixed [four-shape batch-32 subset](confirmation-batch32-plan.json). All 1,024 confirmation proofs verified and all 512 exact identities passed, but proof-time overhead repeated in every shape (paired medians +0.35% to +0.60%, all lower interval endpoints positive). The current binary therefore fails the requested performance bar. The full 20-shape confirmation plan remains preserved and unexecuted; a code fix is needed before further qualification.
## Final R7 outcome

The preceding sections preserve the first production attempt and its failure. Subsequent R4/R5 attempts also failed; R6 improved medium arithmetic but retained large/boundary concerns. The accepted R7 implementation adds grouped raw-binding coefficient expansion and uses the fixed [70-shape extension](r7-qualification-plan.json), retaining every original shape, seed and timing convention. [Final R7 qualification](r7-qualification/README.md) verifies 7,552 proofs, 2,496 exact unchanged-v7 identities, zero positive proving observations and all 280 descriptive paired intervals below zero. All 160 targeted correctness tests passed before its production-only timing run. The [top-level report](README.md) links every preserved failed attempt and diagnostic.
