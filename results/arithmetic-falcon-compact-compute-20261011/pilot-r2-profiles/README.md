# R2 large-batch component profiles

Compact binding still costs more than legacy binding. At security 100, lower bridge rows reduce BitZ costs enough to nearly offset binding at N=1024, but not N=512. At security 128, BitZ savings shrink or reverse in these diagnostics. They do not support enabling this compact configuration generally.

All **64 diagnostic proofs verified** and matched the corresponding untraced R2 proof contents exactly (root, full Debug digest, input/protocol, payload and physical source size). All runs use the same frozen binaries as the pilot, seed 42, one warmup and three measured proofs; no builds, tests or other profiling ran concurrently. Traced latencies are diagnostic and are not substituted into the acceptance gates.

Batch is 1024 throughout. Component deltas below are combined minus v7, in milliseconds. `Residual` is the prove root minus ring, arithmetic, BitZ and PCS; it includes binding construction, prime sampling, joint/setup and other work. It is not exclusively joint proving. Forest is nested inside BitZ; binding is nested inside arithmetic, so those columns must not be added twice. Component medians need not sum exactly to the median root.

| N | Security | Threads | Prove v7 → combined ms | Ring Δ | Arithmetic Δ | Binding Δ | BitZ Δ | Forest Δ | PCS Δ | Residual Δ |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 512 | 100 | 1 | 231.175 → 237.845 | -0.495 | +22.148 | +22.087 | -14.373 | -14.031 | -0.019 | -0.693 |
| 512 | 100 | 8 | 49.862 → 50.736 | -0.170 | +3.057 | +2.996 | -1.772 | -2.143 | -0.016 | -0.025 |
| 1024 | 100 | 1 | 391.558 → 394.066 | -1.851 | +35.059 | +35.083 | -30.659 | -29.691 | -0.045 | +0.164 |
| 1024 | 100 | 8 | 91.601 → 90.612 | -0.562 | +5.859 | +5.856 | -6.451 | -7.057 | -0.039 | +0.460 |
| 512 | 128 | 1 | 245.910 → 271.622 | -0.301 | +25.695 | +25.551 | -0.609 | -0.353 | +1.606 | -0.400 |
| 512 | 128 | 8 | 61.288 → 70.489 | -0.010 | +2.701 | +2.666 | +5.968 | +5.515 | +0.227 | +0.136 |
| 1024 | 128 | 1 | 408.512 → 433.416 | -0.031 | +39.464 | +39.661 | -19.506 | -18.626 | +4.485 | +0.699 |
| 1024 | 128 | 8 | 105.830 → 111.863 | -0.755 | +3.837 | +3.936 | +1.801 | +1.107 | +0.530 | +0.525 |

The available trace has **no internal forest column-sum span**. It measures column packing and total forest time, so the specific column-sum hypothesis cannot be isolated from this binary. R2 doubles the main compact bridge column counts relative to the initial v8 layout by reducing their row exponents, but the observed cost cannot be assigned to column sums alone.

| N | v7 bridge layout (log rows, log columns) | R2 compact layouts |
|---:|---|---|
| 512 | `(13,11)` | `(12,11)`, `(11,11)`, `(10,6)` |
| 1024 | `(13,12)` | `(12,12)`, `(11,12)`, `(10,7)` |

| N | Security | Threads | Binding v7 → combined ms | Prefix accumulators Δ | Prepare tail Δ | Fold tail Δ | Column packing Δ | Prime Δ | PCS grinding Δ |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 512 | 100 | 1 | 119.785 → 141.872 | +14.777 | +3.007 | -5.337 | -0.225 | -0.057 | -0.001 |
| 512 | 100 | 8 | 16.765 → 19.761 | -0.167 | -0.459 | -1.126 | -0.029 | -0.061 | -0.001 |
| 1024 | 100 | 1 | 178.914 → 213.997 | +22.960 | +5.431 | -11.218 | -0.459 | -0.088 | -0.002 |
| 1024 | 100 | 8 | 29.901 → 35.757 | +0.361 | -0.718 | -2.395 | -0.076 | -0.090 | -0.001 |
| 512 | 128 | 1 | 120.556 → 146.107 | +15.100 | +2.918 | -5.451 | -0.216 | +0.188 | +1.326 |
| 512 | 128 | 8 | 18.987 → 21.654 | -0.209 | -2.056 | -1.096 | -0.037 | +0.183 | +0.167 |
| 1024 | 128 | 1 | 179.683 → 219.343 | +23.320 | +5.533 | -10.908 | -0.453 | -0.105 | +4.312 |
| 1024 | 128 | 8 | 33.917 → 37.853 | +0.365 | -4.054 | -2.389 | -0.071 | -0.108 | +0.521 |

One-thread prefix accumulation rises approximately 15–23 ms, and its prepare-tail stage rises approximately 3–6 ms. Tail folding becomes faster but does not offset the whole binding increase. Eight-thread prefix/tail work is mostly flat or faster while total compact binding still rises; remaining compact preparation/initial-dot work is not separately spanned here. Security-128 PCS grinding differs between protocols and contributes up to 4.31 ms in these selected traces; it does not explain the entire binding or forest changes. These are one-input stage diagnostics, not a distributional grinding study.

`stage-summary.json` preserves every available stage; `component-deltas.json` has all paired component values; `pilot-proof-identity.json` contains all 64 proof identity checks. The separate [untraced pilot](../pilot-r2/README.md) retains the acceptance gate failures.
