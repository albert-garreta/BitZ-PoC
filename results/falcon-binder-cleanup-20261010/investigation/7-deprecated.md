# Dimension 7 — deprecated, legacy, and fallback paths

Scope: read-only review of `51b472c55..ef2508f3a`, with surrounding callers at `ef2508f3a`. No builds, installations, tests, or source edits were performed. This report uses source/reference searches and reads of archived experiment manifests.

## Findings

### Low priority / SUGGESTION: retire the unused group-cap experiment control if this campaign is complete

- Locations: `src/sumcheck/inner/packed/binding_options.rs:44` (`benchmark_group_cap`), its call at line 115; `scripts/binder_experiments.py:67` (accepted configuration key/default), line 87 (validation), line 268 (environment emission); `scripts/binder_experiments.md:62` and line 73 (configuration documentation/example).
- Evidence: the current runner exposes an arbitrary positive `group_cap`, and the benchmark feature can override the measured 8/16 policy through `BITZ_BENCH_BINDER_GROUP_CAP`. None of the inspected archived `results/binder-experiments-20261010/{screen,pilot-v2,holdout-v2,production,smoke,trace-v1,trace-v2-verifier}/manifest.json` configurations or jobs use a non-`auto` cap. The tracked holdout/production config files also do not request a cap. The current runner tests contain no group-cap references.
- This is reachable optional experiment functionality, **not dead code or a correctness defect**. Removing it narrows the benchmark interface; keeping it is reasonable if further grouping experiments are intended. If retired, remove the Rust override helper/call and the Python key/default/validation/emission together with the documentation example. Retain `BindingOptions::group_cap` and the production 8/16 policy: `src/sumcheck/inner/packed/streaming.rs:383` actively uses the cap for grouped replay.
- Dependencies: coordinate with dimension 3 (unused experiment surface) and dimension 8 (its documentation). This is independent of the already accepted `CompositeMultilinearExtension` removal. Do not rewrite archived evidence or mutate historical manifests to fit a revised runner.

### High priority / shared existing finding: delete the private CompositeMultilinearExtension family, not the compact ECDSA implementation

- This is the already confirmed parent/dimension-3 deletion, not a second independent recommendation. Search results locate the private type at `src/poly/mle/structured.rs:28`, its re-export at `src/poly/mle.rs:5`, and its packed adapter at `src/sumcheck/inner/packed/composite.rs:11`; remaining construction references are tests.
- `src/sumcheck/inner/packed.rs:367` still re-exports `CompactCompositeMle`; preserve that implementation and helpers used by it. Likewise do not remove `FactoredMultilinearExtension` or `EqualityWeights` wholesale just because they share `structured.rs` with the obsolete type.
- Dependencies: blocked by/reconciled into dimension 3; deletion wins over any dedup/type recovery proposed for the obsolete family. No additional rewrite is needed from this dimension.

## Retain: reviewed paths with live purposes

- **Bit v5 relation and its public selector.** `src/piop/spartan/falcon1024_algebraic/protocol.rs:139` still selects Bits for every default configuration except Falcon-512/batch-1024/security-128. `new_with_binding` at line 151 explicitly supports old v5 proof verification, and Words rejects partial capacity at line 165. The bit branch in arithmetic/protocol is therefore supported behavior, not a dead fallback. `docs/falcon-word-binder.md:6` accurately documents this policy.
- **Arithmetic reference construction/evaluation and replay fallback.** `src/piop/spartan/falcon1024_algebraic/arithmetic.rs:441` only qualifies selected batch/worker combinations, and `src/sumcheck/inner/packed/binding_options.rs:101` only qualifies selected platform/degree/security/worker combinations. The reference paths remain reachable for unqualified workloads and as test oracles. Renaming or deleting these paths is not justified by the word-binder introduction.
- **All three BindingKernel controls.** `src/sumcheck/inner/packed/binding_options.rs:5` supports Replay/Construction/Evaluation; `scripts/binder_experiments.py:40` generates their eight arithmetic combinations. Archived screening/holdout evidence contains the corresponding configurations, including `r000`, `r011`, and `r111`; retain these controls to reproduce and inspect the experiments. `BITZ_BENCH_WORD_BINDER` similarly supports the archived comparisons and explicit relation choice.
- **Equality-point bridge wrappers.** `src/hybrid/integer_bridge.rs:339` preserves `verify` while delegating to weighted internals. Existing full Falcon calls remain at `src/piop/spartan/falcon/hybrid.rs:688` and line 797; bit algebraic callers remain at `protocol.rs:470` and line 586. These wrappers preserve the existing transcript relation and are not removable compatibility clutter.
- **Historical screen/holdout writeups.** Statements that the first word implementation needed remeasurement are intentionally retained historical evidence; `results/falcon-binder-20261010/REPORT.md:97` explicitly distinguishes frozen V2 measurements from production results. Do not rewrite these as if every result describes final HEAD.

No additional deprecated path or stale fallback deletion was established in the changed implementation. No LSP/typecheck claim is made; those checks belong to the later serial implementation/validation phase if a removal is selected.
