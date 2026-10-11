# Falcon-512 thread scaling — retained result

**16 workers now beat 8 for arithmetic and full Falcon. No statistically detected proving slowdown appears at 1/4/8/16 workers in the selected measured-sample or first-proof evidence.** At 16 workers, total proving improves **10.49% for arithmetic** and **4.77% for full Falcon** versus the frozen pre-scheduling baseline. Some full-Falcon cells remain inconclusive; this is not proof of zero change. Full-Falcon 16-worker peak RSS increases **0.160%**, retained as a modest secondary tradeoff under the user's updated priority.

## Evidence provenance and scope

Falcon-512, 1,024 distinct signatures, 128-bit target; native release production `falcon-hybrid` binaries. Baseline revision is `78c164f0fdb5939924561e2831241d7eb558c17b`, not the earlier historical 51b472c55. Each process runs two warmups and seven measured proofs. Both campaigns use exactly three baseline-first and three candidate-first pairs per worker count.

- **Arithmetic:** reused early-drop qualification on seeds **100–105**, 48 selected processes/432 verified trials. The current arithmetic executable SHA-256 is exactly `8b9a8a43ca2a5b12fb509cc5661d1382c60c5aec9f26f1157891afba3ac1458b`, byte-identical to the earlier qualified executable. This historical candidate was selected after later experiments; it was not independently rerun for the final selection.
- **Full Falcon:** fresh final-source holdout on seeds **180–185**, 48 processes/432 verified trials, executable SHA-256 `1eaaf10210ee2d7498bf114b027eb35edc972599a5f3beebe35a13731b3f98d0`.

These are **two distinct campaigns, not one fresh combined matrix**. Complete source/build, binary and reproduction details are in [source-build.json](source-build.json). All selected proofs verified; complete proof-debug digests, payload sizes, input identities, source commitments and protocol IDs match their baseline counterparts across worker counts. Independent audits checked frozen hashes, full argv, raw metric normalization, affinity, balanced pair ordering and non-overlapping timed processes.

## Selected measured-sample results

Times are medians of six process medians. Percentage changes are geometric means of matched candidate/baseline ratios, so they need not equal ratios of displayed medians. Negative changes mean faster or less memory.

| Variant | Workers | Baseline total ms | Candidate total ms | Total change | Prove-only change | Verify change | Peak RSS change |
|---|---:|---:|---:|---:|---:|---:|---:|
| arithmetic | 1 | 182.043 | 181.344 | -0.286% | -0.500% | -3.451% | -3.056% |
| arithmetic | 4 | 65.240 | 64.627 | -0.967% | -0.688% | -5.904% | -24.417% |
| arithmetic | 8 | 45.825 | 45.459 | -1.298% | -1.016% | -7.224% | -24.131% |
| arithmetic | 16 | 49.551 | 44.389 | -10.495% | -11.444% | -5.653% | -4.252% |
| full | 1 | 1984.129 | 1985.738 | +0.054% | -0.049% | -0.159% | +0.066% |
| full | 4 | 627.877 | 624.485 | -0.392% | -0.535% | -1.670% | +0.309% |
| full | 8 | 403.766 | 401.748 | -0.347% | -0.372% | -2.143% | +0.026% |
| full | 16 | 340.305 | 326.030 | -4.774% | -4.974% | -2.485% | +0.160% |

Every arithmetic total/prove/verify/RSS cell passes the original strict confidence gate. Full Falcon total/prove improves at 4 and 16 workers; at 1 and 8 the intervals cross zero and are inconclusive. Full 16 RSS has a positive interval **+0.046% to +0.300%**, with displayed process-peak medians **2,483,094 → 2,485,424 KiB**. Other full RSS changes are inconclusive. The user now accepts modest verification/RSS increases; no numerical allowance was invented. Full verification improves at 4/8/16 workers, with 1 worker inconclusive.

| Variant | Candidate 8-worker total ms | Candidate 16-worker total ms | Paired 16/8 total change | Exact 95% interval | Paired 16/8 prove change | Exact 95% interval |
|---|---:|---:|---:|---|---:|---|
| arithmetic | 45.459 | 44.389 | -2.164% | [-3.047%, -1.203%] | -1.161% | [-2.101%, -0.100%] |
| full | 401.748 | 326.030 | -18.984% | [-19.938%, -17.668%] | -19.743% | [-20.507%, -18.732%] |

## First proof in each process

The first warmup proof was audited separately using already-collected output. Setup can initialize allocator/cache state, so these are first-proof/process measurements, not universally cold proofs. They are not pooled with the prespecified measured-sample medians.

| Variant | Workers | First total change | Exact 95% interval | First prove-only change | Exact 95% interval |
|---|---:|---:|---|---:|---|
| arithmetic | 1 | -0.482% | [-0.827%, -0.096%] | -0.574% | [-0.922%, -0.179%] |
| arithmetic | 4 | -0.736% | [-1.276%, -0.208%] | -0.798% | [-1.248%, -0.308%] |
| arithmetic | 8 | -1.251% | [-2.077%, -0.328%] | -1.355% | [-2.023%, -0.430%] |
| arithmetic | 16 | -10.307% | [-11.976%, -7.842%] | -11.314% | [-13.103%, -8.511%] |
| full | 1 | +0.325% | [-0.121%, +0.905%] | +0.306% | [-0.170%, +0.956%] |
| full | 4 | +0.251% | [-0.287%, +0.791%] | +0.394% | [-0.227%, +1.079%] |
| full | 8 | -0.233% | [-0.779%, +0.315%] | -0.284% | [-0.825%, +0.227%] |
| full | 16 | -3.024% | [-4.067%, -2.030%] | -4.018% | [-5.155%, -2.904%] |

