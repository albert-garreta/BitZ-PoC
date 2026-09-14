# Unified arithmetic benchmark measurements

Allowed slowdown: 1.0%. Ratios are candidate / baseline. Intervals are pointwise 95% hierarchical paired bootstrap CIs.
P95 describes timed-batch averages, not individual-call tails. Missing architectures/cases are unmeasured. Diagnostics cannot be selected.

| Arch | Operation | Size | Variant | Selected | Median ratio [CI] | P95 ratio [CI] | P50 / P95 (µs) | Payload (MiB) | Alloc calls / bytes | Status |
|---|---|---|---|---|---|---|---:|---:|---:|---|
| aarch64 | products | 16 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 16 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 16 | shared_unroll2 |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 16 | shared_unroll4 |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 16 | shared_unroll8 |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 16 | scalar_lanes | yes | — | — | — | — | — | unmeasured |
| aarch64 | products | 16 | schoolbook |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 16 | karatsuba |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 16 | karatsuba_barrett |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 1024 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 1024 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 1024 | shared_unroll2 |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 1024 | shared_unroll4 |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 1024 | shared_unroll8 |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 1024 | scalar_lanes | yes | — | — | — | — | — | unmeasured |
| aarch64 | products | 1024 | schoolbook |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 1024 | karatsuba |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 1024 | karatsuba_barrett |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 65536 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 65536 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 65536 | shared_unroll2 |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 65536 | shared_unroll4 |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 65536 | shared_unroll8 |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 65536 | scalar_lanes | yes | — | — | — | — | — | unmeasured |
| aarch64 | products | 65536 | schoolbook |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 65536 | karatsuba |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 65536 | karatsuba_barrett |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 1048576 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 1048576 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 1048576 | shared_unroll2 |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 1048576 | shared_unroll4 |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 1048576 | shared_unroll8 |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 1048576 | scalar_lanes | yes | — | — | — | — | — | unmeasured |
| aarch64 | products | 1048576 | schoolbook |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 1048576 | karatsuba |  | — | — | — | — | — | unmeasured |
| aarch64 | products | 1048576 | karatsuba_barrett |  | — | — | — | — | — | unmeasured |
| aarch64 | chain | 16 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | chain | 16 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | chain | 16 | scalar_lanes | yes | — | — | — | — | — | unmeasured |
| aarch64 | chain | 16 | schoolbook |  | — | — | — | — | — | unmeasured |
| aarch64 | chain | 16 | karatsuba |  | — | — | — | — | — | unmeasured |
| aarch64 | chain | 16 | karatsuba_barrett |  | — | — | — | — | — | unmeasured |
| aarch64 | chain | 1024 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | chain | 1024 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | chain | 1024 | scalar_lanes | yes | — | — | — | — | — | unmeasured |
| aarch64 | chain | 1024 | schoolbook |  | — | — | — | — | — | unmeasured |
| aarch64 | chain | 1024 | karatsuba |  | — | — | — | — | — | unmeasured |
| aarch64 | chain | 1024 | karatsuba_barrett |  | — | — | — | — | — | unmeasured |
| aarch64 | chain | 65536 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | chain | 65536 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | chain | 65536 | scalar_lanes | yes | — | — | — | — | — | unmeasured |
| aarch64 | chain | 65536 | schoolbook |  | — | — | — | — | — | unmeasured |
| aarch64 | chain | 65536 | karatsuba |  | — | — | — | — | — | unmeasured |
| aarch64 | chain | 65536 | karatsuba_barrett |  | — | — | — | — | — | unmeasured |
| aarch64 | chain | 1048576 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | chain | 1048576 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | chain | 1048576 | scalar_lanes | yes | — | — | — | — | — | unmeasured |
| aarch64 | chain | 1048576 | schoolbook |  | — | — | — | — | — | unmeasured |
| aarch64 | chain | 1048576 | karatsuba |  | — | — | — | — | — | unmeasured |
| aarch64 | chain | 1048576 | karatsuba_barrett |  | — | — | — | — | — | unmeasured |
| aarch64 | square | 16 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | square | 16 | shared | yes | — | — | — | — | — | unmeasured |
| aarch64 | square | 1024 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | square | 1024 | shared | yes | — | — | — | — | — | unmeasured |
| aarch64 | square | 65536 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | square | 65536 | shared | yes | — | — | — | — | — | unmeasured |
| aarch64 | square | 1048576 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | square | 1048576 | shared | yes | — | — | — | — | — | unmeasured |
| aarch64 | inverse | 1 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | inverse | 1 | shared | yes | — | — | — | — | — | unmeasured |
| aarch64 | inverse | 16 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | inverse | 16 | shared | yes | — | — | — | — | — | unmeasured |
| aarch64 | inverse | 1024 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | inverse | 1024 | shared | yes | — | — | — | — | — | unmeasured |
| aarch64 | fixed_prepare | zero | existing_prepare |  | — | — | — | — | — | unmeasured (diagnostic) |
| aarch64 | fixed_prepare | half | existing_prepare |  | — | — | — | — | — | unmeasured (diagnostic) |
| aarch64 | fixed_prepare | full | existing_prepare |  | — | — | — | — | — | unmeasured (diagnostic) |
| aarch64 | fixed | zero_n16 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | zero_n16 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | zero_n16 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | zero_n16 | specialized | yes | — | — | — | — | — | unmeasured |
| aarch64 | fixed | zero_n16 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | zero_n1024 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | zero_n1024 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | zero_n1024 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | zero_n1024 | specialized | yes | — | — | — | — | — | unmeasured |
| aarch64 | fixed | zero_n1024 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | zero_n65536 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | zero_n65536 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | zero_n65536 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | zero_n65536 | specialized | yes | — | — | — | — | — | unmeasured |
| aarch64 | fixed | zero_n65536 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | zero_n1048576 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | zero_n1048576 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | zero_n1048576 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | zero_n1048576 | specialized | yes | — | — | — | — | — | unmeasured |
| aarch64 | fixed | zero_n1048576 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n16 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n16 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n16 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n16 | specialized | yes | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n16 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n1024 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n1024 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n1024 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n1024 | specialized | yes | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n1024 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n65536 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n65536 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n65536 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n65536 | specialized | yes | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n65536 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n1048576 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n1048576 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n1048576 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n1048576 | specialized | yes | — | — | — | — | — | unmeasured |
| aarch64 | fixed | half_n1048576 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | full_n16 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | full_n16 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | full_n16 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | full_n16 | specialized | yes | — | — | — | — | — | unmeasured |
| aarch64 | fixed | full_n16 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | full_n1024 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | full_n1024 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | full_n1024 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | full_n1024 | specialized | yes | — | — | — | — | — | unmeasured |
| aarch64 | fixed | full_n1024 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | full_n65536 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | full_n65536 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | full_n65536 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | full_n65536 | specialized | yes | — | — | — | — | — | unmeasured |
| aarch64 | fixed | full_n65536 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | full_n1048576 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | full_n1048576 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | full_n1048576 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | fixed | full_n1048576 | specialized | yes | — | — | — | — | — | unmeasured |
| aarch64 | fixed | full_n1048576 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n16 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n16 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n16 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n16 | specialized | yes | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n16 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n1024 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n1024 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n1024 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n1024 | specialized | yes | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n1024 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n65536 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n65536 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n65536 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n65536 | specialized | yes | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n65536 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n1048576 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n1048576 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n1048576 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n1048576 | specialized | yes | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | zero_n1048576 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | half_n16 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | half_n16 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | half_n16 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | half_n16 | specialized | yes | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | half_n16 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | half_n1024 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | half_n1024 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | half_n1024 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | half_n1024 | specialized | yes | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | half_n1024 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | half_n65536 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | half_n65536 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | half_n65536 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | half_n65536 | specialized | yes | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | half_n65536 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | half_n1048576 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | half_n1048576 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | half_n1048576 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | half_n1048576 | specialized | yes | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | half_n1048576 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | full_n16 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | full_n16 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | full_n16 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | full_n16 | specialized | yes | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | full_n16 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | full_n1024 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | full_n1024 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | full_n1024 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | full_n1024 | specialized | yes | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | full_n1024 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | full_n65536 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | full_n65536 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | full_n65536 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | full_n65536 | specialized | yes | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | full_n65536 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | full_n1048576 | flock |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | full_n1048576 | shared |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | full_n1048576 | prepared |  | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | full_n1048576 | specialized | yes | — | — | — | — | — | unmeasured |
| aarch64 | butterfly | full_n1048576 | scalar_lanes |  | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 16 | f2z_wide |  | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 16 | flock_eager |  | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 16 | shared_wide1 | yes | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 16 | shared_wide2 |  | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 16 | shared_wide4 |  | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 16 | shared_wide8 |  | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 1024 | f2z_wide |  | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 1024 | flock_eager |  | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 1024 | shared_wide1 | yes | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 1024 | shared_wide2 |  | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 1024 | shared_wide4 |  | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 1024 | shared_wide8 |  | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 65536 | f2z_wide |  | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 65536 | flock_eager |  | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 65536 | shared_wide1 | yes | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 65536 | shared_wide2 |  | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 65536 | shared_wide4 |  | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 65536 | shared_wide8 |  | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 1048576 | f2z_wide |  | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 1048576 | flock_eager |  | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 1048576 | shared_wide1 | yes | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 1048576 | shared_wide2 |  | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 1048576 | shared_wide4 |  | — | — | — | — | — | unmeasured |
| aarch64 | gf_dot | 1048576 | shared_wide8 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_setup | 100 | existing_contexts |  | — | — | — | — | — | unmeasured (diagnostic) |
| aarch64 | prime_setup | 128 | existing_contexts |  | — | — | — | — | — | unmeasured (diagnostic) |
| aarch64 | prime_mul | q100_n16 | raw_ctx |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_mul | q100_n16 | branded | yes | — | — | — | — | — | unmeasured |
| aarch64 | prime_mul | q100_n16 | checked_per_term |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_mul | q100_n16 | configured_field |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_mul | q100_n1024 | raw_ctx |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_mul | q100_n1024 | branded | yes | — | — | — | — | — | unmeasured |
| aarch64 | prime_mul | q100_n1024 | checked_per_term |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_mul | q100_n1024 | configured_field |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_mul | q100_n1048576 | raw_ctx |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_mul | q100_n1048576 | branded | yes | — | — | — | — | — | unmeasured |
| aarch64 | prime_mul | q100_n1048576 | checked_per_term |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_mul | q100_n1048576 | configured_field |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_mul | q128_n16 | raw_ctx |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_mul | q128_n16 | branded | yes | — | — | — | — | — | unmeasured |
| aarch64 | prime_mul | q128_n16 | checked_per_term |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_mul | q128_n16 | configured_field |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_mul | q128_n1024 | raw_ctx |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_mul | q128_n1024 | branded | yes | — | — | — | — | — | unmeasured |
| aarch64 | prime_mul | q128_n1024 | checked_per_term |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_mul | q128_n1024 | configured_field |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_mul | q128_n1048576 | raw_ctx |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_mul | q128_n1048576 | branded | yes | — | — | — | — | — | unmeasured |
| aarch64 | prime_mul | q128_n1048576 | checked_per_term |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_mul | q128_n1048576 | configured_field |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q100_n16 | existing_delayed |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q100_n16 | acc2 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q100_n16 | acc4 | yes | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q100_n16 | acc8 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q100_n16 | checked_per_term |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q100_n16 | eager_raw |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q100_n16 | crypto_bigint_reduce |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q100_n16 | convert_then_delayed |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q100_n1024 | existing_delayed |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q100_n1024 | acc2 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q100_n1024 | acc4 | yes | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q100_n1024 | acc8 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q100_n1024 | checked_per_term |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q100_n1024 | eager_raw |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q100_n1024 | crypto_bigint_reduce |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q100_n1024 | convert_then_delayed |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q100_n1048576 | existing_delayed |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q100_n1048576 | acc2 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q100_n1048576 | acc4 | yes | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q100_n1048576 | acc8 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q100_n1048576 | checked_per_term |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q100_n1048576 | eager_raw |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q100_n1048576 | crypto_bigint_reduce |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q100_n1048576 | convert_then_delayed |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n16 | existing_delayed |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n16 | acc2 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n16 | acc4 | yes | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n16 | acc8 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n16 | checked_per_term |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n16 | eager_raw |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n16 | crypto_bigint_reduce |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n16 | convert_then_delayed |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n1024 | existing_delayed |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n1024 | acc2 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n1024 | acc4 | yes | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n1024 | acc8 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n1024 | checked_per_term |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n1024 | eager_raw |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n1024 | crypto_bigint_reduce |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n1024 | convert_then_delayed |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n1048576 | existing_delayed |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n1048576 | acc2 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n1048576 | acc4 | yes | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n1048576 | acc8 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n1048576 | checked_per_term |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n1048576 | eager_raw |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n1048576 | crypto_bigint_reduce |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_dot | q128_n1048576 | convert_then_delayed |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q100_n16 | existing_delayed |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q100_n16 | acc2 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q100_n16 | acc4 | yes | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q100_n16 | acc8 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q100_n16 | crypto_bigint_reduce |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q100_n1024 | existing_delayed |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q100_n1024 | acc2 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q100_n1024 | acc4 | yes | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q100_n1024 | acc8 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q100_n1024 | crypto_bigint_reduce |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q100_n1048576 | existing_delayed |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q100_n1048576 | acc2 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q100_n1048576 | acc4 | yes | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q100_n1048576 | acc8 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q100_n1048576 | crypto_bigint_reduce |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q128_n16 | existing_delayed |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q128_n16 | acc2 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q128_n16 | acc4 | yes | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q128_n16 | acc8 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q128_n16 | crypto_bigint_reduce |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q128_n1024 | existing_delayed |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q128_n1024 | acc2 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q128_n1024 | acc4 | yes | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q128_n1024 | acc8 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q128_n1024 | crypto_bigint_reduce |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q128_n1048576 | existing_delayed |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q128_n1048576 | acc2 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q128_n1048576 | acc4 | yes | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q128_n1048576 | acc8 |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_linear | q128_n1048576 | crypto_bigint_reduce |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_reduce_product | 100 | existing_optimized |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_reduce_product | 100 | crypto_bigint |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_reduce_product | 128 | existing_optimized |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_reduce_product | 128 | crypto_bigint |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_reduce_linear | 100 | existing_optimized |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_reduce_linear | 100 | crypto_bigint |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_reduce_linear | 128 | existing_optimized |  | — | — | — | — | — | unmeasured |
| aarch64 | prime_reduce_linear | 128 | crypto_bigint |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l1_n16 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l1_n16 | fused | yes | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l1_n16 | checked_per_term |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l1_n1024 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l1_n1024 | fused | yes | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l1_n1024 | checked_per_term |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l1_n65536 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l1_n65536 | fused | yes | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l1_n65536 | checked_per_term |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l2_n16 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l2_n16 | fused | yes | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l2_n16 | checked_per_term |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l2_n1024 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l2_n1024 | fused | yes | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l2_n1024 | checked_per_term |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l2_n65536 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l2_n65536 | fused | yes | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l2_n65536 | checked_per_term |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l4_n16 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l4_n16 | fused | yes | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l4_n16 | checked_per_term |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l4_n1024 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l4_n1024 | fused | yes | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l4_n1024 | checked_per_term |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l4_n65536 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l4_n65536 | fused | yes | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l4_n65536 | checked_per_term |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l9_n16 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l9_n16 | fused | yes | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l9_n16 | checked_per_term |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l9_n1024 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l9_n1024 | fused | yes | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l9_n1024 | checked_per_term |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l9_n65536 | circuit_z |  | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l9_n65536 | fused | yes | — | — | — | — | — | unmeasured |
| aarch64 | integer_mac | l9_n65536 | checked_per_term |  | — | — | — | — | — | unmeasured |
| aarch64 | p256_product | active4_n16 | existing_p256 |  | — | — | — | — | — | unmeasured |
| aarch64 | p256_product | active4_n16 | fixed9 |  | — | — | — | — | — | unmeasured |
| aarch64 | p256_product | active4_n16 | public_bound4 | yes | — | — | — | — | — | unmeasured |
| aarch64 | p256_product | active4_n1024 | existing_p256 |  | — | — | — | — | — | unmeasured |
| aarch64 | p256_product | active4_n1024 | fixed9 |  | — | — | — | — | — | unmeasured |
| aarch64 | p256_product | active4_n1024 | public_bound4 | yes | — | — | — | — | — | unmeasured |
| aarch64 | p256_product | active4_n65536 | existing_p256 |  | — | — | — | — | — | unmeasured |
| aarch64 | p256_product | active4_n65536 | fixed9 |  | — | — | — | — | — | unmeasured |
| aarch64 | p256_product | active4_n65536 | public_bound4 | yes | — | — | — | — | — | unmeasured |
| aarch64 | p256_product | active9_n16 | existing_p256 |  | — | — | — | — | — | unmeasured |
| aarch64 | p256_product | active9_n16 | fixed9 | yes | — | — | — | — | — | unmeasured |
| aarch64 | p256_product | active9_n1024 | existing_p256 |  | — | — | — | — | — | unmeasured |
| aarch64 | p256_product | active9_n1024 | fixed9 | yes | — | — | — | — | — | unmeasured |
| aarch64 | p256_product | active9_n65536 | existing_p256 |  | — | — | — | — | — | unmeasured |
| aarch64 | p256_product | active9_n65536 | fixed9 | yes | — | — | — | — | — | unmeasured |
| aarch64 | projection | q100_l2_n16 | runtime_modulus |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q100_l2_n16 | fixed_crypto_bigint_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q100_l2_n1024 | runtime_modulus |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q100_l2_n1024 | fixed_crypto_bigint_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q100_l4_n16 | runtime_modulus |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q100_l4_n16 | fixed_crypto_bigint_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q100_l4_n1024 | runtime_modulus |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q100_l4_n1024 | fixed_crypto_bigint_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q100_l9_n16 | runtime_modulus |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q100_l9_n16 | fixed_crypto_bigint_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q100_l9_n1024 | runtime_modulus |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q100_l9_n1024 | fixed_crypto_bigint_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q128_l2_n16 | runtime_modulus |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q128_l2_n16 | fixed_crypto_bigint_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q128_l2_n1024 | runtime_modulus |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q128_l2_n1024 | fixed_crypto_bigint_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q128_l4_n16 | runtime_modulus |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q128_l4_n16 | fixed_crypto_bigint_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q128_l4_n1024 | runtime_modulus |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q128_l4_n1024 | fixed_crypto_bigint_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q128_l9_n16 | runtime_modulus |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q128_l9_n16 | fixed_crypto_bigint_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q128_l9_n1024 | runtime_modulus |  | — | — | — | — | — | unmeasured |
| aarch64 | projection | q128_l9_n1024 | fixed_crypto_bigint_vartime |  | — | — | — | — | — | unmeasured |
