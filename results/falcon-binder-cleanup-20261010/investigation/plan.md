# Reconciled cleanup plan

Scope: optimization branch 51b472c55..ef2508f3a and adjacent packed adapters.
All eight read-only reports reconciled before source edits. No demonstrated
protocol soundness defect. Preserve measured policies, wire/transcript behavior,
sparse lane15, in-place coefficient reuse, wide accumulators, and v5 compatibility.

1. Unused: delete crate-private CompositeMultilinearExtension and exclusively
   reachable helpers/tests; LSP refs, whole-source rg and Clippy agree. Retarget
   noncanonical-input regression to live factored adapter. Preserve compact
   ECDSA adapter and its independent dense oracle. Gate shared composite module
   outside tests/ECDSA. Run feature build + packed/structured tests; commit.
2. Types: uniform owned weighted BridgeClaim, (Proof, BridgeClaim) return;
   derive Bits equality row weights only after identical source-opening transcript
   absorption. Both modes use weighted bridge; drop claim at bridge completion.
   Remove new_config forwarding layer. Run both-degree arithmetic/word and bridge
   checks; commit. Preserve proof Debug and phase-transfer Option (safe invariant).
3. Dedup: share exact coefficient dot-product and plain word product-pair formula,
   keeping bounds and accumulation order. Hoist duplicate common weighted-bridge
   guards above private split/unsplit dispatch, preserving all rejection checks.
   Update stale derived-Debug comments in same touched protocol scope.
   Run dense-oracle and bridge tests; commit.
4. Deprecated experiment surface: remove only unmeasured group-cap override and
   corresponding runner/docs knob. Production caps remain. Keep three measured
   switches and word selector. Run runner tests + bench-feature check; commit.
5. Fresh final validation: native production bench/test builds; full targeted
   regression set including ECDSA compact oracle; sequential paired before/after
   Falcon512 arithmetic/full 1/4/8/16 worker trials. Require proof identity,
   compare total/prove-only time, verification and peak RSS. Retain frozen prior
   binaries/evidence and save distinct cleanup results. Investigate regressions.

No cycle, extra weak-type or fail-open finding justified broader edits. Unavailable
similarity-rs/jscpd/ast-grep/machete are tool gaps, not clean scan claims. Used manual
scope/diff/ref review, working stable rust-analyzer LSP, compiler and curated Clippy.
One source writer (root), logical commits, no changes to original dirty checkout.

## Measurement reconciliation

The optional hot arithmetic helpers and bridge guard rearrangement were rejected
after initial and inlined holdout comparisons measured small regressions. Restore
the exact original hot loops and bridge checks, keeping the corrected Debug
comments. Retained source 454b623e0 has 773 net lines removed, a uniform owned
bridge claim (no duplicated 128 KiB weights), the obsolete Composite deletion,
constructor collapse, and removal of the unmeasured group-cap override. Final
native release tests and new-seed campaign will requalify this narrower result.

Final qualification completed: 164 native release tests; separate 49-test ECDSA
check; final arithmetic/full matrix plus v5 compatibility and fixed 16-worker
verifier follow-up (98 processes/548 trials). All proofs verify and match within
protocol. Total/prove-only performance remains near parity but unresolved; the
combined full 16-worker verifier comparison is +0.329% [0.147%, 0.504%]. Retain this
explicit tradeoff. The objcopy provenance incident was repaired by recovering the
byte-identical baseline and repeating full comparisons; superseded data remains.
