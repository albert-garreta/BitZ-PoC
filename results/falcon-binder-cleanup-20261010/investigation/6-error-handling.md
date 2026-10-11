# Dimension 6: error handling

Reviewed `51b472c55..ef2508f3a` as one optimization change, with surrounding callers and tests. Read-only source review; no builds, installations, or tests run. The report is the only write. Applied simplify-general, simplify-rust, and rust-reviewer/error-contract guidance from `/home/john-wu/code/ai-agent-army/claude/skills/`.

No demonstrated fail-open path, swallowed proof-validation error, or unsafe newly reachable unwrap was found in the scoped changes. The actionable items below are structural simplifications, not security defects.

## Findings

### Medium — consolidate the already-planned mandatory bridge-claim representation

- Locations: `src/piop/spartan/falcon1024_algebraic/arithmetic.rs:109-115,143-145`; `src/piop/spartan/falcon1024_algebraic/word_binding.rs:509-514`; `src/piop/spartan/falcon1024_algebraic/protocol.rs:581-585`.
- Evidence: the word opening always constructs `row_weights: Some(row_weights)`. Its prover immediately clones and unwraps those weights; its verifier retrieves them with `ok_or_else(|| error("missing word binder weights"))`. This error describes an internally impossible state under the current constructor/caller relation, rather than malformed serialized proof input.
- Recommendation: let the parent agent's already-planned claim/type consolidation remove the optional field, redundant ownership, and impossible-state error together. Do not independently replace the prover unwrap with a fallible error: that retains the representation problem. Preserve reconstruction of the weights from public data and transcript challenges.
- Dependencies: **blocked by dimensions 2/5** (claim representation); coordinate with dedup/ownership changes. No separate patch needed in this dimension.

### Low — replace the coefficient-table ownership sentinel with `mem::take`

- Locations: `src/piop/spartan/falcon1024_algebraic/word_binding.rs:304,328-337`.
- Evidence: `coefficient_tables` starts as `Some(form.coefficients)`; the coefficient-round branch borrows it via `as_mut().unwrap()`, and exactly one transition at `round == COEFFICIENT_LOG` consumes it via `take().unwrap()`. Later rounds never access it. The monotonically increasing round counter proves both unwraps safe.
- Optional simplification: hold `[Vec<F>; 2]` directly, pass `&mut coefficient_tables` to `fold_coefficients`, and use `let [s1, s2] = std::mem::take(&mut coefficient_tables)` at the side transition. This removes both unwraps and the `Option` without retaining the large allocations past the transition; the replacement is two empty vectors.
- This is **not** a panic bug. Keeping the existing sentinel is also defensible; if kept, explanatory `expect` text is a possible small clarity change. Do not add fallible errors or build an enum/state framework for this local invariant.
- Dependencies: none; can be applied after any word-binder structural edits.

## Checks and invariants to preserve

- Keep `SlackTable::new`'s scan/rejection of nonzero padding at `word_binding.rs:208-222`. Compact storage omits those source lanes; removing the check would silently exclude malicious source data from the prover's relation. The verifier's opening includes all lane-15 bits through weights at `477-514`.
- Keep full-capacity checks at `protocol.rs:175-177` and word `prove`/`verify` (`word_binding.rs:272-274,383-385`). These define the supported relation and establish the power-of-two shape used by folding. The internal checks also support direct unit testing.
- Keep word terminal field validation, equation check, round-boundary validation, and terminal-merge nonce verification at `word_binding.rs:391-417`. In particular, `terminal_merge_nonce: Option<u64>` is legitimately optional according to the grinding schedule; it is unlike the internally mandatory word weights.
- Keep the bit verifier's rejection of a word-terminal nonce at `arithmetic.rs:226-228`; proof relation separation is not a redundant error branch.
- Keep row-weight length/canonicality, modulus, shape, magnitude/read-off, forest-length/trailing-message, and grinding checks in `integer_bridge.rs`. Weighted verification obtains weights from its caller's reconstructed claim, not proof-supplied weights. New weighted entry points need their own validation even when old equality-weight entry points establish some related invariants.
- Existing `integer_bridge.rs` equality-table `expect` calls are on points whose shapes are locally constructed or validated. Test unwraps remain appropriate. No mass unwrap rewrite is justified.
- Benchmark-only switches in `src/sumcheck/inner/packed/binding_options.rs:15-60` deliberately reject invalid environment values. `.parse().ok().filter(...).expect(...)` combines invalid/zero values into the explicit configuration failure; it does not silently select `auto`. Replacing this with an `unwrap_or` fallback would change behavior. The bench relation switch similarly returns an error for unknown values (`benches/falcon_algebraic.rs:90-106`).
- `scripts/binder_experiments.py:138-143` adds file/line context while preserving the original JSON exception as the cause, and its final catch exits with failure. Trial verification, process exit checks, raw-output hash checks, and equivalence checks remain fail-closed; they are real I/O/data boundaries, not defensive noise to delete.

## Existing validation evidence to retain

- `word_sumcheck_matches_dense_two_product_oracle_and_bit_opening` checks independent dense round coefficients, nonces, terminal claims, and source opening; mutation cases reject altered terminals and missing nonces (`word_binding.rs:613-715`).
- `word_source_mutations_and_partial_batches_are_rejected` covers committed coefficient/sign/slack/padding mutations and unsupported partial batches (`word_binding.rs:717-758`).
- `weighted_bridge_authenticates_canonical_public_patterns` exercises both bridge modes and rejects noncanonical/incorrectly sized weights and changed source obligations (`integer_bridge.rs:860-1035`).
- These are tests inspected, not tests executed in this pass. Existing focused tests should validate an applied ownership/type cleanup; adding implementation-mirroring tests solely for the safe `mem::take` substitution is unnecessary.
