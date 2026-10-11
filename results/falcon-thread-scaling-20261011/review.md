# Selected candidate: final source review

Reviewed the selected eight-file diff against `471e1b6c820d9002b61664db453d69afde395db9`. All source hashes in `restored-fused-source-manifest.json` match the current files. No blocking or material correctness findings remain. This note supersedes the earlier candidate-selection notes; no production source changes or workloads were performed for this final review.

The selected implementation preserves these reviewed invariants:

- Forest and dense-kernel macros give cyclic/fallback consumers distinct closures with unchanged arithmetic, row indices, disjoint ownership, dispatch gates, and initialization boundaries. Three x86 inlining attributes affect code generation only. The forest-test diff is formatting only.
- Fused packing exactly matches the historical early-drop files by SHA-256. Indexed jobs safely append final words, with adjacent duplicate words for the two-limb bridge. Transpose padding and row tails retain their original meaning; the restored independent bit oracle checks this representation. No new unsafe operation is added.
- Public-power batches retain adjacent image ordering and limb-major root weights. Parallel exact GF reduction is gated independently by length and worker count, with the original serial fallback. The verifier equality table is released immediately after root-claim calculation.
- Public-message factoring uses exact GF distributivity: sum selected instance weights, then multiply by the common bit weight. Fixed SHAKE padding sums only `instances[..messages.len()]`; inactive signatures remain excluded. The old per-signature multiplication oracle covers all public bits, mixed messages, nontrivial weights, and partial capacity.
- Six explicit drops release owned witnesses and claims after their final use. No transcript operation, proof encoding, challenge boundary, grinding schedule, or polynomial degree is changed.

Vendor scratch caching is unchanged from `471e1b6c8`. The bounded-fit, smaller-cache-count, caller-owned packing, and Drain-dispatch experiments are absent.

## Evidence and limits

The selected arithmetic binary SHA-256 is `8b9a8a43ca2a5b12fb509cc5661d1382c60c5aec9f26f1157891afba3ac1458b`, exactly matching the previously tested early-drop binary. Its evidence is therefore reused from the historical seeds 100..105 campaign, not presented as a new run: all 16 arithmetic metric cells pass, first-proof timing checks pass, and 16 workers outperform 8 (total/prove time approximately -2.164%/-1.161%). See `early-drop-holdout-independent.json` and `early-drop-first-proof-diagnostic.json`.

The selected full binary SHA-256 is `1eaaf10210ee2d7498bf114b027eb35edc972599a5f3beebe35a13731b3f98d0`. Its fresh seeds 180..185 campaign records 48 processes and 432 verified trials. Warmed and first-proof checks show no statistically established proving regression; full one-worker and eight-worker timing cells remain statistically inconclusive rather than passing strict nonregression gates. Sixteen versus eight workers improves total/prove time approximately 18.984%/19.743%. Full 16-worker peak RSS increases approximately 0.160%, within the user's accepted modest memory tradeoff. See `restored-fused-full-holdout-independent.json` and `restored-fused-full-first-proof-diagnostic.json`.

These are two explicitly identified evidence sets, not one fresh combined matrix, and not every metric has a statistical pass. Reported verification, proof-identity, and startup evidence is recorded in those audits. Qualification applies to the measured host/native build and Falcon-512 batch-1024 workload at 1/4/8/16 workers; it is not a universal performance guarantee.


## Completed test and build records

The table records existing completed logs. These checks belong to the listed source snapshots; they are not represented as one newly rerun suite. The selected implementation reuses those reviewed components, and final native proof-identity campaigns cover Falcon-512 at all requested worker counts.

All test invocations used `cargo test --offline --locked`, the configuration/filter below, and `-- --test-threads=1`. The initial full-profile filter that selected zero tests is excluded.

| Snapshot | Configuration and filter | Completed result | Log |
|---|---|---|---|
| Separate row closures | `--features falcon-hybrid --lib bitz::` | 76 passed | `.tmp/thread-fix-20261011/separate-parallel-tests.log` |
| Separate row closures | `--no-default-features --lib bitz::` | 72 passed | `.tmp/thread-fix-20261011/separate-serial-tests.log` |
| Fused packing | `--features falcon-hybrid --lib ligerito` | 33 passed, 1 ignored | `.tmp/thread-fix-20261011/packed-rows-packing-tests.log` |
| Fused packing | `--features falcon-hybrid --lib hybrid::integer_bridge` | 16 passed | `.tmp/thread-fix-20261011/packed-rows-bridge-tests.log` |
| Fused packing | `--no-default-features --lib ligerito::tests` | 8 passed | `.tmp/thread-fix-20261011/packed-rows-serial-packing-tests.log` |
| Public powers | `--features falcon-hybrid --lib hybrid::integer_bridge` | 17 passed | `.tmp/thread-fix-20261011/public-powers-bridge-tests.log` |
| Early drops | `--features falcon-hybrid --lib end_to_end_and_tampering` | 1 passed | `.tmp/thread-fix-20261011/early-drop-algebraic-test.log` |
| Early drops | `--features falcon-hybrid --lib shared_prime_hybrid_complete_proofs_and_protocol_binding` | 1 passed | `.tmp/thread-fix-20261011/early-drop-full-test.log` |
| Public-link helper, unchanged in selected source | `--features falcon-hybrid --lib public_initial_bit_weights_match_per_signature_products` | 1 passed | `.tmp/thread-fix-20261011/bounded-fit-public-link-tests.log` |
| Public-link helper integration snapshot | `--features falcon-hybrid --lib shared_prime_hybrid_complete_proofs_and_protocol_binding` | 1 passed | `.tmp/thread-fix-20261011/bounded-fit-full-test.log` |

The full-profile unit tests instantiate the shared Falcon-1024 profile. Selected native benchmark/proof-identity evidence separately covers the requested Falcon-512 degree. Development unit tests do not establish native SIMD execution; the native production binaries do exercise the selected host's SIMD path. Tests of rejected scratch-cache code are excluded from retained validation.

The selected native build completed using:

```sh
RUSTFLAGS='-C target-cpu=native' CARGO_BUILD_JOBS=8 \
CARGO_TARGET_DIR=/home/john-wu/code/BitZ-pcs/target/falcon-binder-native \
cargo bench --offline --locked --profile release --features falcon-hybrid \
  --bench falcon_algebraic --bench falcon_hybrid --no-run --message-format=json
```

The build log, Cargo artifact log, exact source hashes and frozen executables are in [source-build.json](source-build.json). The build reproduced the historical arithmetic executable byte-for-byte and produced the fresh full executable used in its final campaign. No tests, builds, instrumentation or statistical audits overlapped timed qualification processes.
