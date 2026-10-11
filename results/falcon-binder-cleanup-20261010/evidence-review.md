# Cleanup evidence review

The retained cleanup (`454b623e0`) has completed validation with the intended binaries. Its original accepted eight-cell total-prover estimates range from −0.223% to +0.299%, with no positive interval excluding parity. A fixed 16-worker full follow-up gives a combined ten-pair total change of +0.490% [−0.377%, 1.805%] and verifier change of +0.329% [0.147%, 0.504%]. All accepted proof identities and targeted tests pass. There is no conclusive measured prover slowdown under these intervals, but strict verifier nonregression is unsupported and the follow-up gives wider prover uncertainty than the original matrix alone.

The initial cleanup (`ea5fe3158`) and its inlined follow-up (`3e7072cb2`) preserve observed proof contents but show small regressions and remain historical measurements. The optional arithmetic helper extraction and bridge-validation deduplication were subsequently rejected. The retained cleanup's first full-prover comparison is also excluded from acceptance because binary inspection accidentally rewrote both full executables. The original baseline was recovered byte-for-byte, the final candidate rebuilt intact, and a separate clean full rerun now supplies accepted full results.

The comparator is the frozen production build associated with source baseline `ef2508f3a`, not the older bit-binder baseline used by the original binder optimization project. `ea5fe3158..3e7072cb2` changes only two `#[inline(always)]` attributes. The initial source manifest correctly remains associated with `ea5fe3158`; it must not be represented as final-source provenance for later revisions.

## Method and independent checks

Reviewed `scripts/binder_experiments.py`, `scripts/bench_statistics.py`, their documented method, both validation drivers, the relevant source diffs, retained test logs, manifests, raw process outputs, and summaries. No builds, tests, binary inspection, or benchmark work was performed by this reviewer while timing was active.

Both timing campaigns use Falcon-512, batch 1024, security 128, arithmetic and full variants, and 1/4/8/16 workers on the pinned Ryzen 9 9950X3D host. Each cell contains five seed-matched baseline/candidate process pairs. Each process has one warmup and five measured trials. Initial seeds are 45–49; the inlining holdout uses fresh seeds 50–54 and a new randomized order seed. The runner shuffles matched blocks and configuration order and executes processes sequentially. Stage tracing is disabled.

Timing ratios use the geometric mean of paired process-median ratios. The independent sample count is five per cell, not 25 trial iterations. The runner's 95% paired bootstrap uses 10,000 deterministic resamples of log ratios. This review additionally enumerated all 5^5 possible resamples; the intervals below use that exact enumeration. They are descriptive, unadjusted intervals with five pairs on one host. An interval crossing zero percent change is inconclusive, not evidence of equality or nonregression.

The independent raw-output audit passed for:

| Campaign | Processes | Trials, including warmup | Identity groups |
|---|---:|---:|---:|
| `performance` | 80 | 480 | 10 variant/seed groups |
| `inline-holdout` | 80 | 480 | 10 variant/seed groups |
| `compat-512` | 4 | 4 | 1 |
| `compat-1024` | 4 | 4 | 1 |
| `inline-compat-512` | 4 | 4 | 1 |
| `inline-compat-1024` | 4 | 4 | 1 |

For every listed process, the audit checked completed-job identity, recorded raw stdout SHA-256, successful exit without timeout, expected trial counts, every trial's verification flag, degree/batch/security/seed, requested main/worker/auxiliary affinity, full-verifier affinity where applicable, and absence of recorded stage events. All checks passed. Independently recomputed process medians, paired geometric timing ratios, and maximum process RSS agreed with the corresponding summary ratios.

For each variant/seed group, **both configurations and every requested worker count** have one identical tuple of actual protocol ID, input digest, source root, complete proof debug digest, and proof payload bytes across warmup and measured trials. This is stronger than runner success alone: `word: auto` can form a separate equivalence group if its actual protocol changes, so the audit explicitly checked equality between baseline and cleanup protocols. The arithmetic target shape reports `bitz/falcon512-algebraic/non-zk/word-binder/v6`; full reports `bitz/falcon/shared-prime/non-zk/v6`.

