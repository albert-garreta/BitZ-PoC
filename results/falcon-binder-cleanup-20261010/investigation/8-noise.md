# Dimension 8: comments, stubs, and unnecessary adapters

Scope: read-only audit of `51b472c55..ef2508f3a` in this worktree. Read the simplify-general, simplify-rust, and rust-reviewer skills. Reviewed the changed optimization source, protocol documentation, and experiment runner documentation. No builds or source edits were performed. `.tmp/simplify/8-noise.md` is ignored.

## Findings

1. **Low / suggestion — update two now-stale Debug comments while preserving the formatter.** `src/piop/spartan/falcon1024_algebraic/protocol.rs:1102-1103` says every nested proof derives Debug; `:1170-1172` similarly says “Derived Debug includes every proof field.” The range replaces the arithmetic proof's derive with a handwritten formatter at `arithmetic.rs:76-89`, intentionally omitting `terminal_merge_nonce` when absent. Those comments are no longer literally accurate and obscure the compatibility reason for the custom implementation. Retain the whole formatter and digest comparison. Replace the comments with accurate language such as “Debug includes the proof contents and nonces; absent word-merge nonces retain the legacy bit-proof formatting. This compares contents within one build, not a wire codec.” A short rationale immediately above the custom `Debug` impl is useful if the test comment remains brief. **Dependencies:** none; fold into the existing arithmetic/protocol cleanup rather than create a cosmetic sweep. **Evidence:** code inspection, no behavioral defect asserted.

## Reconciliation notes

- The one-caller `PreparedFalconAlgebraic::new_config` at `protocol.rs:159` is a real forwarding layer (`new_with_binding` only converts the enum to a bool). Root already owns its collapse; no separate dimension-8 action is needed.
- Mandatory owned weighted `BridgeClaim` normalization is the useful simplification for the bridge branching/clone adapters. Root already owns this; do not separately edit the current equality wrappers in `integer_bridge.rs`, whose old transcript behavior is documented and compatibility-tested.
- No TODO/FIXME/HACK/XXX, `todo!`, or `unimplemented!` stubs were found in the changed source, runner, or accompanying current documentation. No broad comment deletion is warranted.

## Preserve

- `word_binding.rs:106-109,128-129`: allocation reuse and read-before-write explanation makes the in-place fold reviewable.
- `word_binding.rs:3-5,208-209,238`: the sparse lane-15 representation, padding authentication, and omitted-coordinate derivation explain soundness invariants.
- Protocol relation/version/security/bounds rationale in `docs/falcon-word-binder.md` and the algebraic README. The default-mode, full-capacity, and v5 compatibility notes match the implementation.
- The weighted bridge's pending-PCS-obligation comments and mutation-test explanations; those prevent treating the bridge's return value as standalone authentication.
- `falcon_polynomial.rs:20-21`, `transcript/mod.rs:95-97`, and retained workload-policy comments: these explain the purpose and qualification of the optimizations.
- `scripts/binder_experiments.md` distinctions between Debug content hashes and portable serialization, subprocess replication and within-process iterations, and diagnostic timings and latency measurements.

No high- or medium-priority issue was identified in this dimension.
