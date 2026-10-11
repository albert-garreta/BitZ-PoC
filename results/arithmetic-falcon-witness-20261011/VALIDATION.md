# Arithmetic Falcon witness optimization validation

The arithmetic-only Falcon-512 and Falcon-1024 relation remains public `(h,t)` with exact supplied private `(s1,s2)`, a negacyclic ring equation, and the integer norm bound. SHAKE-256 and HashToPoint remain outside this API. Both original signed15 storage at the PCS floor and compact split storage above it preserve the supplied values without centering or truncation. The arithmetic protocol domain is v7; older arithmetic proofs are incompatible.

## Correctness checks

The final release checks passed: **118 tests, zero failures**.

- Arithmetic suite: **83 passed**. The two large qualifications ignored by default were then run explicitly.
- Large qualifications: **2 passed**, covering both Falcon degrees, batches 32/1024, and security targets 100/128.
- Shared integer bridge and binary sumchecks: **32 passed**.
- Full Falcon regression `n1024_k11::hybrid::tests::hybrid_falcon_roundtrip_and_tampering`: **1 passed**.

Builds used `--locked --offline --release --features falcon-hybrid`, `RUSTFLAGS='-C target-cpu=native'`, `RAYON_NUM_THREADS=8`, and `CARGO_TARGET_DIR=/tmp/bitz-arithmetic-baseline-target`. Logs are retained in [the arithmetic run](release-arithmetic-tests.log), [large cases](release-large-tests.log), and [shared/full Falcon run](release-shared-full-tests.log). These correctness checks ran after all performance measurements and analysis finished.

The [final source audit](performance-first-final-source-check.json) confirms all 594 captured source/build-input hashes still match the benchmarked candidate.

Coverage includes independent schoolbook negacyclic references, exact signed15 reconstruction, maximum exceptions, integer norm boundaries, noncentered coefficients, changed public inputs, malformed metadata and proofs, inconsistent source bits, cross-signature norm borrowing, and complete proof/transcript agreement across packing strategies and worker counts. The minimum-size negative test successfully commits deliberately malformed padding before requiring the proof constraints to reject it; correctness therefore does not depend on a constructor padding scan.

The new parallel marginal test compares against independent direct bit sums for seven input shapes, including odd block/row counts, across one, three and eight workers. Carry accumulation uses an independent coordinate-major `i128` reference, signed15 boundary values and a 129-signature partial chunk across one, two and eight workers. Parallel packing similarly checks a final partial chunk and exact exception indices.

The unchanged final vendored field files were tested separately in portable and optimized native builds: **14 passed, one pre-existing ignored test in each build**. This includes exhaustive SIMD reduction over all 46,081 permitted raw sums, fixed-basis references, randomized scalar/SIMD comparisons, extremes, malformed inputs, and guard sentinels. Source hashes match the tested copies:

- `q12289.rs`: `b634cd4f3622fa458997380e58dcf0d8706f152f0d1f74b5dcfdf723a9094049`
- `q12289/avx512.rs`: `86f0b2f25dfba29ef3ed5afdfb7aaa93ac63c60b542d0a127e230d630169e901`

Two independent read-only reviews found no blocking issues in the final minimum-size layout or the parallel reductions. The reviews checked exact value preservation, transcript mode binding, exception validation, padding soundness, task coverage, partial chunks and integer accumulator bounds.

`git diff --check` passes. A targeted `rustfmt --check` reports only two existing formatting sites, confirmed unchanged from `HEAD`: the `partition_point` calls in `joint_sumcheck.rs` and the compact conditional in `arithmetic.rs`. Those unrelated lines were preserved; the new and changed code introduces no additional reported formatting differences.

## Reproduction

```sh
export RUSTFLAGS='-C target-cpu=native'
export RAYON_NUM_THREADS=8
export CARGO_TARGET_DIR=/tmp/bitz-arithmetic-baseline-target
cargo test --locked --offline --release --features falcon-hybrid --lib algebraic -- --test-threads=1
cargo test --locked --offline --release --features falcon-hybrid --lib large_batches_at_both_security_targets -- --ignored --test-threads=1
cargo test --locked --offline --release --features falcon-hybrid --lib -- hybrid::integer_bridge hybrid::joint_sumcheck hybrid::sumcheck hybrid_falcon_roundtrip_and_tampering --test-threads=1
```

## Performance qualification

All **24 configurations pass both strict observed-median proving gates**. The fixed matrix covers Falcon-512/1024, security 100/128, batches 1/32/1024, and one/eight threads. It contains **3,456 measured proofs and 1,152 warmups**, all verified; every paired input digest matches. All **768 Split proofs** match the preceding qualified candidate exactly. The **1,536 Single proofs** use the intended new floor layout and pass input, verification, trial, protocol-ID and physical-size checks.

Proof-generation medians improve 0.213–4.074%, total proving improves 0.184–3.051%, and payload medians shrink 2.448–21.168%. Verification is mixed, from 1.327% faster to 2.434% slower. Tiny timing differences remain subject to input and machine variation; passing observed gates is not a universal speed guarantee. Large compact sources retain the 18.75% physical witness reduction, while minimum-size sources retain their original bit counts to avoid unnecessary compression work.

See [the full benchmark report](performance-first-report.md) for every timing, paired-seed uncertainty, raw artifacts and provenance. Earlier failed candidates and their reports remain preserved.