Both revisions also preserve identical v5 proof identities between baseline and cleanup at 1/4 workers for Falcon-512 batch 3/security 128 and Falcon-1024 batch 1/security 100. These compatibility campaigns use one seed each and are functional checks, not performance evidence. Debug-content digests are the benchmark's deterministic complete-content hashes, not a portable proof serialization or a proof of security.

## Initial cleanup and inlining follow-up

Positive percentages mean higher cost. The initial campaign has total-prover regressions whose intervals exclude parity: arithmetic 1T +0.604% [0.366%, 0.853%], full 4T +0.141% [0.063%, 0.243%], and full 8T +0.332% [0.059%, 0.606%]. The other total intervals cross parity. Initial arithmetic 1T prove-only cost is +0.668%; its verifier cost is +0.560%. Initial full 1T total cost is nearly unchanged (−0.006%), despite +0.099% prove-only cost; witness/commit behavior accounts for the difference. Thus total and prove-only conclusions must remain separate.

The inlining follow-up still has positive total-prover point estimates in all eight cells:

| Variant | Workers | Total change, exact 95% interval | Prove-only change | Verifier change | Lifetime RSS change |
|---|---:|---:|---:|---:|---:|
| Arithmetic | 1 | +0.140% [0.067%, 0.204%] | +0.197% | +0.542% | −0.141% |
| Arithmetic | 4 | +0.154% [−0.101%, 0.467%] | +0.150% | +0.847% | −0.026% |
| Arithmetic | 8 | +0.905% [−0.048%, 1.863%] | +0.693% | +1.027% | +0.100% |
| Arithmetic | 16 | +0.267% [−0.213%, 0.913%] | +0.275% | +2.393% | −0.169% |
| Full | 1 | +0.182% [0.071%, 0.254%] | +0.233% | −0.497% | +0.022% |
| Full | 4 | +0.194% [0.016%, 0.362%] | +0.115% | +0.609% | +0.086% |
| Full | 8 | +0.103% [−0.217%, 0.437%] | +0.067% | −0.285% | +0.107% |
| Full | 16 | +0.262% [−0.039%, 0.566%] | +0.185% | +0.736% | −0.094% |

The small total increases at arithmetic 1T and full 1T/4T remain distinguishable from parity under this descriptive bootstrap. Arithmetic verification at 16T increases +2.393% [1.874%, 2.810%]; at 1T it increases +0.542% [0.199%, 0.887%]. Full verification increases at 4T and 16T have positive intervals as well. These findings should not be dismissed solely because total latency shifts are small. The data do not isolate a causal contribution from each cleanup component or establish that inlining alone caused the changes between campaigns, which use different seeds and run periods.

RSS is the maximum observed process high-water mark over warmup and samples, including setup, allocator retention, and proof-digest formatting. Every inlining-holdout RSS interval crosses parity. The source-level removal of an 8192-element `u128` clone saves a 128 KiB allocation at the target shape, but these lifetime RSS measurements do not establish a measured peak-memory reduction from that change.

## Functional test scope and disposition

The initial cleanup has 164 passing targeted native release tests over Falcon-512/1024, integer bridges, packed sumcheck, openings, transcript framing, polynomial helpers, and structured MLEs. They precede the final two inline attributes. A separate 49-test ECDSA/packed run passed against the same structural revision before the attributes. After adding the attributes, the fresh arithmetic-filter run passed 25 tests. The 25 are a later targeted run, not an independent 25-test expansion of the earlier suite; do not add these counts into a claim about one final-source suite. Both measured revisions additionally pass the benchmark verification and proof-identity checks above.

Rejecting the optional hot-path deduplication is consistent with a strict performance-nonregression objective. Retaining dead-code removal, stronger bridge-claim ownership, and removal of the unused benchmark control produced the narrower candidate assessed below. No broad correctness, security, verifier, RSS, or latency nonregression claim is supported beyond the explicitly checked fixtures and revisions.

## First retained campaign: arithmetic valid, original full rows superseded

