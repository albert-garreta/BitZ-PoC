# Applying Speeding Up Sum-Check Proving (Extended Version)

Source: [Dao, DeStefano, Bagad, Domb, Thaler, ePrint 2026/587](https://eprint.iacr.org/2026/587.pdf), March 24, 2026 version.

## Techniques relevant to this implementation

1. **Delay reduction across mixed products (§3).** Keep exact integer operations
   outside the field and reduce weighted accumulations in batches. Our capability
   implementations and equality buckets already do this. The current additional
   optimization uses raw two’s-complement words plus one masked radix correction
   per residual, eliminating per-word positive/negative weight selection. The
   invariant is the same weighted residue at Montgomery scale R. BigInt tests
   cover signs, unsigned top bits within a narrower public magnitude bound,
   maximum values, carries, and the one-limb reduction fallback.

2. **Factor equality weights (§6).** Move reusable equality factors outside the
   inner product traversal. Our ordinary and prefix paths share two-level
   buckets, exact local accumulators, and a bounded outer merge. Any alternative
   partition must preserve zero/one coordinates and public-size scheduling.

3. **Reuse extrapolation work (Appendix D.1).** The paper describes shifted
   evaluation recurrences and multi-addition chains. Our existing K=3 traversal
   constructs finite differences once, then advances both ends to the six
   required exterior nodes. A candidate addition chain must beat this actual
   traversal, rather than an independent dot product at every node. Candidate
   skip-v4 transposes the existing recurrence: for each side, three running
   values replace three in-place passes through an eight-entry difference table.
   It retains the same 28 initial subtractions and 42 extrapolation operations.
   This is our implementation experiment inspired by the shared-work principle;
   it is not the paper’s coefficient-based addition chain. The independent
   BigInt oracle now calls this exact production helper.

## Compatibility constraints and interpretation

Section 7 proposes symmetric integer domains. For this task, the existing base
points, exterior-node order, infinity normalization, and transcript bytes are
fixed. Merely translating every base and target point by the same constant
leaves the Lagrange weights unchanged, so that translation alone cannot improve
our coefficient bounds. This is an implementation-specific inference, not a
claim made by the paper.

The paper’s §9.2 ablation combines multiple improvements against an unoptimized
baseline. Our required comparator is already optimized master. Its reported
speedup is therefore not evidence that our 24 cells pass. In particular, the
full-width u128 fixtures require wide residuals; small-input gains must be
measured anew on the Linux/Ryzen host.
