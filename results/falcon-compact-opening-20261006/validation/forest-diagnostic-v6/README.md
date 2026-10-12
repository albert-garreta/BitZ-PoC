# Native/128/batch-1 forest diagnostic

Five consecutive proofs from the frozen V6 candidate used seed 42, no warmup, and the existing 16-worker affinity policy. All five verified and exactly matched the frozen V6 cold run's input digest, source root, proof debug digest, payload, and grinding diagnostics. This is a cost diagnostic, not a qualification comparison or evidence that the performance gate passes.

`method.json` records the exact executable, hash, command, environment, and exit status. `candidate.jsonl` retains proof and placement records; `candidate.stderr` retains both existing stage timings and `BITZ_TRACE`; `forest-only.log` extracts the latter. `equivalence.json` records comparisons against the frozen cold result. `summary.json` contains every parsed forest sample and its median.

| Operation | Median of five, ms |
| --- | ---: |
| Initial L3 value tables | 0.1708 |
| Folded L2 tables | 0.1448 |
| Folded L1 tables | 0.1357 |
| Folded L0 tables | 0.1469 |
| L4 rebuild | 0.0486 |
| Per-trial sum of those five operations | 0.6468 |

The first subtotal was 0.8491 ms and the last was 0.6115 ms. Arithmetic binding-template preparation measured 3.273341, 2.668465, 2.533910, 2.496249, and 2.544280 ms (median 2.544280 ms).

Stage logging adds substantial overhead to whole-proof and whole-forest times, including formatting/output during grinding. The table/rebuild spans above do not contain grinding calls. The existing trace's page-fault display assumes 16 KiB pages, so its displayed “MB faulted” values must not be interpreted as byte counts on this Linux host.

## Existing dense alternative

Native batch 1 uses `t=14`, `s=4`, and product depth 13. Its 16 columns have 262,144 leaves (4 MiB). All dense witness layers together occupy less than 8 MiB, comparable to one existing 8 MiB lookup table plus its arena. The existing dense engine is `GrandProductCircuit::new(leaves).batched_eval(32)` followed by `gpgkr_prove` at the same five-coordinate root point. The count 32 retains the unmultiplied limb coordinate alongside the four column coordinates. `Forest::prove_depth` selects product depth, not a materialization cutoff; there is no existing zero-versus-three cutoff switch.

`partial_depth_modes_match_dense` already compares the current lookup forest against this dense engine, including partial depth and Boolean/non-Boolean root points. A useful isolated comparison would include leaf construction, all witness-layer construction, and proving; compare every emitted pair, challenge count, terminal claim, and transcript state with the same transcript adapter. Grinding policy and calls must remain unchanged. A public small-column selection rule would affect execution only. Extra dense low-round arithmetic may offset the table savings, so no production change or performance benefit is established by this diagnostic.