`retained-performance` completed 80 processes and 480 trials using new seeds 55–59 and order seed 20261012. Its 40 arithmetic processes/240 trials are valid acceptance evidence; the 40 full processes/240 trials are preserved as superseded measurements affected by the binary-inspection incident below. `retained-compat-512` and `retained-compat-1024` each completed four processes/four trials at 1/4 workers and remain valid. All observed proofs preserve the v6 and v5 identities described above, respectively. The final diff is 78 insertions and 851 deletions across ten files; the two arithmetic helpers and bridge-validation extraction are absent.

A second independent audit reconciled **every stored normalized trial directly against raw stdout** in all nine campaigns. It also checked all timing/RSS/payload medians and paired ratios against the summaries, actual absence of diagnostic stage records, successful process metadata, command-to-manifest binary identity, all requested affinity readbacks, and complete per-variant/seed identity equality across configurations and worker counts. These checks passed for 264 processes and 1,464 trials in total, including the rejected revisions. The final source file hashes match `retained-source-manifest.json`; the current final campaign binary hashes, runner hash, and imported helper hashes match its manifest. `evidence-audit.json` retains the identity groups, counts, hash checks, paired metric ratios, and exact-enumeration intervals.

The percentages below are paired changes in cost; positive values are increases. Brackets contain exact 95% bootstrap intervals. **Full rows in this table are superseded and excluded from final performance acceptance.**

| Variant | Workers | Total change | Prove-only change | Verifier change | Lifetime RSS change |
|---|---:|---:|---:|---:|---:|
| Arithmetic | 1 | +0.187% [−0.118%, 0.557%] | +0.242% | +0.319% | +0.859% |
| Arithmetic | 4 | +0.229% [−0.273%, 0.829%] | +0.092% | +0.879% | −0.109% |
| Arithmetic | 8 | −0.046% [−0.285%, 0.147%] | −0.042% | +0.641% | −0.137% |
| Arithmetic | 16 | +0.299% [−0.127%, 0.838%] | −0.009% | +0.261% | −0.231% |
| Full | 1 | +0.052% [0.003%, 0.129%] | −0.042% | −0.362% | +0.031% |
| Full | 4 | +0.124% [−0.132%, 0.381%] | +0.253% | −0.002% | +0.240% |
| Full | 8 | −0.184% [−0.583%, 0.267%] | −0.193% | −0.053% | +0.136% |
| Full | 16 | −0.301% [−0.580%, −0.111%] | −0.185% | −0.286% | +0.115% |

All valid arithmetic total intervals cross parity, with point estimates from −0.046% to +0.299%; these measurements do not establish strict zero-regression. In the superseded full data, 16T improves under this interval while 1T has a tiny positive shift whose interval excludes parity. That full 1T shift accompanies witness/commit +0.412% [0.282%, 0.532%] and prove-only −0.042% [−0.111%, 0.041%]. These full results describe the incident-affected executables only and require replacement before any final full performance conclusion.

In that campaign, all eight prove-only intervals and all eight verifier intervals cross parity. Arithmetic verifier point estimates range from +0.261% to +0.879%; for example, arithmetic 4T has +0.879% [−0.351%, 2.125%]. These are inconclusive measurements, not verified zero overhead. The superseded full verifier point estimates range from −0.362% to −0.002%, also with intervals crossing parity.

In that campaign, the RSS intervals exclude parity only for arithmetic 8T, −0.137% [−0.238%, −0.019%], and superseded full 1T, +0.031% [0.002%, 0.057%]. Other RSS intervals cross parity; arithmetic 1T is notably wide, +0.859% [−0.526%, 3.191%]. The same lifetime-peak limitations apply, and the source allocation saving does not justify a broad claim of lower peak memory.

Final-source functional logs establish 164 passing targeted native release tests in `retained-unit-test-summary.json`. Separate final development runs pass 49 ECDSA/packed tests, 25 arithmetic-filter tests, 16 bridge tests, and one structured-MLE test; the Python runner has six passing tests. The filters overlap with the release suite, so these are separate checks rather than a summed count of distinct tests.

