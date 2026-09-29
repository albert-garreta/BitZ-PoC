# Wfbitz cleanup: commit disposition ledger

Audited range: `6115da167f901bf8fd880042fecdf3703c3281df..57a632e0c10e5ac8a1ad037a7e61c24fc0196fc4` (47 commits, including the merge).
The dispositions below record the retained and retired ideas in the cleanup. Historical measurements retain their
original backend labels. This ledger is not an independent cryptographic audit
or a performance acceptance report.

The retained implementation is `src/wfbitz`, including its optimized native
forest/GKR kernels. The deleted implementation is `src/merged_forest.rs` and
its exclusive virtual-batching, dual-basis, extension, tap, RLC and codec code.

| # | Commit | Change | Disposition |
|---|---|---|---|
| 1 | `95f97ba5` | Initial parity PCS and opener; feature/dependencies; field kernels | Promote `wfbitz`; make spongefish/crypto-bigint and field encoding support unconditional in the root crate; retain field arithmetic optimizations. Retire feature wrapper. |
| 2 | `d83c26e9` | Multiplication benchmark backend selection and wfbitz split | Promote wfbitz multiplication path and split; retain u64 and ordinary integer-word workloads. Retire backend string and dual runners. |
| 3 | `1341a7cb` | Round 0 batched into the wfbitz opening | Retain Round-0 presence, statement order, and OOD batching. Preserve it when unifying standalone and relation adapters. |
| 4 | `05e5aa87` | u64 campaign launcher | Retain workload/rate/thread parameters and result workflow; remove feature/backend selection arguments. |
| 5 | `1ac63a0d` | u64 comparison measurements | Preserve as historical measurement evidence; update active instructions without relabeling old forest measurements as new results. |
| 6 | `6ba3ce85` | Additional eight-thread comparison measurements | Preserve historical provenance; no requirement to retain obsolete executable backend. |
| 7 | `a5b5635c` | Table script thread-column selection | Retain; orthogonal to PCS backend. |
| 8 | `a4f15372` | u32/u128 measured split comparisons | Preserve historical evidence and retained ordinary u32/u128 workloads; remove obsolete backend instructions. |
| 9 | `ef70cb22` | Unified reference split minus one row variable | Retain as canonical default for direct wfbitz workloads. Do not override explicit SHA relation geometry with it. |
| 10 | `242e0b5c` | Multiplication split stops at 64 gates per row | Retain geometry rule and boundary coverage. |
| 11 | `c2f4f9d5` | Word-aligned PoW prefix | Retain optimization and deterministic nonce semantics when extracting shared PoW helpers. |
| 12 | `3c8ee086` | Paper campaign and ladder selection | Retain ladder/rate selection and campaign functionality; retire backend feature flag. |
| 13 | `11cd7250` | General virtual opening | Retain; this is the destination for CM and linear SHA terminal claims. Preserve map/shape/claim binding. |
| 14 | `9895892c` | Small shapes, explicit PCS configs, composable GKR exit | Retain all three; required by small SHA contexts, MultiSwap, and hybrid. Do not impose the embedded fast-ladder floor on explicit ladders. |
| 15 | `2721003c` | SHA+ECDSA wfbitz virtual opening | Promote wfbitz proof payload; retire `Sha256EcdsaOpener`, its field/accessors, opening enum, and forest branch. |
| 16 | `b9f07c53` | Hybrid multiplication wfbitz fold/GKR | Promote; replace `MulGkr` with narg bytes and remove empty clear sums. Preserve the joint hybrid sumcheck/ring switch/shared Ligerito. |
| 17 | `7a1f9139` | MultiSwap reduced virtual wfbitz opening | Promote; retain integer lift, second prime, reduction nonce, explicit ladder, and canonical proof checks. |
| 18 | `07957489` | Benchmark/sweep/report integration | Retain output and comparison capabilities; remove backend CLI/env selectors and duplicate proof wrappers. |
| 19 | `3c23b13f` | Identity virtual-map fast path | Retain only with later full-grid fixes from #43; do not restore the broader early eligibility. |
| 20 | `2a1bd3a2` | Structured chained SHA+ECDSA opening | Retain structured geometry, sum-of-products binary query, and dense virtual fallback. Distinct from a second PCS backend. |
| 21 | `af4f8a9b` | Selected chained ladder and cheaper source gather | Retain selected ladder, lazy source rows, and gather/copy optimization. |
| 22 | `f0fdb386` | Bit-matrix transpose and hybrid split | Retain packing/transposition optimizations; construct hybrid with wfbitz split directly. |
| 23 | `a3be3bd1` | Prescaled wide forest fold tables | Retain inside the wfbitz forest. The word forest alone does not identify legacy PCS code. |
| 24 | `62e65c4a` | Reusable nibble-row indices | Retain optimized wfbitz representation. |
| 25 | `5459e868` | Forest cost probes | Retire the dedicated cost-probe module under the user's explicit cleanup plan, after recording its four AArch64-only probes as architecture-ineligible on this AMD host. Retain the optimized Wfbitz kernels and full-proof benchmark counterparts; probe retirement is distinct from old PCS engine deletion. |
| 26 | `29ed3076` | Nibble-row threshold | Retain threshold/dispatch and associated correctness coverage. |
| 27 | `bee80f6e` | Single-pass bit rounds | Retain; new security boundary hooks must execute identically on this and ordinary prover paths. |
| 28 | `5cc3d343` | Correct UDR per-fold grinding; standalone Round 0 | Retain accounting/config correction across binary PCS, hybrid, CLI and wfbitz. Do not reuse pre-fix UDR margins. |
| 29 | `aebf4951` | Canonical claims, exact maps, chained eligibility | Retain all validation and optimized-kernel fixes; include later active-parameter validation. |
| 30 | `dde8022b` | Separate forest/wfbitz wire formats; reject host nonces | Retire backward-compatible format dispatch. Preserve canonical decoding/EOF invariants; removing host nonce field makes the old forbidden state unrepresentable. |
| 31 | `1e1a00c2` | Lambda128 transcript pins after UDR fix | Retain regression intent; re-record only after intended protocol changes are reviewed. Pins are not a reason to retain old backend bytes. |
| 32 | `ced5942b` | Commitment configuration checks, discharge dispatch, OOD presence | Retain protections. Replace ungrinded-only rejection with enforced native wfbitz grinding schedule for high profiles; never just remove security checks. |
| 33 | `aa19f47e` | Tail weights restricted to tail cells; one effective ECDSA ladder | Retain strict tail support and effective ladder for commitment, statement, Round 0 and accounting. |
| 34 | `d6818e33` | Bind resolved ladder before Round 0 | Retain exact ordering and policy binding under unified adapter. |
| 35 | `cabc49cc` | Fold gate, standalone path, campaign/default corrections | Retain standalone capability and actual fold bound; update historical revision/instructions honestly. |
| 36 | `8d8b1ea6` | Review follow-ups across OOD, chained opening, UDR accounting and CLI | Retain fixes: late OOD opening-claim handling, selected ladder/security consistency, checked decoding, correct native work accounting. Retire only selector/compatibility shell. |
| 37 | `167558b5` | Carry column weights in dense-round left half | Retain GKR optimization and equivalent challenge ordering. |
| 38 | `17584271` | Position tables without temporaries; reused buffer | Retain buffer reuse. |
| 39 | `9f74ce2b` | Build eight nibble rows per cache line | Retain optimized packing. |
| 40 | `b33cfaa7` | Compute column folds from nibble rows | Retain fold/kernel reuse and canonical mathematical result. |
| 41 | `e2ee26d4` | Scatter four pair buckets in one pass | Retain optimized forest path and transcript equivalence. |
| 42 | `f710c440` | Packed column combination without per-byte loop | Retained shared `ligerito` packing kernels used by Wfbitz/binary/hybrid, extracted from the deleted engine. |
| 43 | `212fc301` | Direct identity requires full-grid identity | Retain exact grid-size checks; smaller identity maps remain virtual and zero-padded. |
| 44 | `92d7fa99` | Validate canonical claims against active parameters | Retain validation at execution, not merely construction; changing parameters must not bypass canonical checks. |
| 45 | `4e080f08` | Merge fix series | Preserve integrated result; no standalone code to port. |
| 46 | `a45e8ba6` | Public mod-q statement binding and mandatory Johnson Round 0 | Ported invariants/tests to the canonical Wfbitz standalone API; retired old function signatures and duplicate helper flows. |
| 47 | `57a632e0` | Direct BitZ claim binding before folds | Retain parameter, root and claim binding before first fold challenge and regression coverage in `tests/wfbitz_claim_binding.rs`. |

The final API also removes matrix word-width state and the extension-projection
parameter shell. Prime sampling retains its previous interval and transcript
sequence. Forest-only phase tables, environment controls, and obsolete CSV /
snapshot collectors were removed; native full-proof benchmarks remain.
