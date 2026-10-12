# V4 stopped qualification

V4 is **not qualified**. The watchdog stopped after 61 matched pairs and 488 verified proofs; all 60 Native/100/batch-1 seed pairs and the first batch-3 pair are retained. Both primary timing bounds exceeded the unchanged 1.02 limit. No samples were excluded.

[Canonical report](qualification/REPORT.md), [summary](qualification/summary.json), [final integrity manifest](qualification/manifest.json), [stop marker](qualification/STOP).

| Native / 100 bits / batch 1 | Baseline mean, ms | Candidate mean, ms | Ratio | One-sided 95% upper | Status |
|---|---:|---:|---:|---:|---|
| total_prover_ms | 21.925679 | 22.313791 | 1.017701 | 1.028277 | inconclusive |
| proof_verify_ms | 14.089142 | 14.412671 | 1.022963 | 1.040531 | inconclusive |
| witness_commit_ms | 0.812492 | 0.808290 | 0.994828 | 1.004980 | diagnostic |
| proof_prove_ms | 21.109427 | 21.500273 | 1.018515 | 1.029189 | diagnostic |

Mean payload fell from 169229.867 to 146942.400 bytes (13.170% smaller). This size gate passed.

All global, auxiliary, main, and actual private verifier masks passed. All proofs verified and matched the expected input/root/security/grinding invariants. Final integrity checks rehashed both executables, both frozen source manifests, and all 240 cached fixtures successfully.

Cold and stage diagnostics completed all 16 pairs each. The V4 candidate produced exactly the same stored proof digests, payloads, roots, input digests, security terms, and grinding records as V2 across all 16 cold cases. [Exact equivalence](cold/v2-equivalence.json).

The following seed-42 stage measurements are single cold instrumented observations. They do not isolate the source of the 60-seed timing difference.

| Stage | Baseline, ms | Candidate, ms |
|---|---:|---:|
| falcon_hybrid:arithmetic_prefix | 10.659420 | 10.844221 |
| falcon_hybrid:binary_bridge | 4.156439 | 4.189853 |
| falcon_hybrid:keccak_prefix | 12.459268 | 12.197643 |
| falcon_hybrid:joint_sumcheck | 1.882053 | 1.813854 |
| falcon_hybrid:shared_opening | 1.582006 | 1.589219 |
| op:ligerito | 0.875087 | 0.873934 |
| falcon_bench:verify | 14.986031 | 15.081853 |

No new measurements or production changes were made after stopping V4. The earlier V1/V2 stopped campaigns and unmeasured V3 builds remain preserved.