**Binary-inspection incident, cause established:** this audit found the full baseline recorded at the original production path with SHA-256 `ff7174b92cbe9fa92b9c719063b7d9b00e18cdd948234e88861fec7b292af751`, although the earlier production manifest and both rejected-cleanup campaign manifests record `79dd92b01e5bc2f1d74305306c36fe9013862bffd5a41870d7ff881f50c7dbb2`. The parent agent confirmed it invoked `objcopy --dump-section .text=<dump> <binary>` without an explicit output executable, which rewrites the input. This changed both the old full baseline and the new retained full executable before the retained campaign. Arithmetic executables were untouched, and the initial/inlining campaigns preceded the incident. Matching raw process records and current hashes therefore validate what was run but do not qualify it as the intended untouched compiler output.

Recovery and rerun are now complete. A read-only search of the workspace and `/tmp` checked 71 distinct executable ELF copies matching the relevant Falcon/full names and found no byte-identical `79dd92…` backup. The parent rebuilt the original source with the original worktree path/toolchain/flags and obtained the exact original `79dd92…` executable, restored the production path from it, restored final source, and built the untouched final full candidate with SHA-256 `ae69fcf83d379d220e998aec054384e7ad16be33029d7667c353d1ead6f6c835`. This reviewer independently rehashed both recovered executables and the restored production path; verified both successful release builds without `bench-internals`; and checked current final source and recovery backups against the `454b623e0` source manifest. All checks pass. `provenance-recovery/manifest.json` retains the commands and hashes. The incident-affected full results remain recorded above for transparency and are not pooled with the clean rerun.

## Accepted final comparison

Accepted arithmetic measurements come from `retained-performance`, seeds 55–59; accepted full measurements come from `recovered-full-performance`, fresh seeds 60–64 and order seed 20261013. The clean full campaign contributes 40 processes/240 trials and five full-protocol identity groups, with both configurations and all four worker counts agreeing within each group. Adding the valid retained arithmetic and v5 compatibility checks gives **88 final processes and 488 verified trials**. The complete independent audit covers 304 processes/1,704 trials across ten campaigns, including the 40 superseded full processes/240 trials. Those historical counts must not be presented as final-source acceptance counts.

The clean full raw-output audit repeats all earlier checks: raw hashes, direct raw-to-normalized equality, successful process metadata, exact expected trial counts, every verification flag, public shape/security/seed, command binary identities, affinity readbacks, absent stage diagnostics, complete proof identities, and independent reconciliation of every timing/RSS/payload median and paired ratio. All pass. Clean binary, runner/helper, source, and source-backup hashes also match. `evidence-audit.json` now holds the corrected `final_metrics` below and separately preserves `superseded_retained_full_metrics`; its provenance recovery records identify the accepted executables.

| Variant | Workers | Total change, exact 95% interval | Prove-only change | Verifier change | Lifetime RSS change |
|---|---:|---:|---:|---:|---:|
| Arithmetic | 1 | +0.187% [−0.118%, 0.557%] | +0.242% | +0.319% | +0.859% |
| Arithmetic | 4 | +0.229% [−0.273%, 0.829%] | +0.092% | +0.879% | −0.109% |
| Arithmetic | 8 | −0.046% [−0.285%, 0.147%] | −0.042% | +0.641% | −0.137% |
| Arithmetic | 16 | +0.299% [−0.127%, 0.838%] | −0.009% | +0.261% | −0.231% |
| Full | 1 | −0.128% [−0.247%, 0.043%] | −0.112% | +0.010% | −0.010% |
| Full | 4 | −0.223% [−0.394%, −0.070%] | −0.270% | −0.019% | +0.472% |
| Full | 8 | +0.026% [−0.078%, 0.134%] | +0.093% | −0.352% | +0.180% |
| Full | 16 | −0.216% [−0.678%, 0.395%] | −0.145% | +0.456% | −0.163% |

Seven accepted total intervals cross parity; full 4T shows a small improvement under the descriptive interval. Its prove-only change is −0.270% [−0.524%, −0.066%]; all other accepted prove-only intervals cross parity. The observed maximum +0.299% total point estimate is not an upper bound on possible regression: arithmetic 16T's interval extends to +0.838%, and this one-host study does not establish a general performance bound.

Verification must remain a separate qualification. Full 8T improves −0.352% [−0.554%, −0.139%], while full 16T increases +0.456% [0.268%, 0.675%]. All other accepted verifier intervals cross parity. The small 16T verifier regression is real evidence under this descriptive method and must be disclosed rather than replaced with a broad claim of no regression.

