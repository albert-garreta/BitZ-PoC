# Compact Falcon opening messages

This change reduces the stored proof payload relative to the one-source
implementation at `9a401286c98db041762b97f48cfe4f1a8d85ce7c`.

The evidence applies to the frozen V7 source snapshot at
`/tmp/falcon-compact-opening-candidate-v7` and its recorded executable, not
to the current working tree. Concurrent Falcon parameterization changes
are preserved and are outside this validation. See the
[technical report](TECHNICAL_REPORT.md), [exact task-owned file list](task-owned-files-v7.json),
and [working-tree divergence audit](performance/v7/working-tree-concurrent-audit.json).
V7 timing acceptance remains pending.

The initial source opening transmits only occupied lanes: eleven field
elements per queried row for the usual geometry, or thirteen at batch one.
The verifier restores the remaining lanes to zero before hashing the
complete canonical sixteen-element row and using it in the PCS reduction.
The source commitment and its multiproof hashes remain unchanged.

The final recursive opening transmits the complete small message before
its last lane folds. The verifier reencodes that message, checks its
existing recursive root, and derives the folded residual. The former final
residual, row table, and multiproof are no longer stored separately. All
existing transcript observations, queries, and grinding remain in place.
After authenticating the complete message, the final queried-row equation
holds identically by linearity of encoding and lane folding. Its identical
contributions cancel from both sides of the residual equality; the verifier
checks the equivalent remaining equality, including all earlier claims.

These encodings have a deterministic reconstruction into the preceding
proof representation. Structural zeros are fixed by the prepared geometry;
the terminal message is bound by its existing Merkle root. Reconstruction
therefore introduces no new algebraic soundness term. Falcon uses the new
representation directly, with no old-proof compatibility branch. The
unrelated generic PCS clients retain their existing representation.

The [protocol description](../../src/piop/spartan/falcon1024_ct/ONE_SOURCE.md)
gives the mappings and equations. The
[correctness report](validation/REPORT.md) records the passing profile,
malformation, transcript-equivalence, and regression checks.

Runtime qualification is in progress. See the
[performance campaign](performance/README.md) for build provenance,
fixed-seed methodology, and the unchanged two-percent runtime limit.
Cold and instrumented diagnostics show smaller proofs in every case;
they are not substitutes for the complete sixty-seed timing qualification.
