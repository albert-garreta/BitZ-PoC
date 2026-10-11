# Dimension 1: deduplication

Reviewed snapshot: `ef2508f3adaf153b891825d1c2f6dc4a69853898`, endpoint range `51b472c55..HEAD`, in the Falcon binder experiment worktree. Scope is the changed arithmetic/word/protocol/bridge code and adjacent packed binder/composite/transcript code. This is a read-only source review; the only written file is this report. No source edits, builds, tests, or installations were performed.

Instructions read: supplied `rust-reviewer/SKILL.md`, `readability-reviewer.md`, and `protocol-reviewer.md`. Classification: protocol-sensitive simplification of measured hot paths. Findings below are **SUGGESTION**, not demonstrated correctness defects. Priorities indicate implementation order, not vulnerability severity. All line numbers refer to the reviewed snapshot.

## Suggested changes

### D1. Remove the duplicate owned word row weights (priority 1)

- Location: `src/piop/spartan/falcon1024_algebraic/arithmetic.rs:144-156`, particularly `output.claim.row_weights.clone().unwrap()` at 145.
- Evidence: `word_binding::opening` puts the sole generated row vector in `BridgeClaim.row_weights` (`word_binding.rs:494-513`). Arithmetic then clones it into the tuple's second member. The only production caller is `protocol.rs:444-482`: it passes the separate vector to the bridge and never reads `claim.row_weights`. With `Layout::row_vars() == 13` (`source.rs:97-98`), the duplicate holds 8192 `u128`s, or 128 KiB, and survives the explicit `drop(row_weights)` at protocol line 482 because the clone still lives in `claim`.
- Smallest correction: make `output` mutable and move weights with `output.claim.row_weights.take().expect(...)`; the prover's returned claim needs only point/modulus/value after that move. A cleaner larger alternative is a single explicit prover output owning weights, but do not introduce it solely to eliminate this one clone.
- Compatibility: the verifier still needs its reconstructed `Some(weights)` at protocol lines 580-583. Direct word-binder tests compare complete claims at `word_binding.rs:652-662`; keep `word_binding::prove` itself unchanged. Existing arithmetic claim-equality tests currently exercise bit schedules, so document that the arithmetic prover tuple owns weights separately if taking from the claim. No transcript or proof payload changes.
- Dependencies: none. Coordinate with the ownership/dead-code pass to avoid competing changes to the output type.
- Benefit is demonstrated allocation removal; runtime impact is unmeasured.

### D2. Share the exact batched coefficient dot product (priority 2)

- Locations: `arithmetic.rs:628-640` and `word_binding.rs:436-448`.
- Evidence: both iterate the same two coefficient arrays at `instance * N..(instance + 1) * N`, zip the same `2*N` coefficient/side equality weights, accumulate in `FpProductAcc<2>`, and reduce with `prepare_product_reduction(2*N)`. Bit evaluation obtains these weights from `point[4..local_vars]` at arithmetic line 601; word evaluation obtains them from `point[..COEFFICIENT_LOG+1]` at word line 434. These are the same coordinate domain with the four bit lanes removed. Instance, slack, padding, and bit decoder handling differ and should remain local.
- Correction: a small private `BindingForm::coefficient_product_sum(instance, local_weights)` helper returning `FpProductAcc<2>`, used only by these two exact loops; callers retain the existing prepared reducer and parallel dispatch. An inline helper can remove roughly a dozen lines while putting the array layout in one place.
- Preserve the legacy scalar branch (`arithmetic.rs:642-657`), public-workload dispatch, `2*N` reduction bound, and the existing point-length guards. Avoid allocating or recomputing the reducer per instance.
- Dependencies: none, but combine with D3 only if the small helpers still improve readability. This is source deduplication, not a speed claim; generated-code/benchmark behavior has not been checked.

### D3. Share the repeated plain product-pair accumulation in word folds (priority 3)

