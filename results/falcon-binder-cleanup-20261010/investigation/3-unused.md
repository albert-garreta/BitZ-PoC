# Dimension 3: unused code

Snapshot: `ef2508f3adaf153b891825d1c2f6dc4a69853898`, range `51b472c55..HEAD`. Worktree: `/home/john-wu/code/BitZ-pcs/.tmp/falcon-binder-experiments`. Read-only source investigation; no build, tests, installs, or source edits were performed. This report is the only written artifact. Read `simplify-general` and `rust-reviewer` from `/home/john-wu/code/ai-agent-army/claude/skills/`.

## High priority: remove the obsolete noncompact composite representation as one group

`CompositeMultilinearExtension` is crate-private, and all its constructors are in tests. Its sumcheck implementation is therefore unreachable from every production feature combination. Production ECDSA uses `CompactCompositeMle`, with an independent dense oracle already covering that implementation. The old composite's tests test the old representation itself; they are not an oracle consumed by the compact adapter or Falcon.

Evidence from a whole-workspace `.rs` symbol search (including ignored source directories but excluding generated target/.tmp/git; vendor additionally checked):

| Symbol/use | Current locations | Reachability |
| --- | --- | --- |
| `CompositeMultilinearExtension` definition and implementation | `src/poly/mle/structured.rs:28`, `:160` | Crate-private |
| Re-export | `src/poly/mle.rs:5` | Crate-private |
| Its `InnerSumcheckMleSource` implementation | `src/sumcheck/inner/packed/composite.rs:11` | No production constructor |
| Construction in structural MLE tests | `src/poly/mle/structured.rs:396`, `:422` | `#[cfg(test)]` module starts at `:341` |
| Construction in packed tests | `src/sumcheck/inner/packed.rs:3094` | Noncanonical-input regression test |
| Construction in composite tests | `src/sumcheck/inner/packed/composite.rs:779`, `:782`, `:838` | `#[cfg(test)]` module starts at `:720` |

No benches, examples, integration tests, workspace crate, or vendor source refer to this representation. There are no public APIs that expose it. The replacement production path is:

`ecdsa_sha256/proof.rs:273` -> `sumcheck/bridge/composite.rs:25` (`CompositeCoefficients::as_mle`) -> `CompactCompositeMle::new` -> `PackedInput::new` at `ecdsa_sha256/proof.rs:283` -> `packed/state.rs:56` / `:92` -> compact prefix and tail methods. `sumcheck/bridge/mod.rs:24` gates the bridge adapter on `ecdsa`.

Exact removable group, using snapshot lines:

1. `src/poly/mle/structured.rs:26–42`: old composite type; `:160–258`: its entire impl; `:393–414` and `:421–423`: clauses that only test it. Preserve the rest of the structured-MLE tests and `FactoredMultilinearExtension`.
2. `src/poly/mle.rs:5`: remove just the old composite re-export.
3. `src/sumcheck/inner/packed/composite.rs:9–234`: old type import and trait impl.
4. The same file `:248–317`: `tail_pieces`; callers exist only in the removed old trait impl (`:76`, `:161`).
5. The same file `:430–526`: `tail_run_beta_values`; its only caller is the removed old trait impl (`:77`). The compact adapter implements chunk iteration directly and never calls this function.
6. The same file `:746–918`: `structured_tail_matches_generic` and `composite_prefix_matches_generic_with_constant_tail_and_boundary`, which exercise only the old representation. Remove the resulting unused `Blake3Transcript` test import at `:723`. Keep `ternary_extension_table_matches_extend_lsb` (`:727`), which tests a shared production helper.

Preserve the purpose of `packed.rs:3081` before deleting the type: retarget `noncanonical_structured_inputs_are_rejected_before_the_transcript` to the live `FactoredMultilinearExtension::with_leading_value` adapter, injecting the noncanonical value separately into outer factor, inner factor, and leading value. `validate_factored_mle` at `packed.rs:456` validates all three. The existing test's `PackedInput` live length must change from 6 to the chosen factored live length (e.g. 5 for 2x2 factors plus a leading value). This keeps the important rejection-before-transcript assertion on a production implementation. Compact ECDSA-origin validation may additionally be covered by retargeting one fixture to `CompactCompositeMle::new` under `ecdsa`; its constructor calls `validate_shape` at `compact.rs:36`, which validates origin at `:108`.

After removal, gate `mod composite` at `packed.rs:365` with `#[cfg(any(test, feature = "ecdsa"))]` so Falcon/default library builds do not compile ECDSA helpers without consumers. This preserves the ternary test under default unit tests. Do not put `#[allow(dead_code)]` on the helpers or delete the production helpers to silence warnings.

Dependencies: blocks any dedup/type cleanup of these obsolete pieces; deletion should win over consolidation. Requires adapting the live noncanonical regression test first/in the same patch. Main agent must validate the default/Falcon and ECDSA feature paths after applying. The main agent subsequently supplied rust-analyzer `findReferences` evidence in `.tmp/simplify/raw/lsp-references.json`: 16 references for `CompositeMultilinearExtension`, 2 for `tail_run_beta_values`, and 3 for `tail_pieces`, agreeing with the textual trace. The main agent also completed its curated `falcon-hybrid` Clippy check (`raw/clippy-dedup.txt`), which confirms the old composite is never constructed. I did not run those checks myself.

