# Dimension 5 — Weak types

Scope: Rust production and benchmark changes in `51b472c55..ef2508f3a`, reviewed at clean HEAD `ef2508f3adaf153b891825d1c2f6dc4a69853898`. Read-only static review; no builds, tests, installs, or source changes performed.

## Findings

No additional actionable weak-type findings beyond the `BridgeClaim::row_weights` invariant already assigned to the main reviewer. No high-, medium-, or low-priority findings to add.

## Evidence and validation

- Searched all ten changed Rust files for erased `Any` types, `serde_json::Value`, pervasive `String`, nullable booleans, and public mode/configuration boundaries. No erased or JSON-value domain types were introduced. Benchmark JSON remains at the output boundary.
- `FalconAlgebraicBinding` is a concrete public enum (`backend.rs:31`); `new_with_binding` accepts this enum and its shared constructor rejects partial-capacity word batches and unsupported security targets (`protocol.rs:151–170`). The private `word_binding` boolean represents that validated two-case choice. Replacing it alone would be stylistic and would not remove a demonstrated invalid public construction path.
- `BindingKernel` identifies the three benchmark kernel overrides; `[Option<bool>; 3]` stores the exact auto/on/off states and string parsing stays at the environment boundary (`binding_options.rs:5–39`). No additional configuration framework or enum is justified by this diff.
- Weighted bridge entrypoints validate row count, canonical row weights, column dimensions, and magnitude bounds before use (`integer_bridge.rs:269–297`, `:412–451`). Word verifier weights are reconstructed from transcript challenges by `word_binding::opening`; they are not supplied by the proof. No extra public validation defect was established.
- Inspected tests covering explicit relation selection, rejection of partial word batches, terminal/nonce/round-count mutation rejection, and cross-relation rejection (`protocol.rs:930–1043`). Tests were inspected, not executed.

## Dependencies

The main reviewer's `BridgeClaim::row_weights` cleanup may affect these same files; do not duplicate that work. This report neither blocks nor requires another dimension.

## Limits

This is a focused weak-type and adjacent boundary review, not an independent audit of the algebraic protocol or its soundness proof. No LSP replacement or type recovery was needed because no erased-type candidate survived the textual and caller review.