Arithmetic first-proof total/prove improves at all requested worker counts. Full first-proof total/prove improves at 16; 1/4/8 remain inconclusive, including small positive point estimates at 1/4. No selected first-proof proving interval has a positive lower bound. This does not guarantee identical startup time on every run.

## Retained implementation and validation

Large forest rows keep stable worker ownership on eligible matrices above 8 workers. Distinct cyclic/fallback closures and three x86 inlining attributes preserve optimized row arithmetic. Fused column packing writes directly into final allocations, and sufficiently large public-power batches run in parallel with preserved limb ordering. Six last-use drops release temporary proof data earlier. Public initial-state weights factor a common GF multiplication and skip publicly zero lanes. The vendor scratch-cache policy is the original policy; all cache-policy and caller-allocation experiments were removed.

No transcript, proof-format, security-schedule or public API change is introduced. Source reviews found no material correctness/safety findings. Validation includes separately recorded 76/72-test parallel/nonparallel BitZ snapshots, 57 packing/bridge checks, 17 public-power bridge tests, focused end-to-end tests and the public-weight oracle. These tests belong to their recorded snapshots rather than one newly rerun suite. Native selected campaigns exercise Falcon-512 with verified matching proof contents; the focused full test instantiated the shared Falcon-1024 profile. Details and limits are in [review.md](review.md).

## Selection history and rejected experiments

All earlier values and classifications remain in [summaries.json](summaries.json). Earlier memory/verification failures used the then-stricter rule; they are not silently relabeled, but they are not automatic blockers under the user's later preference.

| Experiment | Evidence and disposition |
|---|---|
| Separate closures | Lower-worker arithmetic/full regressions remained; superseded. |
| Additional x86 inlining | Full 1/8 total increased 0.234%/0.429% in screening; superseded. |
| Fused packing alone | Arithmetic 8 total increased 0.245%; full 4 verify increased 0.469%; superseded by public-power batching. |
| Public-power candidate | Primary holdout arithmetic 1 total/prove/RSS and full 1 total/prove increased; rejected. |
| Early-drop | Primary proving and retrospective first-proof evidence had no detected regressions; initially rejected only for full 8/16 RSS. Its identical arithmetic executable is now reused. |
| Caller-owned packed allocation | Two-pair RSS increased 0.477%/0.613% at 8/16; reverted. |
| Original packing restoration | Two-pair full 16 RSS increased 0.375%; later fused packing restored for arithmetic performance. |
| Retained cyclic jobs | Two-pair full 16 RSS increased 0.239%; reverted. |
| Scratch count 23 | Fresh holdout full 1 verify increased 0.280%, full 16 RSS 0.189%; reverted. |
| Bounded-fit scratch reuse | Measured-sample timing had no detected regressions and RSS fell 26–27%, but first full proof at 1 worker total/prove increased 0.659%/0.864%, and 4-worker total 0.317%. Rejected for startup tradeoff; primary matrix remains recorded as successful. |
| Prove-first with original packing | Fresh arithmetic 1 total increased 0.408%; first-proof data had no detected regressions. Fused packing restored. |
| Final restored fused packing + public-link helper | Fresh full primary/first-proof data have no detected proving regressions; exact historical arithmetic executable reused as disclosed above. |

An identical-binary A/A calibration showed a first-process disadvantage that balanced ordering largely cancels; no A/A value was subtracted. A trace/RSS diagnostic located the process maximum in Keccak but found the candidate gap already present in the preceding bridge, so it did not establish Keccak as the cause. These diagnostics are not qualification timings.

## Reproduction and measurement limits

Run the recorded build and campaign argument arrays from the isolated optimization worktree; they are preserved in [source-build.json](source-build.json). Native build flags are `-C target-cpu=native`, release profile and production `falcon-hybrid`, without `bench-internals`. Cached original-Falcon inputs were prepared before timed execution. The benchmark-only `word=off/auto` configuration labels do not override these production binaries; audited protocol identities establish the actual comparison.

Host: AMD Ryzen 9 9950X3D. Main thread CPU 6; worker order `6,7,4,5,2,3,0,1,14,15,12,13,10,11,8,9`, truncated to worker count. Eight workers occupy one L3 group; sixteen span both. Existing auxiliary/verifier affinity remains in force. Builds, tests and audits ran outside timed campaigns.

Exact paired-bootstrap 95% intervals are descriptive, pointwise and unadjusted across cells; six independent process pairs limit precision. A crossing-zero interval is inconclusive, not proof of nonregression. Results apply to this host, binary and workload; historical arithmetic selection is disclosed, and no universal performance guarantee is made. Process RSS includes fixture setup and proof-digest formatting. Raw campaigns, immutable binaries and full audits remain in ignored `.tmp/thread-fix-20261011/`; compact exact results, proof identities and provenance are retained here.