## Shared composite helpers that must remain

The compiler warnings from a non-ECDSA production build are not evidence that these helpers are dead:

| Keep | Definition in `packed/composite.rs` | Production consumer in `packed/composite/compact.rs` |
| --- | --- | --- |
| `TailPiece` | `:241` | constructed at `:177`, consumed at `:195` |
| `TailShapeState` and `new` | `:367`, `:373` | `:155`, `:188`, `:207`, `:220`, `:224` |
| `accumulate_tail_block` | `:391` | `:195` |
| `finish_tail_shapes` | `:528` | `:225` |
| `shape_index` | `:385` | transitively used by `accumulate_tail_block` |
| `RepeatedState` | `:598` | transitively used by `repeated_beta_values` |
| `repeated_beta_values` | `:611` | `:125` |
| `ternary_extension_table` | `:323` | transitively used by repeated and tail finish helpers |

Keep the compact adapter's two tests at `compact.rs:449` and `:508`. The latter independently constructs integer tail values and tensor products (`:557–566`), compares all evaluation entries, checks folded evaluations, and compares full proofs to the generic closure path for prefix widths 0..4 (`:596` onward). It tests multiple moduli, aligned and unaligned widths, sparse corrections, and a long tail crossing chunks. These tests do not depend on `CompositeMultilinearExtension`, so deleting the old group preserves the active implementation's oracle.

## Medium priority: group-cap override is unmeasured scaffolding, not unreachable code

`benchmark_group_cap` (`binding_options.rs:44–63`) is called under `bench-internals` at `:115`; it is therefore not dead code. Its configuration flow is fully wired:

`scripts/binder_experiments.py:67–88` accepts and validates `group_cap`, `:268` exports `BITZ_BENCH_BINDER_GROUP_CAP`, `binding_options.rs:50` reads it, and `streaming.rs:383` uses the resulting cap for grouped work. `scripts/binder_experiments.md:62` documents it and `:73–74` gives a non-auto example.

However, no archived campaign configuration or artifact in `results/falcon-binder-20261010/` contains `group_cap` or `BITZ_BENCH_BINDER_GROUP_CAP`, and none of `default_configurations()` (`scripts/binder_experiments.py:35–50`), holdout configs, or production configs selects a non-auto cap. All archived campaigns therefore use the default. There is no measurement evidence in this snapshot for retaining the override. It does not affect production builds.

Given the user's request to remove experiment scaffolding that did not contribute to the measured optimization, removing this knob is justified as a deliberate simplification, not as a proven-unreachable deletion. If selected, remove the helper (`:44–63`) and call (`:114–115`) together with runner allow-list/default/validation/export entries and the documentation option/example. Preserve `BindingOptions.group_cap` itself and its degree/security-based values; those drive the retained production grouping, including full Falcon, and have callers in `streaming.rs:383` and production `falcon/opening_compact.rs`.

Dependencies: coordinate with the benchmark/deprecation dimension so docs and runner stay consistent. Historical frozen artifacts should remain untouched. Retaining old runner config parsing for `group_cap="auto"` may be appropriate if the main agent finds archived external resume manifests outside this snapshot; no such dependency is present in the repository evidence checked here.

## Retain the actual reference and optimization paths

- `BindingKernel::{Replay,Construction,Evaluation}` all have production-policy and benchmark callers (`binding_options.rs:108`; `arithmetic.rs:468`, `:586`). The runner's default 2^3 arithmetic screen and full replay comparison deliberately exercise every switch (`binder_experiments.py:40–47`); removing them breaks reproducibility of the measured ablation.
- `BindingStrategy::Legacy` is reachable for unqualified worker/batch shapes (`arithmetic.rs:441–452`) and through benchmark overrides, and provides the equivalence oracle in `factored_construction_and_batched_evaluation_match_legacy` (`:1270`). Its construction/evaluation branches are not abandoned experiments.
- The v5 bit binder remains production behavior for all defaults outside N=512/batch=1024/security=128 (`protocol.rs:139–145`) and remains publicly selectable through `new_with_binding(..., Bits)` (`:151`). Keep its transcript/proof paths and `PROTOCOL_ID`.
- Every production function in newly added `word_binding.rs` has a concrete consumer. `arithmetic.rs:144` / `:226` call prove/verify, which reach dense/coefficient folding, slack handling, evaluation and opening. Its dense oracle at `word_binding.rs:568` is test-only by design and validates the selected v6 optimization (`:614`).
- Keep both integer-bridge wrappers and weighted entry points. Bit proofs call `bridge::prove`/`verify` at `protocol.rs:470`, `:586`; word proofs call weighted variants at `:459`, `:571`. The full Falcon path also calls the wrappers (`falcon/hybrid.rs:688`, `:797`). The unsplit bridge remains selected for full Falcon at 100-bit security (`falcon/hybrid.rs:312–315`).
- Keep transcript reference absorption in the new regression test (`src/transcript/mod.rs:179`): it independently verifies exact framing and challenges of the selected small-message optimization. It is not production residue.

No additional orphaned private definitions were found in the changed Rust files after tracing low-reference-count symbols across the workspace. This is a source reachability conclusion, not a claim that tests or feature builds passed; those checks belong to the main agent.
