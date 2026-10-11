# Cleanup source review

Reviewed optimization range 51b472c55..ef2508f3a in eight read-only dimensions,
then reviewed retained cleanup ef2508f3a..454b623e0 independently as a single change.

Concrete issues fixed:
- Word proofs cloned 8192 u128 row weights (128 KiB) and retained both allocations
  through the bridge. One owned weighted claim now serves both binder modes.
- A crate-private dense CompositeMultilinearExtension adapter had no production
  constructors. Removed its type, adapter and exclusively reachable helpers/tests;
  preserved ECDSA's live CompactCompositeMle and independent dense/proof oracle.
- The bridge claim used one point field with two meanings and optional row weights
  guaranteed present in word mode. It now carries a column point and mandatory
  owned weights. This removes the duplicate return value, mode branches and unwrap.
- An arbitrary group-cap benchmark override was wired but never measured. Removed
  that interface, retaining production caps and the three measured kernel controls.
- Collapsed the single-caller constructor wrapper and corrected stale Debug comments.

The optional arithmetic helper extraction and bridge-validation deduplication were
measured, then reverted after small timing regressions. The original hot loops and
bridge guards are retained. Final source delta: 78 insertions, 851 deletions (773
net lines removed) across ten files.

No demonstrated protocol soundness or fail-open defect found in this scoped review.
This is not a formal security proof. Kept v5 selection, v6 selectors/padding
checks, terminal-merge grinding, transcript framing, wide accumulation, sparse
lane-15 support and in-place allocation reuse. The final independent reviewer
confirmed the bit path absorbs its full opening point before deriving rows and
removing row coordinates; the word path changes only claim ownership and names.
The ECDSA compact implementation is byte-identical across the cleanup range, and
its feature gates agree with its import and caller gates.

Deletion evidence: whole-workspace rg, stable rust-analyzer references, compiler
checks and curated Clippy. The default rust-analyzer proxy was unavailable; the
installed stable executable worked with a recorded metadata fallback warning.
Optional similarity-rs, jscpd, ast-grep and cargo-machete were unavailable. No clean
results are claimed for missing tools, and Clippy has existing repository warnings.

Retargeted the malformed-input fixture to the live factored MLE, testing rejection
of noncanonical outer, inner and leading values before transcript mutation. Packed
ECDSA oracle coverage remains. Native release and feature-specific development
test results are saved separately; overlapping filters are not added together.

Performance evidence uses separate campaigns. An objcopy inspection accidentally
rewrote the full baseline and retained full executable before retained-performance;
that campaign's full rows are superseded. The original baseline was recovered
by an exact-source/toolchain rebuild and matched its original SHA-256 byte-for-byte.
A fresh final full executable was built and the separate clean full rerun passed.
A fixed five-seed verifier follow-up also completed; its original and held-out
measurements are both retained. Detailed tradeoffs appear in the evidence review. All historical raw measurements remain available and labeled.
