# V6 stopped qualification

V6 is **not qualified**. The unchanged gate stopped the campaign after 301 matched pairs and 2,408 verified proofs. All four Native/100 cells passed. The Native/128/batch-1 prover confidence bound exceeded 1.02; its verifier and payload gates passed. One Native/128/batch-3 pair is retained. Every sample remains available.

[Canonical report](qualification/REPORT.md), [summary](qualification/summary.json), [final integrity manifest](qualification/manifest.json), [stop marker](qualification/STOP).

| Complete 60-seed cell | Prover ratio | Prover upper 95% | Verifier ratio | Verifier upper 95% | Payload ratio |
|---|---:|---:|---:|---:|---:|
| native/100/batch 1 | 0.986955 | 0.997094 | 0.971291 | 0.985816 | 0.868301 |
| native/100/batch 3 | 0.986722 | 1.000937 | 0.966846 | 0.986188 | 0.863714 |
| native/100/batch 32 | 0.996758 | 1.000758 | 0.982214 | 0.985329 | 0.893787 |
| native/100/batch 1024 | 1.000078 | 1.002307 | 0.979566 | 0.981881 | 0.946070 |
| native/128/batch 1 | 1.031210 | 1.050534 | 0.982486 | 0.986336 | 0.862536 |

The Native/128/batch-1 arithmetic means of per-seed medians are:

| Measurement | Baseline, ms | Candidate, ms | Ratio |
|---|---:|---:|---:|
| total_prover_ms | 32.774734 | 33.797625 | 1.031210 |
| proof_verify_ms | 12.979821 | 12.752489 | 0.982486 |
| witness_commit_ms | 0.754660 | 0.792138 | 1.049662 |
| proof_prove_ms | 32.015691 | 33.003513 | 1.030854 |

All observed global, auxiliary, main, and private verifier CPU masks passed. Every proof verified, with expected input digests, source roots, security terms, and grinding diagnostics. Final integrity checks rehashed both executables, both frozen source manifests, and all 240 cached fixtures successfully. The benchmark lock was released after the final checks.

Cold and stage diagnostics completed all 16 pairs each. Every V6 candidate cold proof exactly matched the V5 candidate in stored proof digest, payload, source root, input digest, security ledger, grinding records, and protocol. [Exact equivalence](cold/v5-equivalence.json).

V6 differs from V5 only in the compile-time PIOP schedule; no grinding implementation or policy changed. The baseline executable, compiler, CPU placement, harness, fixture set, sampling, and acceptance thresholds stayed unchanged. [Source audit](../v6-source-audit.json).

The campaign continued through a conversation interruption. Its original benchmark process and output files remained active; no build or qualification process was restarted. After the UI stream handles became unavailable, bounded file polling monitored the existing manifest/summary and requested the cooperative stop when the completed cell failed. The process finished its active pair and performed its normal final integrity checks.

No V6 attribution profile or unchanged qualification retry was run. Earlier V5 profiles remain diagnostic evidence about V5 and do not establish a cause for this V6 result. No further measurements are running.

A subsequent read-only comparison found that per-seed slowdowns do not reproduce across V5/V6, while both exhibit substantial speedup across the three measured trials. This does not alter either qualification result. [Retained-data comparison](V5-COMPARISON.md).

A separately authorized four-process frequency diagnostic observed reported CPU frequency rising from roughly 4.4 to 5.4 GHz as measured prover latency fell from roughly 38 to 31 ms. Policy settings were unchanged; all 16 diagnostic proofs verified. This association does not establish exclusive causality or alter the qualification failure. [Frequency diagnostic](frequency-abba/README.md).
