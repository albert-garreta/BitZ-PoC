# Dimension 2: types and ownership

Reviewed `51b472c55..ef2508f3adaf153b891825d1c2f6dc4a69853898` in the clean experiment worktree. Read the Rust reviewer, readability/protocol references, and type-safe skill. Read-only investigation: no source edits, builds, tests, installs, or measurements. Line numbers below refer to that snapshot. No demonstrated correctness defect found in this dimension.

## 1. SUGGESTION — highest priority: make the bridge claim uniformly weighted

**Locations:** `src/piop/spartan/falcon1024_algebraic/arithmetic.rs:110-115,132,143-156,191-207,1092-1107`; `word_binding.rs:20-25,477-514`; `protocol.rs:444-482,562-598`; supporting implementation `src/hybrid/integer_bridge.rs:232-255,339-395`.

**Evidence and cost:** `BridgeClaim.point` means a full row-plus-column point for Bits but a column-only point for Words, with `row_weights: Option<Vec<u128>>` acting as an implicit discriminator. Arithmetic proving then returns a separate row-weight vector in addition to the claim. Words clones the claim's vector and unwraps it at arithmetic.rs:145. Layout fixes `row_vars()` to 13 (`source.rs:97-99`), so this is a demonstrated extra allocation/copy of 8192 u128s (128 KiB) per Words proof. The original copy remains inside `claim` even after `drop(row_weights)` at protocol.rs:482. Verifying rechecks the representation indirectly from `schedule.word_binding`, and has an impossible-with-current-constructors missing-weights error at protocol.rs:580-584. These are design/ownership costs, not an observed panic or soundness defect: the Words opening constructor always stores `Some`.

**Smallest coherent correction:** represent the pending obligation as:

```rust
struct BridgeClaim {
    column_point: Vec<F>,
    row_weights: Vec<u128>,
    modulus: u128,
    value: F,
}
```

Return `(Proof, BridgeClaim)` from arithmetic proving. The Words constructor already has precisely this information. For Bits, keep `bind_opening`'s existing domain, modulus, complete point, and value absorption in the exact existing order; only afterward compute the equality row weights from the row prefix and retain the column tail. Reuse the point vector by draining its prefix if desired. Deriving weights performs no transcript operation. Both arithmetic prove and verify call this same helper. Change the two protocol bridge call sites to unconditional `bridge::prove_weighted` / `bridge::verify_weighted`, and drop the claim after the bridge so its vector does not stay live during joint opening.

This removes the Option, clone/unwrap, separate prover return vector, and both repetitive mode branches without introducing an enum, new trait, or generalized proof framework. Point shape and canonical-weight validations remain in the existing weighted bridge entry points. Proof relation selection remains in the arithmetic binder and transcript preparation.

**Compatibility proof from code:** the old Bits `bridge::prove` wrapper only validates total point length, slices away the row prefix, and invokes `prove_weighted` (integer_bridge.rs:242-254). The old Bits verifier derives the same equality weights then calls `verify_weighted_with_field` (349-366); `verify_weighted` constructs the same field and calls that implementation (382-394). No wrapper adds transcript absorption. Retain these wrappers for other protocol callers; this change needs no integer_bridge production edit. The bridge module already has an inspected prover equivalence test at integer_bridge.rs:1048-1087, although that test was not run in this review.

**Dependencies/verification:** update arithmetic unit tests' tuple destructuring and the independent opening assertion at arithmetic.rs:1635 to evaluate the weighted row/column form. Update Words test access at word_binding.rs:664-665. Preserve the Words dense oracle and complete proof/transcript comparison tests. Confirm Bits v5 and Words v6 proof values and transcript continuations before/after; both degrees include this shared implementation. No public API or serialized proof field must change. Keep field validation and all bridge shape checks.

## 2. SUGGESTION — low priority, independent: collapse the enum-to-bool constructor adapter

**Locations:** `backend.rs:31-38,55-63`; `protocol.rs:151-164`; bool consumers `arithmetic.rs:143,225`, `protocol.rs:209,392,658-659,693,698`.

`new_config` has exactly one caller: `new_with_binding` converts the public enum to bool and immediately invokes it. There is no independently useful configuration boundary here. The smallest net simplification is to place the current constructor body directly in `new_with_binding`, beginning with `let word_binding = binding == FalconAlgebraicBinding::Words;`, and delete `new_config`. It removes the single-use forwarding layer without changing behavior.

Storing the public enum directly in Schedule (`binding: FalconAlgebraicBinding`) would avoid a second representation and make dispatch descriptive. However, enum versus bool by itself removes no illegal state: each has precisely two variants. Encoding `terminal_merge_bits` inside a private schedule variant would enforce Bits-with-no-merge-work, but adds match/access machinery and is not needed to fix a demonstrated misuse. Current construction always sets Bits merge bits to zero. Prefer the constructor collapse; treat a Schedule enum migration as optional readability work, not a correctness requirement. If performed, do not accidentally add any new bit-mode digest bytes.

**Dependencies/verification:** independent of finding 1. Existing default-selection/security tests at protocol.rs:893-983 cover behavior to preserve. No new test needed for a forwarding-only removal.

## Investigated and intentionally not promoted to findings

- `word_binding.rs:304,328-337` uses `Option<[Vec<F>;2]>` as a moved-out slot to release the large coefficient allocation at the transition to dense rounds. Both unwraps are dominated by the monotonically increasing round branches: `as_mut` occurs only before the one `take`. Replacing this with a transition enum or splitting the sumcheck loop would add machinery. `mem::take` on an array of Vecs is possible, but does not improve the mathematical round-state invariant; not a correctness defect or priority change.
- `word_binding::Output` repeats some fields that are moved into arithmetic `Proof`. That wrapper is a legitimate local ownership boundary: it does not duplicate the norm proof or allocate a second proof. A common binder-proof type would modify the flat stored proof shape and custom Debug representation. The latter intentionally preserves Bits debug snapshots (`arithmetic.rs:76-89`). Preserve these proof fields and mappings in this cleanup.
- The Words test helper clones proof/nonces (`word_binding.rs:594-609`) because it retains Output for oracle assertions. These are test-only copies and do not justify an ownership API change.
- The weighted and equality bridge entry points already share proof/reduction implementation; their thin wrappers serve other callers and document distinct input contracts. Do not merge unrelated split/unsplit protocols or remove public choice `FalconAlgebraicBinding`.
