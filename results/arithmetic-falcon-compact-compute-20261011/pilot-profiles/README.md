# Initial v8 component diagnosis

The medium Split regressions come mainly from additional BitZ bridge work and compact binding. At N1024/batch32/one thread, combined proving adds 5.71 ms over v7: BitZ adds 4.17 ms, arithmetic adds 0.52 ms, and the residual adds 0.93 ms. Prime sampling and ring checks are effectively unchanged in that case. At large sizes the BitZ savings are offset by slower compact binding, especially with eight threads.

These six root-selected shapes use security 100, seed 42, one warmup and three measured proofs, with `v7`, `padding` and `combined`. All **72 proofs verified** and exactly matched the corresponding untraced pilot proof contents; see [the identity audit](pilot-proof-identity.json). Profiles use the same frozen binaries and no concurrent builds or tests. Tracing adds overhead; these timings diagnose components and are not acceptance timing.

The table reports median milliseconds. Component deltas are combined minus v7, so positive means slower. Arithmetic includes integer norm and binding; the BitZ forest is contained within BitZ. The residual is total proving minus ring, arithmetic, BitZ, and PCS, and includes prime sampling, construction, joint work and other setup. Nested spans overlap and must not be added together.

| N | Batch | Threads | Prove v7 → padding → combined | Ring Δ | Integer outer Δ | Arithmetic Δ | BitZ Δ | PCS Δ | Prime Δ | Residual Δ |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1024 | 32 | 1 | 24.335 → 24.504 → 30.047 | -0.000 | -0.039 | +0.517 | +4.172 | +0.044 | +0.004 | +0.929 |
| 1024 | 32 | 8 | 8.508 → 8.505 → 11.820 | -0.004 | +0.002 | +0.743 | +1.763 | +0.056 | +0.004 | +0.781 |
| 1024 | 1024 | 1 | 390.511 → 389.923 → 389.993 | -0.188 | -0.010 | +23.482 | -24.409 | +0.015 | -0.140 | +0.354 |
| 1024 | 1024 | 8 | 92.945 → 92.072 → 93.002 | -0.203 | -0.028 | +6.511 | -6.499 | +0.011 | -0.138 | +0.108 |
| 512 | 33 | 8 | 7.794 → 7.737 → 11.153 | -0.026 | -0.006 | +0.882 | +1.664 | +0.125 | +0.002 | +0.746 |
| 512 | 1024 | 8 | 49.580 → 49.343 → 52.371 | -0.078 | -0.006 | +3.807 | -1.159 | -0.038 | -0.074 | +0.358 |

| N | Batch | Threads | Binding v7 → padding → compact (ms) | BitZ forest v7 → combined (ms) | Legacy → compact bridge layouts (row vars, col vars) |
|---:|---:|---:|---:|---:|---|
| 1024 | 32 | 1 | 8.004 → 8.111 → 8.560 | 9.702 → 13.396 | `[[13, 7]]` → `[[13, 6], [12, 6], [10, 6]]` |
| 1024 | 32 | 8 | 2.485 → 2.493 → 3.226 | 1.873 → 3.118 | `[[13, 7]]` → `[[13, 6], [12, 6], [10, 6]]` |
| 1024 | 1024 | 1 | 178.682 → 177.961 → 202.161 | 92.052 → 68.664 | `[[13, 12]]` → `[[13, 11], [12, 11], [10, 7]]` |
| 1024 | 1024 | 8 | 30.214 → 30.103 → 36.767 | 18.231 → 11.294 | `[[13, 12]]` → `[[13, 11], [12, 11], [10, 7]]` |
| 512 | 33 | 8 | 2.101 → 2.099 → 2.977 | 1.907 → 3.067 | `[[13, 7]]` → `[[13, 6], [12, 6], [10, 6]]` |
| 512 | 1024 | 8 | 16.808 → 16.788 → 20.600 | 8.834 → 7.210 | `[[13, 11]]` → `[[13, 10], [12, 10], [10, 6]]` |

Medium Split cases replace one legacy bridge layout `[[13,7]]` with three layouts `[[13,6],[12,6],[10,6]]`. Although the logical binding domain shrinks, the three forest computations add work. At N1024/batch1024/one thread, BitZ improves by 24.41 ms but compact binding adds about 23.48 ms; at eight threads the changes are approximately −6.50 ms and +6.51 ms. At N512/batch1024/eight threads, arithmetic adds 3.81 ms while BitZ saves only 1.16 ms. These measured components justify tuning compact binding and bridge structure before full qualification.

Artifacts: [raw stage medians](stage-summary.json), [component deltas](component-deltas.json), [profile-to-pilot proof audit](pilot-proof-identity.json), and all per-run JSON/commands/stderr spans. The parent directory retains the exact diagnostic plan and variant manifest.