All clean full RSS intervals cross parity. In particular, full 4T's +0.472% paired estimate has interval [−0.561%, 1.516%], while full 8T is +0.180% [−0.019%, 0.366%]. Arithmetic 8T retains its small negative interval; all other arithmetic RSS intervals cross parity. There is no supported universal memory reduction or nonregression conclusion. The removed 128 KiB clone remains a source-level allocation saving, separate from lifetime RSS.

The eight-cell qualification is complete for the recorded revisions, binaries, fixtures, public shapes and worker counts. The retained cleanup has strong scoped functional evidence and near-parity total-latency point estimates in this matrix, with the explicit small verifier tradeoff above and the statistical/host limitations already stated. The separate fixed follow-up below adds uncertainty and must accompany a final overall performance assessment.

## Fixed 16-worker verifier follow-up

The positive clean full 16-worker verifier interval triggered one follow-up with a **fixed five new seeds, 65–69**, order seed 20261014, unchanged recovered binaries, 16 workers, one warmup and five samples. `verifier-followup-plan.md` fixes these choices before the run and explicitly forbids significance-based sample extension or discarding the original positive result. The ten processes/60 trials completed successfully. Independent raw-to-normalized, process, affinity, binary/hash, complete proof identity and summary checks all passed. Each of the five new fixtures has identical protocol/input/source/proof digest/payload across both configurations at 16 workers.

The original eight-cell `final_metrics` table is unchanged. `evidence-audit.json` stores `verifier_followup` and `verifier_combined` separately. The holdout's five-pair intervals enumerate all 5^5 resamples. The combined ten-pair intervals use 100,000 deterministic bootstrap resamples of log ratios, Python `random.Random(0)`, geometric means, and type-7 2.5/97.5 percentiles. The combined sample includes all original seeds 60–64 and all new seeds 65–69. The cell was selected for follow-up after observing its original positive verifier result; pooled intervals remain descriptive and are not a simultaneous guarantee.

| Metric | Original five pairs | Follow-up five pairs | Combined ten pairs |
|---|---:|---:|---:|
| Total prover | −0.216% [−0.678%, 0.395%] | +1.200% [−0.328%, 3.644%] | +0.490% [−0.377%, 1.805%] |
| Prove-only | −0.145% [−0.457%, 0.109%] | +0.960% [−0.143%, 2.675%] | +0.406% [−0.216%, 1.369%] |
| Witness/commit | −0.308% [−1.289%, 0.697%] | +1.792% [−0.661%, 5.584%] | +0.736% [−0.689%, 2.826%] |
| Verifier | +0.456% [0.268%, 0.675%] | +0.202% [−0.052%, 0.446%] | +0.329% [0.147%, 0.504%] |
| Lifetime RSS | −0.163% [−0.401%, 0.106%] | +0.286% [0.087%, 0.495%] | +0.061% [−0.154%, 0.273%] |

The holdout verifier interval alone crosses parity, while the original and combined intervals support a small positive shift. It would be incorrect to dismiss the verifier increase using only the newer inconclusive interval. Prover intervals remain inconclusive; that is absence of a conclusive measured slowdown, not proof of a zero or negligible bound. The follow-up total point estimate is +1.200%, and the pooled interval extends to +1.805%, so a universal “within 0.3%” claim is unsupported. Seed 65's paired total increase is +5.923%; it remains included without an outlier exclusion. No causal diagnosis is inferred from this observation.

The follow-up RSS increase is distinguishable from parity within that five-pair run, while the pooled RSS interval crosses parity. Both are reported, and neither supports universal peak-memory nonregression. Lifetime RSS still does not isolate the removed clone.

Final-source validation including this follow-up comprises **98 processes and 548 verified trials**; the original accepted matrix plus v5 checks remains 88/488. The complete audited history is **314 processes and 1,764 trials**, including rejected revisions and the superseded full campaign. No source edit or benchmark run was performed by this reviewer. The final performance conclusion must preserve the small verifier regression and the wider follow-up prover uncertainty while keeping the earlier eight-cell matrix intact.