- Locations: `word_binding.rs:85-90`, `157-162`, and `184-189`.
- Evidence: all three add `a[0]*a[1]` to the constant accumulator and `(b[0]-a[0])*(b[1]-a[1])` to the quadratic accumulator. They differ only in how the already-read rows are obtained. The first consumes dense rows, the second folds coefficient chunks, and the third crosses the side coordinate. This is genuinely shared arithmetic, not inverse prover/verifier code.
- Correction: one inline private helper taking the field, two mutable product accumulators, and two `Row`s. Keep input reads before the in-place writes in `fold_coefficients`, and leave reductions in their callers with their existing public term bounds.
- Do not mechanically merge `product_coefficients` (`word_binding.rs:43-67`) or omitted slack-coordinate coefficients (`234-245`): their constant/padding semantics differ. A generic callback/reducer framework would add more complexity than it removes.
- Dependencies: none. Net reduction is modest, roughly 10 lines, but the sumcheck polynomial formula becomes a single implementation. Reuse the existing dense oracle test; no new implementation-mirroring test is needed.

### D4. Move mode-independent bridge validation to the common entry (priority 3)

- Locations: split prover validation `integer_bridge.rs:284-290` versus private unsplit prover `571-577`; common verifier validation `423-425,429-431` versus private unsplit verifier `628-630,640-642`.
- Evidence: `prove_unsplit` has exactly one caller at line 274, and `verify_unsplit` exactly one at line 411. The column dimension, valid modulus construction, and canonical row-weight shape/bounds are identical for both bridge modes. Keeping them after the branch forces duplicate guards.
- Correction: put column-point length/modulus/row-weight guards before the mode branch in `prove_weighted`, and column-point length/row-weight guards before the branch in `verify_weighted_with_field`; remove only their now-dominated equivalents from private unsplit functions. Preserve `Shape::new`, mode-specific modulus bounds, checked weight sums, proof variant/forest/message-count guards, and every transcript call.
- Dependencies: none. This reduces around a dozen lines without a new helper. It changes which error wins when several independent input defects coexist, but keeps rejection before transcript mutation; current tests assert rejection, except for a late forest-specific error that these valid-input guards do not affect.

## Deliberately retained

- Both protocol relations and the explicit Bits API. `protocol.rs:139-145` selects Words only for degree 512, batch 1024, target 128. Other workloads and existing v5 proofs still need the bit path.
- Legacy construction/evaluation and reference replay outside qualified workload policies; benchmark results alone do not make them dead.
- `Proof`'s custom Debug implementation (`arithmetic.rs:76-90`): it conditionally omits an absent terminal nonce, preserving historical proof formatting/digests. Deriving Debug would change provenance even though transcript bytes are unchanged.
- Small-slice transcript framing duplication (`transcript/mod.rs:97-114`) and its independent reference in the framing test (`179-184`): this is the measured byte-preserving optimization and its oracle, not redundant implementation to delete.
- Equality-wrapper versus weighted-bridge entry points and separate split/unsplit forest code: these preserve callers, transcript domains, root dimensions, and proof variants. No broad bridge unification is proposed.
- `packed/composite.rs` is unchanged in this range. Its repeated powers-of-two setup belongs to separate tail-piece and fold calculations; changing it would broaden the binder cleanup with little benefit.
- Benchmark overrides are live callers of the archived experiment runner. Their retirement belongs with runner/documentation changes, not an isolated dedup change.

## Validation and tool limits

Used `git diff`, numbered source reads, `rg` caller searches, the frozen report, and existing tests as evidence. `similarity-rs`, `jscpd`, and `ast-grep` were unavailable; `/usr/bin/sg` is util-linux, not ast-grep. `rust-analyzer` exists but was not started. No automated similarity or LSP result is claimed. Repository uses edition 2024 and Rust 1.98.1.

Suggested existing checks after accepted edits: word sumcheck dense-oracle/bit-opening equivalence, word source/padding mutations, weighted bridge in both modes, equality-wrapper transcript equivalence, arithmetic bit/word complete-proof and transcript continuation checks, and transcript framing. Historical campaign results must remain tied to their frozen snapshots; a cleanup build needs a distinct validation record and must not overwrite old source/binary hashes.
