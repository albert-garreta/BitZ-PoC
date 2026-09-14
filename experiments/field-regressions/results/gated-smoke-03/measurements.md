# GF128 and NTT regression measurements

Allowed slowdown: 1.0%. Ratios are candidate / baseline. Intervals are pointwise 95% hierarchical paired bootstrap CIs.
P95 describes timed-batch averages, not individual-call tails. Missing architectures/cases are unmeasured. Diagnostics cannot be selected.

| Arch | Operation | Size | Variant | Selected | Median ratio [CI] | P95 ratio [CI] | Alloc calls | Status |
|---|---|---|---|---|---|---|---:|---|
| aarch64 | dot | 16 | flock |  | — | — | — | unmeasured |
| aarch64 | dot | 16 | shared |  | — | — | — | unmeasured |
| aarch64 | dot | 16 | wide1 | yes | — | — | — | unmeasured |
| aarch64 | dot | 16 | wide2 |  | — | — | — | unmeasured |
| aarch64 | dot | 16 | wide4 |  | — | — | — | unmeasured |
| aarch64 | dot | 16 | wide8 |  | — | — | — | unmeasured |
| aarch64 | dot | 16 | wide16 |  | — | — | — | unmeasured |
| aarch64 | dot | 16 | scalar_lanes |  | — | — | — | unmeasured |
| aarch64 | dot | 16 | vec2 |  | — | — | — | unmeasured |
| aarch64 | dot | 1024 | flock |  | — | — | — | unmeasured |
| aarch64 | dot | 1024 | shared |  | — | — | — | unmeasured |
| aarch64 | dot | 1024 | wide1 | yes | — | — | — | unmeasured |
| aarch64 | dot | 1024 | wide2 |  | — | — | — | unmeasured |
| aarch64 | dot | 1024 | wide4 |  | — | — | — | unmeasured |
| aarch64 | dot | 1024 | wide8 |  | — | — | — | unmeasured |
| aarch64 | dot | 1024 | wide16 |  | — | — | — | unmeasured |
| aarch64 | dot | 1024 | scalar_lanes |  | — | — | — | unmeasured |
| aarch64 | dot | 1024 | vec2 |  | — | — | — | unmeasured |
| aarch64 | dot | 65536 | flock |  | — | — | — | unmeasured |
| aarch64 | dot | 65536 | shared |  | — | — | — | unmeasured |
| aarch64 | dot | 65536 | wide1 | yes | — | — | — | unmeasured |
| aarch64 | dot | 65536 | wide2 |  | — | — | — | unmeasured |
| aarch64 | dot | 65536 | wide4 |  | — | — | — | unmeasured |
| aarch64 | dot | 65536 | wide8 |  | — | — | — | unmeasured |
| aarch64 | dot | 65536 | wide16 |  | — | — | — | unmeasured |
| aarch64 | dot | 65536 | scalar_lanes |  | — | — | — | unmeasured |
| aarch64 | dot | 65536 | vec2 |  | — | — | — | unmeasured |
| aarch64 | dot | 1048576 | flock |  | — | — | — | unmeasured |
| aarch64 | dot | 1048576 | shared |  | — | — | — | unmeasured |
| aarch64 | dot | 1048576 | wide1 | yes | — | — | — | unmeasured |
| aarch64 | dot | 1048576 | wide2 |  | — | — | — | unmeasured |
| aarch64 | dot | 1048576 | wide4 |  | — | — | — | unmeasured |
| aarch64 | dot | 1048576 | wide8 |  | — | — | — | unmeasured |
| aarch64 | dot | 1048576 | wide16 |  | — | — | — | unmeasured |
| aarch64 | dot | 1048576 | scalar_lanes |  | — | — | — | unmeasured |
| aarch64 | dot | 1048576 | vec2 |  | — | — | — | unmeasured |
| aarch64 | products | 16 | flock |  | — | — | — | unmeasured |
| aarch64 | products | 16 | shared |  | — | — | — | unmeasured |
| aarch64 | products | 16 | shared_unroll2 |  | — | — | — | unmeasured |
| aarch64 | products | 16 | shared_unroll4 |  | — | — | — | unmeasured |
| aarch64 | products | 16 | shared_unroll8 |  | — | — | — | unmeasured |
| aarch64 | products | 16 | scalar_lanes | yes | — | — | — | unmeasured |
| aarch64 | products | 16 | schoolbook |  | — | — | — | unmeasured |
| aarch64 | products | 16 | karatsuba |  | — | — | — | unmeasured |
| aarch64 | products | 16 | karatsuba_barrett |  | — | — | — | unmeasured |
| aarch64 | products | 1024 | flock |  | — | — | — | unmeasured |
| aarch64 | products | 1024 | shared |  | — | — | — | unmeasured |
| aarch64 | products | 1024 | shared_unroll2 |  | — | — | — | unmeasured |
| aarch64 | products | 1024 | shared_unroll4 |  | — | — | — | unmeasured |
| aarch64 | products | 1024 | shared_unroll8 |  | — | — | — | unmeasured |
| aarch64 | products | 1024 | scalar_lanes | yes | — | — | — | unmeasured |
| aarch64 | products | 1024 | schoolbook |  | — | — | — | unmeasured |
| aarch64 | products | 1024 | karatsuba |  | — | — | — | unmeasured |
| aarch64 | products | 1024 | karatsuba_barrett |  | — | — | — | unmeasured |
| aarch64 | products | 65536 | flock |  | — | — | — | unmeasured |
| aarch64 | products | 65536 | shared |  | — | — | — | unmeasured |
| aarch64 | products | 65536 | shared_unroll2 |  | — | — | — | unmeasured |
| aarch64 | products | 65536 | shared_unroll4 |  | — | — | — | unmeasured |
| aarch64 | products | 65536 | shared_unroll8 |  | — | — | — | unmeasured |
| aarch64 | products | 65536 | scalar_lanes | yes | — | — | — | unmeasured |
| aarch64 | products | 65536 | schoolbook |  | — | — | — | unmeasured |
| aarch64 | products | 65536 | karatsuba |  | — | — | — | unmeasured |
| aarch64 | products | 65536 | karatsuba_barrett |  | — | — | — | unmeasured |
| aarch64 | products | 1048576 | flock |  | — | — | — | unmeasured |
| aarch64 | products | 1048576 | shared |  | — | — | — | unmeasured |
| aarch64 | products | 1048576 | shared_unroll2 |  | — | — | — | unmeasured |
| aarch64 | products | 1048576 | shared_unroll4 |  | — | — | — | unmeasured |
| aarch64 | products | 1048576 | shared_unroll8 |  | — | — | — | unmeasured |
| aarch64 | products | 1048576 | scalar_lanes | yes | — | — | — | unmeasured |
| aarch64 | products | 1048576 | schoolbook |  | — | — | — | unmeasured |
| aarch64 | products | 1048576 | karatsuba |  | — | — | — | unmeasured |
| aarch64 | products | 1048576 | karatsuba_barrett |  | — | — | — | unmeasured |
| aarch64 | chain | 16 | flock |  | — | — | — | unmeasured |
| aarch64 | chain | 16 | shared |  | — | — | — | unmeasured |
| aarch64 | chain | 16 | scalar_lanes | yes | — | — | — | unmeasured |
| aarch64 | chain | 16 | schoolbook |  | — | — | — | unmeasured |
| aarch64 | chain | 16 | karatsuba |  | — | — | — | unmeasured |
| aarch64 | chain | 16 | karatsuba_barrett |  | — | — | — | unmeasured |
| aarch64 | chain | 1024 | flock |  | — | — | — | unmeasured |
| aarch64 | chain | 1024 | shared |  | — | — | — | unmeasured |
| aarch64 | chain | 1024 | scalar_lanes | yes | — | — | — | unmeasured |
| aarch64 | chain | 1024 | schoolbook |  | — | — | — | unmeasured |
| aarch64 | chain | 1024 | karatsuba |  | — | — | — | unmeasured |
| aarch64 | chain | 1024 | karatsuba_barrett |  | — | — | — | unmeasured |
| aarch64 | chain | 65536 | flock |  | — | — | — | unmeasured |
| aarch64 | chain | 65536 | shared |  | — | — | — | unmeasured |
| aarch64 | chain | 65536 | scalar_lanes | yes | — | — | — | unmeasured |
| aarch64 | chain | 65536 | schoolbook |  | — | — | — | unmeasured |
| aarch64 | chain | 65536 | karatsuba |  | — | — | — | unmeasured |
| aarch64 | chain | 65536 | karatsuba_barrett |  | — | — | — | unmeasured |
| aarch64 | chain | 1048576 | flock |  | — | — | — | unmeasured |
| aarch64 | chain | 1048576 | shared |  | — | — | — | unmeasured |
| aarch64 | chain | 1048576 | scalar_lanes | yes | — | — | — | unmeasured |
| aarch64 | chain | 1048576 | schoolbook |  | — | — | — | unmeasured |
| aarch64 | chain | 1048576 | karatsuba |  | — | — | — | unmeasured |
| aarch64 | chain | 1048576 | karatsuba_barrett |  | — | — | — | unmeasured |
| aarch64 | square | 16 | flock |  | — | — | — | unmeasured |
| aarch64 | square | 16 | shared | yes | — | — | — | unmeasured |
| aarch64 | square | 1024 | flock |  | — | — | — | unmeasured |
| aarch64 | square | 1024 | shared | yes | — | — | — | unmeasured |
| aarch64 | square | 65536 | flock |  | — | — | — | unmeasured |
| aarch64 | square | 65536 | shared | yes | — | — | — | unmeasured |
| aarch64 | square | 1048576 | flock |  | — | — | — | unmeasured |
| aarch64 | square | 1048576 | shared | yes | — | — | — | unmeasured |
| aarch64 | inverse | 1 | flock |  | — | — | — | unmeasured |
| aarch64 | inverse | 1 | shared | yes | — | — | — | unmeasured |
| aarch64 | inverse | 16 | flock |  | — | — | — | unmeasured |
| aarch64 | inverse | 16 | shared | yes | — | — | — | unmeasured |
| aarch64 | inverse | 1024 | flock |  | — | — | — | unmeasured |
| aarch64 | inverse | 1024 | shared | yes | — | — | — | unmeasured |
| aarch64 | fixed_prepare | zero | existing_prepare |  | — | — | — | unmeasured (diagnostic) |
| aarch64 | fixed_prepare | half | existing_prepare |  | — | — | — | unmeasured (diagnostic) |
| aarch64 | fixed_prepare | full | existing_prepare |  | — | — | — | unmeasured (diagnostic) |
| aarch64 | fixed | zero_n16 | flock |  | — | — | — | unmeasured |
| aarch64 | fixed | zero_n16 | shared |  | — | — | — | unmeasured |
| aarch64 | fixed | zero_n16 | prepared |  | — | — | — | unmeasured |
| aarch64 | fixed | zero_n16 | specialized |  | — | — | — | unmeasured |
| aarch64 | fixed | zero_n16 | scalar_lanes | yes | — | — | — | unmeasured |
| aarch64 | fixed | zero_n1024 | flock |  | — | — | — | unmeasured |
| aarch64 | fixed | zero_n1024 | shared |  | — | — | — | unmeasured |
| aarch64 | fixed | zero_n1024 | prepared |  | — | — | — | unmeasured |
| aarch64 | fixed | zero_n1024 | specialized |  | — | — | — | unmeasured |
| aarch64 | fixed | zero_n1024 | scalar_lanes | yes | — | — | — | unmeasured |
| aarch64 | fixed | zero_n65536 | flock |  | — | — | — | unmeasured |
| aarch64 | fixed | zero_n65536 | shared |  | — | — | — | unmeasured |
| aarch64 | fixed | zero_n65536 | prepared |  | — | — | — | unmeasured |
| aarch64 | fixed | zero_n65536 | specialized |  | — | — | — | unmeasured |
| aarch64 | fixed | zero_n65536 | scalar_lanes | yes | — | — | — | unmeasured |
| aarch64 | fixed | zero_n1048576 | flock |  | — | — | — | unmeasured |
| aarch64 | fixed | zero_n1048576 | shared |  | — | — | — | unmeasured |
| aarch64 | fixed | zero_n1048576 | prepared |  | — | — | — | unmeasured |
| aarch64 | fixed | zero_n1048576 | specialized |  | — | — | — | unmeasured |
| aarch64 | fixed | zero_n1048576 | scalar_lanes | yes | — | — | — | unmeasured |
| aarch64 | fixed | half_n16 | flock |  | — | — | — | unmeasured |
| aarch64 | fixed | half_n16 | shared |  | — | — | — | unmeasured |
| aarch64 | fixed | half_n16 | prepared |  | — | — | — | unmeasured |
| aarch64 | fixed | half_n16 | specialized |  | — | — | — | unmeasured |
| aarch64 | fixed | half_n16 | scalar_lanes | yes | — | — | — | unmeasured |
| aarch64 | fixed | half_n1024 | flock |  | — | — | — | unmeasured |
| aarch64 | fixed | half_n1024 | shared |  | — | — | — | unmeasured |
| aarch64 | fixed | half_n1024 | prepared |  | — | — | — | unmeasured |
| aarch64 | fixed | half_n1024 | specialized |  | — | — | — | unmeasured |
| aarch64 | fixed | half_n1024 | scalar_lanes | yes | — | — | — | unmeasured |
| aarch64 | fixed | half_n65536 | flock |  | — | — | — | unmeasured |
| aarch64 | fixed | half_n65536 | shared |  | — | — | — | unmeasured |
| aarch64 | fixed | half_n65536 | prepared |  | — | — | — | unmeasured |
| aarch64 | fixed | half_n65536 | specialized |  | — | — | — | unmeasured |
| aarch64 | fixed | half_n65536 | scalar_lanes | yes | — | — | — | unmeasured |
| aarch64 | fixed | half_n1048576 | flock |  | — | — | — | unmeasured |
| aarch64 | fixed | half_n1048576 | shared |  | — | — | — | unmeasured |
| aarch64 | fixed | half_n1048576 | prepared |  | — | — | — | unmeasured |
| aarch64 | fixed | half_n1048576 | specialized |  | — | — | — | unmeasured |
| aarch64 | fixed | half_n1048576 | scalar_lanes | yes | — | — | — | unmeasured |
| aarch64 | fixed | full_n16 | flock |  | — | — | — | unmeasured |
| aarch64 | fixed | full_n16 | shared |  | — | — | — | unmeasured |
| aarch64 | fixed | full_n16 | prepared |  | — | — | — | unmeasured |
| aarch64 | fixed | full_n16 | specialized |  | — | — | — | unmeasured |
| aarch64 | fixed | full_n16 | scalar_lanes | yes | — | — | — | unmeasured |
| aarch64 | fixed | full_n1024 | flock |  | — | — | — | unmeasured |
| aarch64 | fixed | full_n1024 | shared |  | — | — | — | unmeasured |
| aarch64 | fixed | full_n1024 | prepared |  | — | — | — | unmeasured |
| aarch64 | fixed | full_n1024 | specialized |  | — | — | — | unmeasured |
| aarch64 | fixed | full_n1024 | scalar_lanes | yes | — | — | — | unmeasured |
| aarch64 | fixed | full_n65536 | flock |  | — | — | — | unmeasured |
| aarch64 | fixed | full_n65536 | shared |  | — | — | — | unmeasured |
| aarch64 | fixed | full_n65536 | prepared |  | — | — | — | unmeasured |
| aarch64 | fixed | full_n65536 | specialized |  | — | — | — | unmeasured |
| aarch64 | fixed | full_n65536 | scalar_lanes | yes | — | — | — | unmeasured |
| aarch64 | fixed | full_n1048576 | flock |  | — | — | — | unmeasured |
| aarch64 | fixed | full_n1048576 | shared |  | — | — | — | unmeasured |
| aarch64 | fixed | full_n1048576 | prepared |  | — | — | — | unmeasured |
| aarch64 | fixed | full_n1048576 | specialized |  | — | — | — | unmeasured |
| aarch64 | fixed | full_n1048576 | scalar_lanes | yes | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n16 | flock |  | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n16 | shared |  | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n16 | prepared |  | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n16 | specialized |  | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n16 | scalar_lanes | yes | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n1024 | flock |  | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n1024 | shared |  | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n1024 | prepared |  | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n1024 | specialized |  | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n1024 | scalar_lanes | yes | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n65536 | flock |  | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n65536 | shared |  | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n65536 | prepared |  | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n65536 | specialized |  | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n65536 | scalar_lanes | yes | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n1048576 | flock |  | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n1048576 | shared |  | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n1048576 | prepared |  | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n1048576 | specialized |  | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n1048576 | scalar_lanes | yes | — | — | — | unmeasured |
| aarch64 | butterfly | half_n16 | flock |  | — | — | — | unmeasured |
| aarch64 | butterfly | half_n16 | shared |  | — | — | — | unmeasured |
| aarch64 | butterfly | half_n16 | prepared |  | — | — | — | unmeasured |
| aarch64 | butterfly | half_n16 | specialized |  | — | — | — | unmeasured |
| aarch64 | butterfly | half_n16 | scalar_lanes | yes | — | — | — | unmeasured |
| aarch64 | butterfly | half_n1024 | flock |  | — | — | — | unmeasured |
| aarch64 | butterfly | half_n1024 | shared |  | — | — | — | unmeasured |
| aarch64 | butterfly | half_n1024 | prepared |  | — | — | — | unmeasured |
| aarch64 | butterfly | half_n1024 | specialized |  | — | — | — | unmeasured |
| aarch64 | butterfly | half_n1024 | scalar_lanes | yes | — | — | — | unmeasured |
| aarch64 | butterfly | half_n65536 | flock |  | — | — | — | unmeasured |
| aarch64 | butterfly | half_n65536 | shared |  | — | — | — | unmeasured |
| aarch64 | butterfly | half_n65536 | prepared |  | — | — | — | unmeasured |
| aarch64 | butterfly | half_n65536 | specialized |  | — | — | — | unmeasured |
| aarch64 | butterfly | half_n65536 | scalar_lanes | yes | — | — | — | unmeasured |
| aarch64 | butterfly | half_n1048576 | flock |  | — | — | — | unmeasured |
| aarch64 | butterfly | half_n1048576 | shared |  | — | — | — | unmeasured |
| aarch64 | butterfly | half_n1048576 | prepared |  | — | — | — | unmeasured |
| aarch64 | butterfly | half_n1048576 | specialized |  | — | — | — | unmeasured |
| aarch64 | butterfly | half_n1048576 | scalar_lanes | yes | — | — | — | unmeasured |
| aarch64 | butterfly | full_n16 | flock |  | — | — | — | unmeasured |
| aarch64 | butterfly | full_n16 | shared |  | — | — | — | unmeasured |
| aarch64 | butterfly | full_n16 | prepared |  | — | — | — | unmeasured |
| aarch64 | butterfly | full_n16 | specialized |  | — | — | — | unmeasured |
| aarch64 | butterfly | full_n16 | scalar_lanes | yes | — | — | — | unmeasured |
| aarch64 | butterfly | full_n1024 | flock |  | — | — | — | unmeasured |
| aarch64 | butterfly | full_n1024 | shared |  | — | — | — | unmeasured |
| aarch64 | butterfly | full_n1024 | prepared |  | — | — | — | unmeasured |
| aarch64 | butterfly | full_n1024 | specialized |  | — | — | — | unmeasured |
| aarch64 | butterfly | full_n1024 | scalar_lanes | yes | — | — | — | unmeasured |
| aarch64 | butterfly | full_n65536 | flock |  | — | — | — | unmeasured |
| aarch64 | butterfly | full_n65536 | shared |  | — | — | — | unmeasured |
| aarch64 | butterfly | full_n65536 | prepared |  | — | — | — | unmeasured |
| aarch64 | butterfly | full_n65536 | specialized |  | — | — | — | unmeasured |
| aarch64 | butterfly | full_n65536 | scalar_lanes | yes | — | — | — | unmeasured |
| aarch64 | butterfly | full_n1048576 | flock |  | — | — | — | unmeasured |
| aarch64 | butterfly | full_n1048576 | shared |  | — | — | — | unmeasured |
| aarch64 | butterfly | full_n1048576 | prepared |  | — | — | — | unmeasured |
| aarch64 | butterfly | full_n1048576 | specialized |  | — | — | — | unmeasured |
| aarch64 | butterfly | full_n1048576 | scalar_lanes | yes | — | — | — | unmeasured |
| aarch64 | ntt | log8_lanes1 | flock |  | — | — | — | unmeasured |
| aarch64 | ntt | log8_lanes1 | preserved_schedule | yes | — | — | — | unmeasured |
| aarch64 | ntt | log8_lanes1 | shared_generic |  | — | — | — | unmeasured |
| aarch64 | ntt | log8_lanes1 | shared_twiddle |  | — | — | — | unmeasured |
| aarch64 | ntt | log8_lanes1 | prepared_twiddle |  | — | — | — | unmeasured |
| aarch64 | ntt | log8_lanes1 | reset_only |  | — | — | — | unmeasured (diagnostic) |
| aarch64 | ntt | log8_lanes32 | flock |  | — | — | — | unmeasured |
| aarch64 | ntt | log8_lanes32 | preserved_schedule | yes | — | — | — | unmeasured |
| aarch64 | ntt | log8_lanes32 | shared_generic |  | — | — | — | unmeasured |
| aarch64 | ntt | log8_lanes32 | shared_twiddle |  | — | — | — | unmeasured |
| aarch64 | ntt | log8_lanes32 | prepared_twiddle |  | — | — | — | unmeasured |
| aarch64 | ntt | log8_lanes32 | reset_only |  | — | — | — | unmeasured (diagnostic) |
| aarch64 | ntt | log12_lanes8 | flock |  | — | — | — | unmeasured |
| aarch64 | ntt | log12_lanes8 | preserved_schedule | yes | — | — | — | unmeasured |
| aarch64 | ntt | log12_lanes8 | shared_generic |  | — | — | — | unmeasured |
| aarch64 | ntt | log12_lanes8 | shared_twiddle |  | — | — | — | unmeasured |
| aarch64 | ntt | log12_lanes8 | prepared_twiddle |  | — | — | — | unmeasured |
| aarch64 | ntt | log12_lanes8 | reset_only |  | — | — | — | unmeasured (diagnostic) |
| aarch64 | ntt | log15_lanes32 | flock |  | — | — | — | unmeasured |
| aarch64 | ntt | log15_lanes32 | preserved_schedule | yes | — | — | — | unmeasured |
| aarch64 | ntt | log15_lanes32 | shared_generic |  | — | — | — | unmeasured |
| aarch64 | ntt | log15_lanes32 | shared_twiddle |  | — | — | — | unmeasured |
| aarch64 | ntt | log15_lanes32 | prepared_twiddle |  | — | — | — | unmeasured |
| aarch64 | ntt | log15_lanes32 | reset_only |  | — | — | — | unmeasured (diagnostic) |
| aarch64 | ntt | log16_lanes32 | flock |  | — | — | — | unmeasured |
| aarch64 | ntt | log16_lanes32 | preserved_schedule | yes | — | — | — | unmeasured |
| aarch64 | ntt | log16_lanes32 | shared_generic |  | — | — | — | unmeasured |
| aarch64 | ntt | log16_lanes32 | shared_twiddle |  | — | — | — | unmeasured |
| aarch64 | ntt | log16_lanes32 | prepared_twiddle |  | — | — | — | unmeasured |
| aarch64 | ntt | log16_lanes32 | reset_only |  | — | — | — | unmeasured (diagnostic) |
| aarch64 | ntt | log17_lanes32 | flock |  | — | — | — | unmeasured |
| aarch64 | ntt | log17_lanes32 | preserved_schedule | yes | — | — | — | unmeasured |
| aarch64 | ntt | log17_lanes32 | shared_generic |  | — | — | — | unmeasured |
| aarch64 | ntt | log17_lanes32 | shared_twiddle |  | — | — | — | unmeasured |
| aarch64 | ntt | log17_lanes32 | prepared_twiddle |  | — | — | — | unmeasured |
| aarch64 | ntt | log17_lanes32 | reset_only |  | — | — | — | unmeasured (diagnostic) |
| aarch64 | ntt | log18_lanes32 | flock |  | — | — | — | unmeasured |
| aarch64 | ntt | log18_lanes32 | preserved_schedule | yes | — | — | — | unmeasured |
| aarch64 | ntt | log18_lanes32 | shared_generic |  | — | — | — | unmeasured |
| aarch64 | ntt | log18_lanes32 | shared_twiddle |  | — | — | — | unmeasured |
| aarch64 | ntt | log18_lanes32 | prepared_twiddle |  | — | — | — | unmeasured |
| aarch64 | ntt | log18_lanes32 | reset_only |  | — | — | — | unmeasured (diagnostic) |
