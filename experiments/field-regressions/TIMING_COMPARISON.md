# Baseline versus experimental arithmetic timings

These are saved operation-level measurements on the M1 Max (aarch64), using Rust 1.98.1 release builds with `-C target-cpu=native`. They measure the frozen experimental executable, not an integrated unified library or the end-to-end prover. No new benchmark was run to produce this table. These measurements precede the subsequent zero-dimensional NTT constructor correctness fix.

Times are measured medians per complete workload, in microseconds (µs), shown to six decimal places; this display precision is not an accuracy guarantee. `n1024` means a complete batch of 1,024 terms/items. Limbs contain 64 bits. `log16` means 2^16 points, and lane counts are explicit. Baseline and candidate medians come from the same measurement round.

The paired time ratio and its 95% confidence interval are copied from the saved gate results. They are calculated from paired process medians and can differ from dividing the two pooled medians shown here. A ratio below 1 is faster. Passing the 1% nonregression gate does not by itself demonstrate a speedup. P95 measures timed-batch averages, not individual-call latency tails.

An existing baseline retained in the selection is identified explicitly. Some baselines compare variants within the experiment (for example, scratch reuse), so their names must be considered before interpreting the comparison as a production replacement. Public-input specializations do not establish private-input speedups.

## 1 thread(s): repair-arm-02

Saved run: `2026-09-14T09:47:20-0700`. Five processes × 32 samples initially; the single-thread run also includes the prescribed five-process × 64-sample retry for selected inconclusive workloads. Full measurements: [source](/Users/johnwu/code/zk/f2z-pcs/experiments/field-regressions/results/repair-arm-02/measurements.md). Frozen binary SHA-256: `9d118faf1c2cb03e28e8cc23cadd3825780642e96eb98b6151ffc3fdf6d74815`.

271 selected checks; 105 retain their actual baseline.

### Frozen selections

| Operation / workload | Baseline kernel | Candidate kernel | Baseline median (µs) | Candidate median (µs) | Paired time ratio [95% CI] | Status |
|---|---|---|---:|---:|---|---|
| opt_integer_mac / l1_signed16_n16 | circuit_z | circuit_z | 0.005664 | 0.005664 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_integer_mac / l1_signed16_n1024 | circuit_z | circuit_z | 0.333630 | 0.333630 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_integer_mac / l1_signed16_n65536 | circuit_z | circuit_z | 21.015054 | 21.015054 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_integer_mac / l1_full_n16 | circuit_z | circuit_z | 0.005616 | 0.005616 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_integer_mac / l1_full_n1024 | circuit_z | circuit_z | 0.332678 | 0.332678 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_integer_mac / l1_full_n65536 | circuit_z | circuit_z | 21.168782 | 21.168782 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_integer_mac / l2_signed16_n16 | circuit_z | incumbent | 0.012981 | 0.010922 | 0.836831 [0.831418, 0.847008] | pass |
| opt_integer_mac / l2_signed16_n1024 | circuit_z | incumbent | 0.699056 | 0.664957 | 0.952542 [0.944282, 0.956679] | pass |
| opt_integer_mac / l2_signed16_n65536 | circuit_z | incumbent | 44.293781 | 42.169110 | 0.951313 [0.945107, 0.958692] | pass |
| opt_integer_mac / l2_full_n16 | circuit_z | incumbent | 0.012884 | 0.010851 | 0.841272 [0.831968, 0.847933] | pass |
| opt_integer_mac / l2_full_n1024 | circuit_z | incumbent | 0.698082 | 0.665418 | 0.952770 [0.949620, 0.959937] | pass |
| opt_integer_mac / l2_full_n65536 | circuit_z | incumbent | 44.114098 | 42.069496 | 0.953190 [0.947194, 0.956835] | pass |
| opt_integer_mac / l9_signed16_n16 | circuit_z | incumbent | 0.348171 | 0.281364 | 0.809221 [0.803504, 0.815690] | pass |
| opt_integer_mac / l9_signed16_n1024 | circuit_z | incumbent | 22.174234 | 17.780846 | 0.804805 [0.794881, 0.811564] | pass |
| opt_integer_mac / l9_signed16_n65536 | circuit_z | incumbent | 1426.067625 | 1162.343875 | 0.814032 [0.805481, 0.821226] | pass |
| opt_integer_mac / l9_full_n16 | circuit_z | incumbent | 0.347919 | 0.281122 | 0.808600 [0.803805, 0.814027] | pass |
| opt_integer_mac / l9_full_n1024 | circuit_z | incumbent | 22.211752 | 18.058270 | 0.809877 [0.803884, 0.821090] | pass |
| opt_integer_mac / l9_full_n65536 | circuit_z | incumbent | 1421.390500 | 1150.291625 | 0.811299 [0.802606, 0.813908] | pass |
| opt_integer_mac / l4_signed16_n16 | circuit_z | native128x2 | 0.053720 | 0.049264 | 0.915618 [0.907043, 0.923416] | pass |
| opt_integer_mac / l4_signed16_n1024 | circuit_z | native128x2 | 3.201335 | 2.904439 | 0.906858 [0.898934, 0.916190] | pass |
| opt_integer_mac / l4_signed16_n65536 | circuit_z | native128x2 | 208.212250 | 192.321625 | 0.922748 [0.913447, 0.928929] | pass |
| opt_integer_mac / l4_full_n16 | circuit_z | native128x2 | 0.053629 | 0.049435 | 0.920455 [0.914863, 0.925384] | pass |
| opt_integer_mac / l4_full_n1024 | circuit_z | native128x2 | 3.207906 | 2.917765 | 0.914926 [0.901125, 0.922554] | pass |
| opt_integer_mac / l4_full_n65536 | circuit_z | native128x2 | 210.161438 | 193.128906 | 0.924034 [0.906751, 0.933456] | pass |
| opt_projection / q100_l2_n16 | production | horner_prepared | 2.948507 | 0.123860 | 0.041988 [0.041354, 0.043046] | pass |
| opt_projection / q100_l2_n16 | production | horner_one_shot | 2.948507 | 0.281515 | 0.094880 [0.093971, 0.097349] | pass |
| opt_projection / q100_l2_n1024 | production | horner_prepared | 581.716188 | 7.781250 | 0.013390 [0.013198, 0.013574] | pass |
| opt_projection / q100_l2_n1024 | production | horner_one_shot | 581.716188 | 7.986937 | 0.013762 [0.013579, 0.013902] | pass |
| opt_projection / q128_l2_n16 | production | horner_prepared | 0.317666 | 0.124495 | 0.396332 [0.379583, 0.480062] | pass |
| opt_projection / q128_l2_n1024 | production | horner_prepared | 23.171063 | 7.711507 | 0.333076 [0.328215, 0.336539] | pass |
| opt_projection / q128_l2_n1024 | production | horner_one_shot | 23.171063 | 7.922852 | 0.342507 [0.336076, 0.345188] | pass |
| opt_projection / q100_l4_n16 | production | horner_prepared | 5.785318 | 0.470215 | 0.081727 [0.072965, 0.084548] | pass |
| opt_projection / q100_l4_n16 | production | horner_one_shot | 5.785318 | 0.643962 | 0.113063 [0.098848, 0.114928] | pass |
| opt_projection / q100_l4_n1024 | production | horner_prepared | 1385.859375 | 29.833250 | 0.021656 [0.021195, 0.021769] | pass |
| opt_projection / q100_l4_n1024 | production | horner_one_shot | 1385.859375 | 30.052000 | 0.021760 [0.021461, 0.021942] | pass |
| opt_projection / q128_l4_n16 | production | horner_prepared | 1.044479 | 0.470057 | 0.460152 [0.402822, 0.473315] | pass |
| opt_projection / q128_l4_n16 | production | horner_one_shot | 1.044479 | 0.640625 | 0.617797 [0.543131, 0.647252] | pass |
| opt_projection / q128_l4_n1024 | production | horner_prepared | 84.157547 | 29.971679 | 0.357207 [0.352308, 0.361089] | pass |
| opt_projection / q128_l4_n1024 | production | horner_one_shot | 84.157547 | 30.218422 | 0.359209 [0.354217, 0.363391] | pass |
| opt_projection / q100_l9_n16 | production | horner_prepared | 28.552410 | 1.506348 | 0.052527 [0.050519, 0.056100] | pass |
| opt_projection / q100_l9_n16 | production | horner_one_shot | 28.552410 | 1.758300 | 0.060079 [0.058610, 0.065645] | pass |
| opt_projection / q100_l9_n1024 | production | horner_prepared | 3395.937500 | 95.750000 | 0.028207 [0.027956, 0.028339] | pass |
| opt_projection / q100_l9_n1024 | production | horner_one_shot | 3395.937500 | 96.208500 | 0.028356 [0.028114, 0.028484] | pass |
| opt_projection / q128_l9_n16 | production | horner_prepared | 2.925476 | 1.512899 | 0.512582 [0.489006, 0.552630] | pass |
| opt_projection / q128_l9_n16 | production | horner_one_shot | 2.925476 | 1.747050 | 0.596697 [0.563706, 0.642538] | pass |
| opt_projection / q128_l9_n1024 | production | horner_prepared | 235.514313 | 95.778656 | 0.406490 [0.401173, 0.412570] | pass |
| opt_projection / q128_l9_n1024 | production | horner_one_shot | 235.514313 | 96.074219 | 0.408258 [0.402034, 0.413043] | pass |
| opt_projection_setup / q100_l2 | horner | horner | 0.158078 | 0.158078 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_projection_setup / q128_l2 | horner | horner | 0.149164 | 0.149164 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_projection_setup / q100_l4 | horner | horner | 0.177534 | 0.177534 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_projection_setup / q128_l4 | horner | horner | 0.170295 | 0.170295 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_projection_setup / q100_l9 | horner | horner | 0.228902 | 0.228902 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_projection_setup / q128_l9 | horner | horner | 0.222630 | 0.222630 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_divrem64 / l2_n16 | public_divisor | prepared_reciprocal | 0.376654 | 0.099360 | 0.263995 [0.262963, 0.265746] | pass |
| opt_divrem64 / l2_n1024 | public_divisor | prepared_reciprocal | 23.899902 | 6.333660 | 0.264246 [0.262002, 0.266793] | pass |
| opt_divrem64 / l4_n16 | public_divisor | prepared_reciprocal | 0.633074 | 0.250391 | 0.394408 [0.391619, 0.397870] | pass |
| opt_divrem64 / l4_n1024 | public_divisor | prepared_reciprocal | 40.516277 | 16.019852 | 0.395094 [0.393162, 0.398576] | pass |
| opt_divrem64 / l9_n16 | public_divisor | prepared_reciprocal | 1.285736 | 0.701391 | 0.544526 [0.541055, 0.548784] | pass |
| opt_divrem64 / l9_n1024 | public_divisor | prepared_reciprocal | 82.084641 | 44.726883 | 0.544259 [0.538568, 0.547606] | pass |
| opt_divrem64 / l32_n16 | public_divisor | prepared_reciprocal | 4.907999 | 4.027507 | 0.818978 [0.814315, 0.826299] | pass |
| opt_divrem64 / l32_n1024 | public_divisor | prepared_reciprocal | 312.981781 | 257.278625 | 0.821805 [0.814857, 0.828575] | pass |
| opt_divrem64 / l64_n16 | public_divisor | prepared_reciprocal | 9.456584 | 8.074991 | 0.852393 [0.846950, 0.861324] | pass |
| opt_divrem64 / l64_n1024 | public_divisor | prepared_reciprocal | 612.028625 | 520.651062 | 0.846244 [0.841810, 0.855849] | pass |
| opt_uint_add / l1_n16 | circuit_z | circuit_z | 0.004921 | 0.004921 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_uint_add / l1_n1024 | circuit_z | circuit_z | 0.211863 | 0.211863 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_uint_add / l2_n16 | circuit_z | native_batch | 0.011608 | 0.008515 | 0.734232 [0.719472, 0.739431] | pass |
| opt_uint_add / l2_n1024 | circuit_z | native_batch | 0.664986 | 0.459610 | 0.690059 [0.684048, 0.694310] | pass |
| opt_uint_add / l4_n16 | circuit_z | native_batch | 0.016018 | 0.012475 | 0.778149 [0.772317, 0.782258] | pass |
| opt_uint_add / l4_n1024 | circuit_z | native_batch | 0.954804 | 0.707245 | 0.742637 [0.736106, 0.757052] | pass |
| opt_uint_add / l9_n16 | circuit_z | native_batch | 0.060295 | 0.046339 | 0.765438 [0.753144, 0.815867] | pass |
| opt_uint_add / l9_n1024 | circuit_z | native_batch | 3.593485 | 3.102396 | 0.862579 [0.851694, 0.872601] | pass |
| opt_uint_sub / l1_n16 | circuit_z | circuit_z | 0.004969 | 0.004969 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_uint_sub / l1_n1024 | circuit_z | circuit_z | 0.211916 | 0.211916 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_uint_sub / l2_n16 | circuit_z | native_batch | 0.011572 | 0.008433 | 0.728169 [0.723907, 0.733997] | pass |
| opt_uint_sub / l2_n1024 | circuit_z | native_batch | 0.661361 | 0.456940 | 0.692204 [0.685158, 0.695476] | pass |
| opt_uint_sub / l4_n16 | circuit_z | circuit_z | 0.022019 | 0.022019 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_uint_sub / l4_n1024 | circuit_z | circuit_z | 1.341583 | 1.341583 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_uint_sub / l9_n16 | circuit_z | circuit_z | 0.062153 | 0.062153 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_uint_sub / l9_n1024 | circuit_z | circuit_z | 3.978048 | 3.978048 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_uint_checked_add / l1_n16 | vendor_option | limb_option | 0.019398 | 0.008758 | 0.450829 [0.446533, 0.453761] | pass |
| opt_uint_checked_add / l1_n1024 | vendor_option | vendor_option | 0.489138 | 0.489138 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_uint_checked_add / l2_n16 | vendor_option | limb_option | 0.014073 | 0.012498 | 0.892735 [0.829038, 0.900769] | pass |
| opt_uint_checked_add / l2_n1024 | vendor_option | limb_option | 0.726862 | 0.666387 | 0.918070 [0.913656, 0.923701] | pass |
| opt_uint_checked_add / l4_n16 | vendor_option | limb_option | 0.044944 | 0.015426 | 0.350111 [0.323225, 0.358031] | pass |
| opt_uint_checked_add / l4_n1024 | vendor_option | limb_option | 3.616679 | 0.880574 | 0.243418 [0.238569, 0.252827] | pass |
| opt_uint_checked_add / l9_n16 | vendor_option | limb_option | 0.083754 | 0.059419 | 0.698842 [0.695499, 0.728573] | pass |
| opt_uint_checked_add / l9_n1024 | vendor_option | limb_option | 7.181192 | 4.136556 | 0.571245 [0.561934, 0.589828] | pass |
| opt_uint_checked_sub / l1_n16 | vendor_option | native_option | 0.018950 | 0.008723 | 0.458110 [0.453850, 0.463827] | pass |
| opt_uint_checked_sub / l1_n1024 | vendor_option | vendor_option | 0.464996 | 0.464996 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_uint_checked_sub / l2_n16 | vendor_option | native_option | 0.021338 | 0.012451 | 0.583789 [0.552135, 0.587344] | pass |
| opt_uint_checked_sub / l2_n1024 | vendor_option | native_option | 1.192993 | 0.666474 | 0.559061 [0.555118, 0.562000] | pass |
| opt_uint_checked_sub / l4_n16 | vendor_option | native_option | 0.043251 | 0.025114 | 0.579516 [0.572191, 0.584276] | pass |
| opt_uint_checked_sub / l4_n1024 | vendor_option | native_option | 3.382873 | 1.811626 | 0.531164 [0.522776, 0.543756] | pass |
| opt_uint_checked_sub / l9_n16 | vendor_option | native_option | 0.092745 | 0.066288 | 0.710293 [0.695545, 0.731983] | pass |
| opt_uint_checked_sub / l9_n1024 | vendor_option | native_option | 7.376139 | 4.137248 | 0.560388 [0.555652, 0.566810] | pass |
| opt_int_checked_add / l1_n16 | vendor_option | word_option | 0.021777 | 0.012218 | 0.562461 [0.550645, 0.589559] | pass |
| opt_int_checked_add / l1_n1024 | vendor_option | word_option | 1.223745 | 0.704895 | 0.576796 [0.562784, 0.582621] | pass |
| opt_int_checked_add / l2_n16 | vendor_option | word_option | 0.020930 | 0.017880 | 0.857495 [0.845626, 0.864315] | pass |
| opt_int_checked_add / l2_n1024 | vendor_option | word_option | 1.628591 | 0.761790 | 0.466454 [0.458611, 0.477200] | pass |
| opt_int_checked_add / l4_n16 | vendor_option | word_option | 0.047930 | 0.024375 | 0.516636 [0.473382, 0.538561] | pass |
| opt_int_checked_add / l4_n1024 | vendor_option | word_option | 3.588297 | 1.195476 | 0.334912 [0.329801, 0.339327] | pass |
| opt_int_checked_add / l9_n16 | vendor_option | word_option | 0.094306 | 0.054069 | 0.575629 [0.569986, 0.583670] | pass |
| opt_int_checked_add / l9_n1024 | vendor_option | word_option | 7.410889 | 3.383504 | 0.455530 [0.446371, 0.460426] | pass |
| opt_int_checked_sub / l1_n16 | vendor_option | word_option | 0.017546 | 0.012136 | 0.695102 [0.683029, 0.700769] | pass |
| opt_int_checked_sub / l1_n1024 | vendor_option | word_option | 1.077952 | 0.706686 | 0.673153 [0.622718, 0.684996] | pass |
| opt_int_checked_sub / l2_n16 | vendor_option | word_option | 0.020790 | 0.017882 | 0.856969 [0.852508, 0.865194] | pass |
| opt_int_checked_sub / l2_n1024 | vendor_option | word_option | 1.414515 | 0.764201 | 0.530669 [0.514238, 0.578621] | pass |
| opt_int_checked_sub / l4_n16 | vendor_option | word_option | 0.051541 | 0.027235 | 0.531985 [0.478155, 0.546342] | pass |
| opt_int_checked_sub / l4_n1024 | vendor_option | word_option | 4.017924 | 1.381571 | 0.346425 [0.330070, 0.351136] | pass |
| opt_int_checked_sub / l9_n16 | vendor_option | word_option | 0.113877 | 0.066345 | 0.585160 [0.561185, 0.587367] | pass |
| opt_int_checked_sub / l9_n1024 | vendor_option | word_option | 8.861328 | 4.022745 | 0.449867 [0.447408, 0.465358] | pass |
| opt_exact_product / a1_b1_n16 | crypto_bigint | crypto_bigint | 0.006308 | 0.006308 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_exact_product / a1_b1_n1024 | crypto_bigint | crypto_bigint | 0.334585 | 0.334585 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_exact_product / a2_b2_n16 | crypto_bigint | crypto_bigint | 0.023075 | 0.023075 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_exact_product / a2_b2_n1024 | crypto_bigint | crypto_bigint | 1.342687 | 1.342687 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_exact_product / a4_b4_n16 | crypto_bigint | crypto_bigint | 0.107291 | 0.107291 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_exact_product / a4_b4_n1024 | crypto_bigint | crypto_bigint | 6.428101 | 6.428101 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_exact_product / a9_b9_n16 | crypto_bigint | crypto_bigint | 0.795125 | 0.795125 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_exact_product / a9_b9_n1024 | crypto_bigint | crypto_bigint | 50.940430 | 50.940430 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_exact_product / a2_b9_n16 | crypto_bigint | crypto_bigint | 0.116384 | 0.116384 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_exact_product / a2_b9_n1024 | crypto_bigint | crypto_bigint | 7.591552 | 7.591552 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_exact_product / a32_b32_n16 | crypto_bigint | crypto_bigint | 10.564494 | 10.564494 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_exact_product / a32_b32_n1024 | crypto_bigint | crypto_bigint | 677.486937 | 677.486937 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_exact_product / a64_b64_n16 | crypto_bigint | crypto_bigint | 37.287274 | 37.287274 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_exact_product / a64_b64_n1024 | crypto_bigint | crypto_bigint | 2394.156250 | 2394.156250 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_exact_signed_mac / l2_n16 | wide_then_add | fused_exact | 0.077010 | 0.061153 | 0.800289 [0.782297, 0.803328] | pass |
| opt_exact_signed_mac / l2_n1024 | wide_then_add | fused_exact | 4.822592 | 3.784932 | 0.785301 [0.778915, 0.794946] | pass |
| opt_exact_signed_mac / l4_n16 | wide_then_add | fused_exact | 0.226988 | 0.219151 | 0.963269 [0.955159, 0.972375] | pass |
| opt_exact_signed_mac / l4_n1024 | wide_then_add | fused_exact | 14.241455 | 13.771810 | 0.966477 [0.953529, 0.974371] | pass |
| opt_exact_signed_mac / l9_n16 | wide_then_add | wide_then_add | 1.087987 | 1.087987 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_exact_signed_mac / l9_n1024 | wide_then_add | wide_then_add | 69.583343 | 69.583343 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_exact_unsigned_mac / l2_n16 | wide_then_add | column_exact | 0.040706 | 0.036693 | 0.913209 [0.814636, 0.917350] | pass |
| opt_exact_unsigned_mac / l2_n1024 | wide_then_add | column_exact | 2.474752 | 2.106435 | 0.849607 [0.841966, 0.853636] | pass |
| opt_exact_unsigned_mac / l4_n16 | wide_then_add | column_exact | 0.149676 | 0.128743 | 0.868474 [0.838071, 0.873336] | pass |
| opt_exact_unsigned_mac / l4_n1024 | wide_then_add | column_exact | 9.287313 | 7.924723 | 0.854705 [0.835702, 0.863244] | pass |
| opt_exact_unsigned_mac / l9_n16 | wide_then_add | column_exact | 0.896433 | 0.682953 | 0.764084 [0.753251, 0.768795] | pass |
| opt_exact_unsigned_mac / l9_n1024 | wide_then_add | column_exact | 57.395844 | 42.494140 | 0.739471 [0.731982, 0.758484] | pass |
| opt_prime_dot / q100_n16 | one_acc | one_acc | 0.059916 | 0.059916 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_prime_dot / q100_n1024 | one_acc | one_acc | 2.439931 | 2.439931 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_prime_dot / q100_n65536 | one_acc | one_acc | 155.315750 | 155.315750 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_prime_dot / q128_n16 | one_acc | one_acc | 0.059498 | 0.059498 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_prime_dot / q128_n1024 | one_acc | one_acc | 2.460815 | 2.460815 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_prime_dot / q128_n65536 | one_acc | one_acc | 155.715500 | 155.715500 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_prime_linear / q100_n16 | one_acc | one_acc | 0.040971 | 0.040971 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_prime_linear / q100_n1024 | one_acc | acc4 | 1.521026 | 1.343761 | 0.886795 [0.880236, 0.900811] | pass |
| opt_prime_linear / q100_n65536 | one_acc | acc4 | 96.666672 | 83.798172 | 0.868617 [0.845232, 0.879274] | pass |
| opt_prime_linear / q128_n16 | one_acc | one_acc | 0.040746 | 0.040746 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_prime_linear / q128_n1024 | one_acc | acc4 | 1.538971 | 1.365255 | 0.887954 [0.881352, 0.894723] | pass |
| opt_prime_linear / q128_n65536 | one_acc | acc4 | 97.067703 | 83.864593 | 0.870008 [0.860701, 0.875642] | pass |
| opt_batch_inverse / q100_nonzero_n16 | production_one_inverse | production_one_inverse | 1.169597 | 1.169597 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_batch_inverse / q100_nonzero_n1024 | production_one_inverse | production_one_inverse | 27.006024 | 27.006024 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_batch_inverse / q100_mixed_n16 | production_one_inverse | production_one_inverse | 1.037979 | 1.037979 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_batch_inverse / q100_mixed_n1024 | production_one_inverse | production_one_inverse | 21.199707 | 21.199707 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_batch_inverse / q100_zero_n16 | production_one_inverse | production_one_inverse | 0.084921 | 0.084921 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_batch_inverse / q100_zero_n1024 | production_one_inverse | production_one_inverse | 4.267882 | 4.267882 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_batch_inverse / q128_nonzero_n16 | production_one_inverse | production_one_inverse | 1.304260 | 1.304260 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_batch_inverse / q128_nonzero_n1024 | production_one_inverse | production_one_inverse | 26.723629 | 26.723629 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_batch_inverse / q128_mixed_n16 | production_one_inverse | production_one_inverse | 1.172806 | 1.172806 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_batch_inverse / q128_mixed_n1024 | production_one_inverse | production_one_inverse | 20.996988 | 20.996988 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_batch_inverse / q128_zero_n16 | production_one_inverse | production_one_inverse | 0.084754 | 0.084754 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_batch_inverse / q128_zero_n1024 | production_one_inverse | production_one_inverse | 4.243184 | 4.243184 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_batch_inverse_reuse / q100_nonzero_n16 | compact_vartime | compact_vartime | 1.085357 | 1.085357 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_batch_inverse_reuse / q100_nonzero_n1024 | compact_vartime | compact_vartime | 24.718422 | 24.718422 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_batch_inverse_reuse / q100_mixed_n16 | compact_vartime | compact_vartime | 1.091818 | 1.091818 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_batch_inverse_reuse / q100_mixed_n1024 | compact_vartime | compact_vartime | 24.510094 | 24.510094 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_batch_inverse_reuse / q100_zero_n16 | compact_vartime | compact_vartime | 0.909226 | 0.909226 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_batch_inverse_reuse / q100_zero_n1024 | compact_vartime | compact_vartime | 24.254231 | 24.254231 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_batch_inverse_reuse / q128_nonzero_n16 | compact_vartime | compact_vartime | 1.224421 | 1.224421 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_batch_inverse_reuse / q128_nonzero_n1024 | compact_vartime | compact_vartime | 24.424481 | 24.424481 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_batch_inverse_reuse / q128_mixed_n16 | compact_vartime | compact_vartime | 1.236008 | 1.236008 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_batch_inverse_reuse / q128_mixed_n1024 | compact_vartime | compact_vartime | 24.253906 | 24.253906 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_batch_inverse_reuse / q128_zero_n16 | compact_vartime | compact_vartime | 1.224808 | 1.224808 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_batch_inverse_reuse / q128_zero_n1024 | compact_vartime | compact_vartime | 24.335449 | 24.335449 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_prime_public_pow / e17_n16 | bounded_window | public_binary | 6.746501 | 2.515259 | 0.373281 [0.371394, 0.374530] | pass |
| opt_prime_public_pow / e17_n1024 | bounded_window | public_binary | 428.632875 | 160.099000 | 0.373460 [0.372563, 0.374662] | pass |
| opt_prime_public_pow / e127_n16 | bounded_window | public_binary | 31.054363 | 22.102211 | 0.711648 [0.707925, 0.715322] | pass |
| opt_prime_public_pow / e127_n1024 | bounded_window | public_binary | 1993.760500 | 1417.844000 | 0.712382 [0.705676, 0.718432] | pass |
| opt_gf_fixed / zero_n16 | actual_fixed_gf | public_scalar_dispatch | 0.012737 | 0.007636 | 0.598650 [0.595952, 0.604405] | pass |
| opt_gf_fixed / zero_n1024 | actual_fixed_gf | public_scalar_dispatch | 0.742579 | 0.339620 | 0.458762 [0.455494, 0.460207] | pass |
| opt_gf_fixed / zero_n65536 | actual_fixed_gf | public_scalar_dispatch | 47.634118 | 21.165359 | 0.444686 [0.439825, 0.450064] | pass |
| opt_gf_fixed / half_n16 | actual_fixed_gf | public_scalar_dispatch | 0.012753 | 0.009352 | 0.730348 [0.727204, 0.739185] | pass |
| opt_gf_fixed / half_n1024 | actual_fixed_gf | public_scalar_dispatch | 0.743047 | 0.456370 | 0.612890 [0.611357, 0.615829] | pass |
| opt_gf_fixed / half_n65536 | actual_fixed_gf | public_scalar_dispatch | 47.674484 | 29.060871 | 0.608930 [0.606762, 0.615076] | pass |
| opt_gf_fixed / full_n16 | actual_fixed_gf | actual_fixed_gf | 0.012781 | 0.012781 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_gf_fixed / full_n1024 | actual_fixed_gf | actual_fixed_gf | 0.744197 | 0.744197 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_gf_fixed / full_n65536 | actual_fixed_gf | actual_fixed_gf | 48.031898 | 48.031898 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_gf_butterfly / zero_n16 | actual_fixed_gf | public_scalar_dispatch | 0.015664 | 0.013154 | 0.841064 [0.823923, 0.844300] | pass |
| opt_gf_butterfly / zero_n1024 | actual_fixed_gf | public_scalar_dispatch | 0.917185 | 0.764353 | 0.833156 [0.827182, 0.840547] | pass |
| opt_gf_butterfly / zero_n65536 | actual_fixed_gf | public_scalar_dispatch | 59.472336 | 49.138672 | 0.827766 [0.816959, 0.836957] | pass |
| opt_gf_butterfly / half_n16 | actual_fixed_gf | public_scalar_dispatch | 0.015659 | 0.013277 | 0.848160 [0.841194, 0.856508] | pass |
| opt_gf_butterfly / half_n1024 | actual_fixed_gf | public_scalar_dispatch | 0.918869 | 0.682571 | 0.742027 [0.739397, 0.747409] | pass |
| opt_gf_butterfly / half_n65536 | actual_fixed_gf | public_scalar_dispatch | 59.349609 | 44.217454 | 0.744828 [0.737257, 0.764970] | pass |
| opt_gf_butterfly / full_n16 | actual_fixed_gf | actual_fixed_gf | 0.015666 | 0.015666 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_gf_butterfly / full_n1024 | actual_fixed_gf | actual_fixed_gf | 0.921844 | 0.921844 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_gf_butterfly / full_n65536 | actual_fixed_gf | actual_fixed_gf | 59.861328 | 59.861328 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_gf_round / 16 | production_fused | wide1 | 0.092904 | 0.075501 | 0.812411 [0.807857, 0.818720] | pass |
| opt_gf_round / 1024 | production_fused | wide1 | 5.663025 | 4.616068 | 0.814925 [0.810550, 0.817477] | pass |
| opt_gf_round / 65536 | production_fused | wide1 | 362.427094 | 296.786468 | 0.816364 [0.812419, 0.824726] | pass |
| opt_gf8_mul / 16 | flock_scalar | native_batch | 0.023743 | 0.003194 | 0.133901 [0.133344, 0.134902] | pass |
| opt_gf8_mul / 1024 | flock_scalar | native_batch | 1.502828 | 0.069184 | 0.046015 [0.044312, 0.046355] | pass |
| opt_gf8_mul / 65536 | flock_scalar | native_batch | 96.395828 | 4.447265 | 0.046122 [0.045909, 0.047217] | pass |
| opt_phi8 / 16 | table_public_input | table_public_input | 0.008868 | 0.008868 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_phi8 / 1024 | table_public_input | table_public_input | 0.376347 | 0.376347 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_phi8 / 65536 | table_public_input | table_public_input | 23.765707 | 23.765707 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_b127_mul / 16 | production | production | 0.019532 | 0.019532 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_b127_mul / 1024 | production | production | 1.241150 | 1.241150 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_b127_mul / 65536 | production | production | 80.039383 | 80.039383 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_ood / log10 | production_blocked | reuse_products | 3.098206 | 2.217814 | 0.717746 [0.710707, 0.720256] | pass |
| opt_ood / log16 | production_blocked | reuse_products | 68.174476 | 60.887375 | 0.892630 [0.881073, 0.918514] | pass |
| opt_ood / log18 | production_blocked | reuse_products | 224.131500 | 200.958344 | 0.895061 [0.886167, 0.904040] | pass |
| opt_ood_reuse / log10 | allocate | scratch | 2.209371 | 1.839020 | 0.830782 [0.820167, 0.834818] | pass |
| opt_ood_reuse / log16 | allocate | allocate | 60.748695 | 60.748695 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_ood_reuse / log18 | allocate | allocate | 200.240875 | 200.240875 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_ntt / log8_lanes32 | production | half_depth | 46.258301 | 30.685547 | 0.663340 [0.659474, 0.667043] | pass |
| opt_ntt / log12_lanes8 | production | half_depth | 405.075562 | 328.036437 | 0.811097 [0.798875, 0.827721] | pass |
| opt_ntt / log15_lanes8 | production | half_depth | 3798.312000 | 3245.250000 | 0.853017 [0.845766, 0.862469] | pass |
| opt_ntt / log15_lanes32 | production | half_depth | 11180.792000 | 9522.104000 | 0.853636 [0.849027, 0.861589] | pass |
| opt_ntt / log16_lanes32 | production | half_depth | 23745.000500 | 20937.979000 | 0.876777 [0.866519, 0.885913] | pass |
| opt_ntt / log18_lanes32 | production | half_depth | 106120.083500 | 94161.937500 | 0.884961 [0.878090, 0.900068] | pass |
| opt_ntt / log8_lanes1 | production | production | 2.661682 | 2.661682 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_pack / logs9_9 | production | tiled_write | 24.981281 | 0.572428 | 0.027259 [0.020236, 0.106766] | pass |
| opt_pack / logs16_14 | production | tiled_write | 127.649078 | 68.979172 | 0.531021 [0.519536, 0.606301] | pass |
| opt_pack / logs18_18 | production | tiled_write | 407.171875 | 279.679750 | 0.684692 [0.674041, 0.714278] | pass |
| opt_packed_ood / logs9_9 | production | tiled_indexed_ood | 34.391113 | 2.913898 | 0.089371 [0.075955, 0.336981] | pass |
| opt_packed_ood / logs16_14 | production | tiled_indexed_ood | 263.328156 | 206.278625 | 0.792636 [0.779202, 0.835349] | pass |
| opt_packed_ood / logs18_18 | production | tiled_indexed_ood | 848.640500 | 738.390625 | 0.871938 [0.845275, 0.880445] | pass |
| opt_gf_two_pair / 16 | production_fused | separate_wide | 0.183956 | 0.152740 | 0.832177 [0.823595, 0.836075] | pass |
| opt_gf_two_pair / 1024 | production_fused | separate_wide | 11.218629 | 9.305867 | 0.830812 [0.826024, 0.865632] | pass |
| opt_gf_fold_round / 16 | production_fused | fold_then_wide | 0.185346 | 0.171468 | 0.923449 [0.920356, 0.931687] | pass |
| opt_gf_fold_round / 1024 | production_fused | fold_then_wide | 12.081461 | 11.341470 | 0.939916 [0.927632, 0.947018] | pass |
| opt_gf_grid / 16 | production_fused | production_fused | 0.827921 | 0.827921 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_gf_grid / 1024 | production_fused | production_fused | 53.807289 | 53.807289 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_gf8_inverse / 16 | flock_scalar | vector_chain | 0.584325 | 0.029511 | 0.050889 [0.050196, 0.051199] | pass |
| opt_gf8_inverse / 1024 | flock_scalar | vector_chain | 37.337726 | 1.630043 | 0.043900 [0.043225, 0.044156] | pass |
| opt_gf8_inverse / 65536 | flock_scalar | vector_chain | 2410.572750 | 106.187500 | 0.044413 [0.043231, 0.044878] | pass |
| opt_f2_poly_dot / a1_b1_dense_n16 | production_zero_skip | production_zero_skip | 0.012180 | 0.012180 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_f2_poly_dot / a1_b1_dense_n1024 | production_zero_skip | row_density | 0.659856 | 0.642555 | 0.974858 [0.968299, 0.981772] | pass |
| opt_f2_poly_dot / a1_b1_sparse_n16 | production_zero_skip | production_zero_skip | 0.012778 | 0.012778 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_f2_poly_dot / a1_b1_sparse_n1024 | production_zero_skip | row_density | 0.740550 | 0.473836 | 0.641295 [0.552797, 0.680590] | pass |
| opt_f2_poly_dot / a3_b7_dense_n16 | production_zero_skip | production_zero_skip | 0.142014 | 0.142014 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_f2_poly_dot / a3_b7_dense_n1024 | production_zero_skip | production_zero_skip | 8.337483 | 8.337483 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_f2_poly_dot / a3_b7_sparse_n16 | production_zero_skip | production_zero_skip | 0.058285 | 0.058285 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_f2_poly_dot / a3_b7_sparse_n1024 | production_zero_skip | production_zero_skip | 3.622009 | 3.622009 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_f2_poly_dot / a9_b9_dense_n16 | production_zero_skip | row_density | 1.970682 | 0.838460 | 0.427269 [0.418820, 0.429592] | pass |
| opt_f2_poly_dot / a9_b9_dense_n1024 | production_zero_skip | row_density | 126.206391 | 53.487625 | 0.424821 [0.420518, 0.429561] | pass |
| opt_f2_poly_dot / a9_b9_sparse_n16 | production_zero_skip | row_density | 0.224161 | 0.217823 | 0.975641 [0.949866, 0.993234] | pass |
| opt_f2_poly_dot / a9_b9_sparse_n1024 | production_zero_skip | row_density | 20.235272 | 16.947266 | 0.871292 [0.774943, 0.958144] | pass |
| opt_f2_poly_dot / a1_b1_zero_n16 | production_zero_skip | production_zero_skip | 0.012870 | 0.012870 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_f2_poly_dot / a1_b1_zero_n1024 | production_zero_skip | row_density | 0.468617 | 0.335563 | 0.714413 [0.707278, 0.725253] | pass |
| opt_f2_poly_dot / a1_b1_dense_prefix_n16 | production_zero_skip | production_zero_skip | 0.011232 | 0.011232 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_f2_poly_dot / a1_b1_dense_prefix_n1024 | production_zero_skip | row_density | 0.778875 | 0.478841 | 0.629200 [0.573713, 0.663384] | pass |
| opt_f2_poly_dot / a1_b1_sparse_prefix_n16 | production_zero_skip | production_zero_skip | 0.011962 | 0.011962 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_f2_poly_dot / a1_b1_sparse_prefix_n1024 | production_zero_skip | row_density | 0.666957 | 0.650897 | 0.979017 [0.966585, 0.987600] | pass |
| opt_f2_poly_dot / a1_b1_alternating_n16 | production_zero_skip | production_zero_skip | 0.012532 | 0.012532 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_f2_poly_dot / a1_b1_alternating_n1024 | production_zero_skip | row_density | 0.668152 | 0.327060 | 0.488242 [0.483901, 0.495135] | pass |
| opt_f2_poly_dot / a3_b7_zero_n16 | production_zero_skip | production_zero_skip | 0.020271 | 0.020271 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_f2_poly_dot / a3_b7_zero_n1024 | production_zero_skip | production_zero_skip | 1.116567 | 1.116567 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_f2_poly_dot / a3_b7_dense_prefix_n16 | production_zero_skip | production_zero_skip | 0.085714 | 0.085714 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_f2_poly_dot / a3_b7_dense_prefix_n1024 | production_zero_skip | production_zero_skip | 3.584615 | 3.584615 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_f2_poly_dot / a3_b7_sparse_prefix_n16 | production_zero_skip | production_zero_skip | 0.106307 | 0.106307 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_f2_poly_dot / a3_b7_sparse_prefix_n1024 | production_zero_skip | production_zero_skip | 8.266195 | 8.266195 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_f2_poly_dot / a3_b7_alternating_n16 | production_zero_skip | production_zero_skip | 0.078976 | 0.078976 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_f2_poly_dot / a3_b7_alternating_n1024 | production_zero_skip | production_zero_skip | 4.671671 | 4.671671 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_f2_poly_dot / a9_b9_zero_n16 | production_zero_skip | row_density | 0.084987 | 0.020533 | 0.241247 [0.238270, 0.242896] | pass |
| opt_f2_poly_dot / a9_b9_zero_n1024 | production_zero_skip | row_density | 4.984924 | 1.060812 | 0.212330 [0.210763, 0.213533] | pass |
| opt_f2_poly_dot / a9_b9_dense_prefix_n16 | production_zero_skip | row_density | 0.639033 | 0.357953 | 0.545897 [0.532238, 0.576606] | pass |
| opt_f2_poly_dot / a9_b9_dense_prefix_n1024 | production_zero_skip | row_density | 21.928062 | 14.989014 | 0.664159 [0.540845, 0.800790] | pass |
| opt_f2_poly_dot / a9_b9_sparse_prefix_n16 | production_zero_skip | row_density | 1.495443 | 0.630865 | 0.422679 [0.418152, 0.425190] | pass |
| opt_f2_poly_dot / a9_b9_sparse_prefix_n1024 | production_zero_skip | row_density | 125.953781 | 53.278641 | 0.423528 [0.420676, 0.432206] | pass |
| opt_f2_poly_dot / a9_b9_alternating_n16 | production_zero_skip | row_density | 0.988297 | 0.418996 | 0.420824 [0.416270, 0.427167] | pass |
| opt_f2_poly_dot / a9_b9_alternating_n1024 | production_zero_skip | row_density | 63.608398 | 27.020836 | 0.425968 [0.422275, 0.429832] | pass |

### Other measured candidates

| Operation / workload | Baseline kernel | Candidate kernel | Baseline median (µs) | Candidate median (µs) | Paired time ratio [95% CI] | Status |
|---|---|---|---:|---:|---|---|
| opt_integer_mac / l1_signed16_n16 | circuit_z | products2 | 0.005664 | 0.007180 | 1.248511 [1.235538, 1.297214] | regression |
| opt_integer_mac / l1_signed16_n16 | circuit_z | products4 | 0.005664 | 0.005786 | 1.009777 [1.001149, 1.104353] | inconclusive |
| opt_integer_mac / l1_signed16_n16 | circuit_z | deferred_columns | 0.005664 | 0.005645 | 0.996886 [0.989203, 1.004347] | pass |
| opt_integer_mac / l1_signed16_n16 | circuit_z | incumbent | 0.005664 | 0.005907 | 1.042837 [1.035442, 1.050660] | regression |
| opt_integer_mac / l1_signed16_n16 | circuit_z | native_word | 0.005664 | 0.005696 | 1.006703 [0.996811, 1.014995] | inconclusive |
| opt_integer_mac / l1_signed16_n1024 | circuit_z | products2 | 0.333630 | 0.342401 | 1.028623 [1.006934, 1.037724] | inconclusive |
| opt_integer_mac / l1_signed16_n1024 | circuit_z | products4 | 0.333630 | 0.333131 | 0.999335 [0.988219, 1.006541] | inconclusive |
| opt_integer_mac / l1_signed16_n1024 | circuit_z | deferred_columns | 0.333630 | 0.333633 | 0.997504 [0.992855, 1.006756] | inconclusive |
| opt_integer_mac / l1_signed16_n1024 | circuit_z | incumbent | 0.333630 | 0.334446 | 1.003417 [0.995302, 1.010069] | inconclusive |
| opt_integer_mac / l1_signed16_n1024 | circuit_z | native_word | 0.333630 | 0.333138 | 0.996472 [0.991116, 1.006399] | inconclusive |
| opt_integer_mac / l1_signed16_n65536 | circuit_z | products2 | 21.015054 | 20.991782 | 0.998595 [0.994274, 1.005217] | inconclusive |
| opt_integer_mac / l1_signed16_n65536 | circuit_z | products4 | 21.015054 | 21.033203 | 1.002012 [0.997961, 1.007731] | inconclusive |
| opt_integer_mac / l1_signed16_n65536 | circuit_z | deferred_columns | 21.015054 | 21.048666 | 1.001483 [0.997102, 1.005236] | inconclusive |
| opt_integer_mac / l1_signed16_n65536 | circuit_z | incumbent | 21.015054 | 20.993570 | 1.003505 [0.993680, 1.006284] | inconclusive |
| opt_integer_mac / l1_signed16_n65536 | circuit_z | native_word | 21.015054 | 21.015463 | 1.000004 [0.996228, 1.004526] | inconclusive |
| opt_integer_mac / l1_full_n16 | circuit_z | products2 | 0.005616 | 0.007003 | 1.248827 [1.243771, 1.253895] | regression |
| opt_integer_mac / l1_full_n16 | circuit_z | products4 | 0.005616 | 0.005640 | 1.008058 [0.999381, 1.014192] | inconclusive |
| opt_integer_mac / l1_full_n16 | circuit_z | deferred_columns | 0.005616 | 0.005700 | 1.026181 [0.996717, 1.033599] | inconclusive |
| opt_integer_mac / l1_full_n16 | circuit_z | incumbent | 0.005616 | 0.005874 | 1.047072 [1.041265, 1.052502] | regression |
| opt_integer_mac / l1_full_n16 | circuit_z | native_word | 0.005616 | 0.005785 | 1.056863 [1.001325, 1.071410] | inconclusive |
| opt_integer_mac / l1_full_n1024 | circuit_z | products2 | 0.332678 | 0.337171 | 1.013671 [1.006191, 1.019554] | inconclusive |
| opt_integer_mac / l1_full_n1024 | circuit_z | products4 | 0.332678 | 0.333134 | 1.002460 [0.992579, 1.006473] | inconclusive |
| opt_integer_mac / l1_full_n1024 | circuit_z | deferred_columns | 0.332678 | 0.333154 | 1.002828 [0.996822, 1.007488] | inconclusive |
| opt_integer_mac / l1_full_n1024 | circuit_z | incumbent | 0.332678 | 0.333712 | 1.002469 [0.991616, 1.007188] | inconclusive |
| opt_integer_mac / l1_full_n1024 | circuit_z | native_word | 0.332678 | 0.333050 | 0.999298 [0.993595, 1.007717] | inconclusive |
| opt_integer_mac / l1_full_n65536 | circuit_z | products2 | 21.168782 | 21.162274 | 0.997830 [0.993418, 1.002986] | pass |
| opt_integer_mac / l1_full_n65536 | circuit_z | products4 | 21.168782 | 21.146729 | 0.996735 [0.992726, 1.006016] | inconclusive |
| opt_integer_mac / l1_full_n65536 | circuit_z | deferred_columns | 21.168782 | 21.257569 | 0.997498 [0.994202, 1.002553] | pass |
| opt_integer_mac / l1_full_n65536 | circuit_z | incumbent | 21.168782 | 21.182377 | 0.997871 [0.992315, 1.003935] | inconclusive |
| opt_integer_mac / l1_full_n65536 | circuit_z | native_word | 21.168782 | 21.148195 | 0.995133 [0.985951, 1.004518] | inconclusive |
| opt_integer_mac / l2_signed16_n16 | circuit_z | products2 | 0.012981 | 0.011553 | 0.890995 [0.882545, 0.895072] | pass |
| opt_integer_mac / l2_signed16_n16 | circuit_z | products4 | 0.012981 | 0.012130 | 0.933624 [0.929058, 0.941773] | pass |
| opt_integer_mac / l2_signed16_n16 | circuit_z | deferred_columns | 0.012981 | 0.020946 | 1.611166 [1.576842, 1.630109] | regression |
| opt_integer_mac / l2_signed16_n1024 | circuit_z | products2 | 0.699056 | 0.666006 | 0.950681 [0.941709, 0.958408] | pass |
| opt_integer_mac / l2_signed16_n1024 | circuit_z | products4 | 0.699056 | 0.665728 | 0.952911 [0.945837, 0.958030] | pass |
| opt_integer_mac / l2_signed16_n1024 | circuit_z | deferred_columns | 0.699056 | 1.175578 | 1.685050 [1.672268, 1.694329] | regression |
| opt_integer_mac / l2_signed16_n65536 | circuit_z | products2 | 44.293781 | 42.268067 | 0.952372 [0.947522, 0.959108] | pass |
| opt_integer_mac / l2_signed16_n65536 | circuit_z | products4 | 44.293781 | 42.253418 | 0.954998 [0.946449, 0.960692] | pass |
| opt_integer_mac / l2_signed16_n65536 | circuit_z | deferred_columns | 44.293781 | 73.980633 | 1.677289 [1.659702, 1.686953] | regression |
| opt_integer_mac / l2_full_n16 | circuit_z | products2 | 0.012884 | 0.011517 | 0.892818 [0.882910, 0.901958] | pass |
| opt_integer_mac / l2_full_n16 | circuit_z | products4 | 0.012884 | 0.012055 | 0.933344 [0.925637, 0.944530] | pass |
| opt_integer_mac / l2_full_n16 | circuit_z | deferred_columns | 0.012884 | 0.020975 | 1.631052 [1.618119, 1.641498] | regression |
| opt_integer_mac / l2_full_n1024 | circuit_z | products2 | 0.698082 | 0.670547 | 0.957519 [0.951726, 0.968979] | pass |
| opt_integer_mac / l2_full_n1024 | circuit_z | products4 | 0.698082 | 0.667353 | 0.956607 [0.949827, 0.962375] | pass |
| opt_integer_mac / l2_full_n1024 | circuit_z | deferred_columns | 0.698082 | 1.179960 | 1.693508 [1.681867, 1.706558] | regression |
| opt_integer_mac / l2_full_n65536 | circuit_z | products2 | 44.114098 | 42.166016 | 0.955082 [0.949508, 0.960274] | pass |
| opt_integer_mac / l2_full_n65536 | circuit_z | products4 | 44.114098 | 42.094235 | 0.954922 [0.948651, 0.958086] | pass |
| opt_integer_mac / l2_full_n65536 | circuit_z | deferred_columns | 44.114098 | 74.031574 | 1.675728 [1.669250, 1.684529] | regression |
| opt_integer_mac / l9_signed16_n16 | circuit_z | products2 | 0.348171 | 0.355519 | 1.021569 [1.012567, 1.033804] | regression |
| opt_integer_mac / l9_signed16_n16 | circuit_z | products4 | 0.348171 | 0.366923 | 1.056312 [1.049318, 1.064901] | regression |
| opt_integer_mac / l9_signed16_n16 | circuit_z | deferred_columns | 0.348171 | 0.855255 | 2.459511 [2.441417, 2.473073] | regression |
| opt_integer_mac / l9_signed16_n1024 | circuit_z | products2 | 22.174234 | 22.395264 | 1.008570 [1.000371, 1.018414] | inconclusive |
| opt_integer_mac / l9_signed16_n1024 | circuit_z | products4 | 22.174234 | 22.343182 | 1.008499 [0.996345, 1.014824] | inconclusive |
| opt_integer_mac / l9_signed16_n1024 | circuit_z | deferred_columns | 22.174234 | 54.354084 | 2.446479 [2.429011, 2.471745] | regression |
| opt_integer_mac / l9_signed16_n65536 | circuit_z | products2 | 1426.067625 | 1439.536375 | 1.011497 [1.001714, 1.016920] | inconclusive |
| opt_integer_mac / l9_signed16_n65536 | circuit_z | products4 | 1426.067625 | 1443.447875 | 1.011749 [1.005163, 1.018378] | inconclusive |
| opt_integer_mac / l9_signed16_n65536 | circuit_z | deferred_columns | 1426.067625 | 3512.770750 | 2.459880 [2.448959, 2.477804] | regression |
| opt_integer_mac / l9_full_n16 | circuit_z | products2 | 0.347919 | 0.357083 | 1.024873 [1.015741, 1.031162] | regression |
| opt_integer_mac / l9_full_n16 | circuit_z | products4 | 0.347919 | 0.367435 | 1.052161 [1.047661, 1.060357] | regression |
| opt_integer_mac / l9_full_n16 | circuit_z | deferred_columns | 0.347919 | 0.860985 | 2.466899 [2.448012, 2.486461] | regression |
| opt_integer_mac / l9_full_n1024 | circuit_z | products2 | 22.211752 | 22.539875 | 1.011552 [1.004960, 1.019076] | inconclusive |
| opt_integer_mac / l9_full_n1024 | circuit_z | products4 | 22.211752 | 22.483643 | 1.009448 [1.003074, 1.022598] | inconclusive |
| opt_integer_mac / l9_full_n1024 | circuit_z | deferred_columns | 22.211752 | 54.662029 | 2.455568 [2.444680, 2.470898] | regression |
| opt_integer_mac / l9_full_n65536 | circuit_z | products2 | 1421.390500 | 1432.156125 | 1.006552 [1.001601, 1.011601] | inconclusive |
| opt_integer_mac / l9_full_n65536 | circuit_z | products4 | 1421.390500 | 1432.927125 | 1.009499 [1.003206, 1.014590] | inconclusive |
| opt_integer_mac / l9_full_n65536 | circuit_z | deferred_columns | 1421.390500 | 3498.942750 | 2.462223 [2.453369, 2.471472] | regression |
| opt_integer_mac / l4_signed16_n16 | circuit_z | products2 | 0.053720 | 0.058855 | 1.089834 [1.083673, 1.101920] | regression |
| opt_integer_mac / l4_signed16_n16 | circuit_z | products4 | 0.053720 | 0.060175 | 1.117661 [1.110350, 1.125922] | regression |
| opt_integer_mac / l4_signed16_n16 | circuit_z | deferred_columns | 0.053720 | 0.066902 | 1.243512 [1.232419, 1.251196] | regression |
| opt_integer_mac / l4_signed16_n16 | circuit_z | incumbent | 0.053720 | 0.058211 | 1.085037 [1.074521, 1.091853] | regression |
| opt_integer_mac / l4_signed16_n16 | circuit_z | native128x2_acc2 | 0.053720 | 0.048959 | 0.913076 [0.903316, 0.917160] | pass |
| opt_integer_mac / l4_signed16_n1024 | circuit_z | products2 | 3.201335 | 3.423401 | 1.073387 [1.063865, 1.082273] | regression |
| opt_integer_mac / l4_signed16_n1024 | circuit_z | products4 | 3.201335 | 3.564717 | 1.118863 [1.107345, 1.125877] | regression |
| opt_integer_mac / l4_signed16_n1024 | circuit_z | deferred_columns | 3.201335 | 4.119548 | 1.298139 [1.278386, 1.311582] | regression |
| opt_integer_mac / l4_signed16_n1024 | circuit_z | incumbent | 3.201335 | 4.257935 | 1.341238 [1.324085, 1.346813] | regression |
| opt_integer_mac / l4_signed16_n1024 | circuit_z | native128x2_acc2 | 3.201335 | 2.981161 | 0.931898 [0.925785, 0.939774] | pass |
| opt_integer_mac / l4_signed16_n65536 | circuit_z | products2 | 208.212250 | 223.458344 | 1.074895 [1.066812, 1.083926] | regression |
| opt_integer_mac / l4_signed16_n65536 | circuit_z | products4 | 208.212250 | 227.996125 | 1.092796 [1.083511, 1.103306] | regression |
| opt_integer_mac / l4_signed16_n65536 | circuit_z | deferred_columns | 208.212250 | 269.311218 | 1.290339 [1.280470, 1.307422] | regression |
| opt_integer_mac / l4_signed16_n65536 | circuit_z | incumbent | 208.212250 | 273.058594 | 1.311251 [1.301559, 1.319702] | regression |
| opt_integer_mac / l4_signed16_n65536 | circuit_z | native128x2_acc2 | 208.212250 | 190.988313 | 0.913426 [0.907937, 0.925359] | pass |
| opt_integer_mac / l4_full_n16 | circuit_z | products2 | 0.053629 | 0.058010 | 1.071555 [1.065237, 1.099658] | regression |
| opt_integer_mac / l4_full_n16 | circuit_z | products4 | 0.053629 | 0.059988 | 1.113316 [1.110134, 1.124960] | regression |
| opt_integer_mac / l4_full_n16 | circuit_z | deferred_columns | 0.053629 | 0.067808 | 1.272707 [1.247241, 1.276282] | regression |
| opt_integer_mac / l4_full_n16 | circuit_z | incumbent | 0.053629 | 0.058230 | 1.086411 [1.075583, 1.090942] | regression |
| opt_integer_mac / l4_full_n16 | circuit_z | native128x2_acc2 | 0.053629 | 0.049334 | 0.917903 [0.911347, 0.924845] | pass |
| opt_integer_mac / l4_full_n1024 | circuit_z | products2 | 3.207906 | 3.431091 | 1.076523 [1.058346, 1.081792] | regression |
| opt_integer_mac / l4_full_n1024 | circuit_z | products4 | 3.207906 | 3.568075 | 1.113615 [1.102060, 1.127950] | regression |
| opt_integer_mac / l4_full_n1024 | circuit_z | deferred_columns | 3.207906 | 4.126526 | 1.292362 [1.277636, 1.301210] | regression |
| opt_integer_mac / l4_full_n1024 | circuit_z | incumbent | 3.207906 | 4.259196 | 1.328729 [1.313699, 1.340652] | regression |
| opt_integer_mac / l4_full_n1024 | circuit_z | native128x2_acc2 | 3.207906 | 2.991048 | 0.938284 [0.925917, 0.943996] | pass |
| opt_integer_mac / l4_full_n65536 | circuit_z | products2 | 210.161438 | 227.416656 | 1.076224 [1.068675, 1.093486] | regression |
| opt_integer_mac / l4_full_n65536 | circuit_z | products4 | 210.161438 | 230.423156 | 1.098305 [1.085887, 1.106446] | regression |
| opt_integer_mac / l4_full_n65536 | circuit_z | deferred_columns | 210.161438 | 271.731750 | 1.294759 [1.281111, 1.310793] | regression |
| opt_integer_mac / l4_full_n65536 | circuit_z | incumbent | 210.161438 | 275.328125 | 1.312460 [1.295053, 1.328032] | regression |
| opt_integer_mac / l4_full_n65536 | circuit_z | native128x2_acc2 | 210.161438 | 192.324219 | 0.914416 [0.905786, 0.928293] | pass |
| opt_projection / q100_l2_n16 | production | public_divisor | 2.948507 | 0.972311 | 0.330311 [0.308048, 0.339192] | pass |
| opt_projection / q100_l2_n1024 | production | public_divisor | 581.716188 | 66.299437 | 0.114496 [0.111921, 0.115756] | pass |
| opt_projection / q128_l2_n16 | production | public_divisor | 0.317666 | 0.772067 | 2.422062 [2.365010, 2.977193] | regression |
| opt_projection / q128_l2_n16 | production | horner_one_shot | 0.317666 | 0.274984 | 0.867816 [0.837906, 1.062007] | inconclusive |
| opt_projection / q128_l2_n1024 | production | public_divisor | 23.171063 | 48.648276 | 2.102187 [2.072976, 2.122671] | regression |
| opt_projection / q100_l4_n16 | production | public_divisor | 5.785318 | 1.374104 | 0.240613 [0.209437, 0.244749] | pass |
| opt_projection / q100_l4_n1024 | production | public_divisor | 1385.859375 | 84.567625 | 0.061251 [0.060635, 0.061679] | pass |
| opt_projection / q128_l4_n16 | production | public_divisor | 1.044479 | 1.256225 | 1.234449 [1.080057, 1.263209] | regression |
| opt_projection / q128_l4_n1024 | production | public_divisor | 84.157547 | 79.986000 | 0.957278 [0.940830, 0.964600] | pass |
| opt_projection / q100_l9_n16 | production | public_divisor | 28.552410 | 2.879234 | 0.100469 [0.097207, 0.106578] | pass |
| opt_projection / q100_l9_n1024 | production | public_divisor | 3395.937500 | 180.229500 | 0.052974 [0.052289, 0.053329] | pass |
| opt_projection / q128_l9_n16 | production | public_divisor | 2.925476 | 2.763489 | 0.945637 [0.892259, 1.015028] | inconclusive |
| opt_projection / q128_l9_n1024 | production | public_divisor | 235.514313 | 173.147125 | 0.732019 [0.722964, 0.747476] | pass |
| opt_divrem64 / l2_n16 | public_divisor | generic | 0.376654 | 0.821129 | 2.177499 [2.171890, 2.213380] | regression |
| opt_divrem64 / l2_n16 | public_divisor | prepare_batch | 0.376654 | 0.125504 | 0.333860 [0.331868, 0.336483] | pass |
| opt_divrem64 / l2_n1024 | public_divisor | generic | 23.899902 | 52.589520 | 2.194956 [2.187802, 2.215043] | regression |
| opt_divrem64 / l2_n1024 | public_divisor | prepare_batch | 23.899902 | 6.593914 | 0.274213 [0.272701, 0.277827] | pass |
| opt_divrem64 / l4_n16 | public_divisor | generic | 0.633074 | 1.521876 | 2.399894 [2.385545, 2.414283] | regression |
| opt_divrem64 / l4_n16 | public_divisor | prepare_batch | 0.633074 | 0.272781 | 0.431614 [0.423161, 0.435494] | pass |
| opt_divrem64 / l4_n1024 | public_divisor | generic | 40.516277 | 97.213375 | 2.402234 [2.393081, 2.417084] | regression |
| opt_divrem64 / l4_n1024 | public_divisor | prepare_batch | 40.516277 | 16.207680 | 0.399346 [0.397267, 0.403305] | pass |
| opt_divrem64 / l9_n16 | public_divisor | generic | 1.285736 | 4.336075 | 3.367086 [3.349526, 3.384048] | regression |
| opt_divrem64 / l9_n16 | public_divisor | prepare_batch | 1.285736 | 0.735764 | 0.570784 [0.566473, 0.575062] | pass |
| opt_divrem64 / l9_n1024 | public_divisor | generic | 82.084641 | 277.191727 | 3.370155 [3.352592, 3.390450] | regression |
| opt_divrem64 / l9_n1024 | public_divisor | prepare_batch | 82.084641 | 45.282227 | 0.546666 [0.544503, 0.554581] | pass |
| opt_divrem64 / l32_n16 | public_divisor | generic | 4.907999 | 27.136821 | 5.529718 [5.516480, 5.553587] | regression |
| opt_divrem64 / l32_n16 | public_divisor | prepare_batch | 4.907999 | 4.050293 | 0.824612 [0.817446, 0.829770] | pass |
| opt_divrem64 / l32_n1024 | public_divisor | generic | 312.981781 | 1734.921875 | 5.568364 [5.531391, 5.601492] | regression |
| opt_divrem64 / l32_n1024 | public_divisor | prepare_batch | 312.981781 | 257.746094 | 0.822867 [0.816532, 0.832706] | pass |
| opt_divrem64 / l64_n16 | public_divisor | generic | 9.456584 | 87.394938 | 9.232930 [9.201040, 9.301987] | regression |
| opt_divrem64 / l64_n16 | public_divisor | prepare_batch | 9.456584 | 8.087727 | 0.855530 [0.845194, 0.861327] | pass |
| opt_divrem64 / l64_n1024 | public_divisor | generic | 612.028625 | 5612.158875 | 9.145861 [9.098811, 9.220904] | regression |
| opt_divrem64 / l64_n1024 | public_divisor | prepare_batch | 612.028625 | 520.783812 | 0.846673 [0.840755, 0.855767] | pass |
| opt_uint_add / l1_n16 | circuit_z | word_carry | 0.004921 | 0.005127 | 1.043731 [1.034535, 1.047736] | regression |
| opt_uint_add / l1_n16 | circuit_z | native_batch | 0.004921 | 0.005125 | 1.042907 [1.031741, 1.046527] | regression |
| opt_uint_add / l1_n1024 | circuit_z | word_carry | 0.211863 | 0.212385 | 1.000886 [0.985015, 1.004202] | inconclusive |
| opt_uint_add / l1_n1024 | circuit_z | native_batch | 0.211863 | 0.211140 | 0.996664 [0.978554, 1.001553] | inconclusive |
| opt_uint_add / l2_n16 | circuit_z | word_carry | 0.011608 | 0.011929 | 1.023238 [1.010019, 1.031646] | regression |
| opt_uint_add / l2_n1024 | circuit_z | word_carry | 0.664986 | 0.663722 | 0.999521 [0.995448, 1.006956] | inconclusive |
| opt_uint_add / l4_n16 | circuit_z | word_carry | 0.016018 | 0.012323 | 0.767697 [0.763805, 0.774679] | pass |
| opt_uint_add / l4_n1024 | circuit_z | word_carry | 0.954804 | 0.708013 | 0.744629 [0.736624, 0.779921] | pass |
| opt_uint_add / l9_n16 | circuit_z | word_carry | 0.060295 | 0.050890 | 0.837036 [0.818717, 0.902186] | pass |
| opt_uint_add / l9_n1024 | circuit_z | word_carry | 3.593485 | 3.229248 | 0.894026 [0.889553, 0.907071] | pass |
| opt_uint_sub / l1_n16 | circuit_z | word_carry | 0.004969 | 0.005168 | 1.039073 [1.032091, 1.051573] | regression |
| opt_uint_sub / l1_n16 | circuit_z | native_batch | 0.004969 | 0.005163 | 1.038141 [1.031632, 1.049578] | regression |
| opt_uint_sub / l1_n1024 | circuit_z | word_carry | 0.211916 | 0.212315 | 1.000717 [0.993483, 1.009449] | inconclusive |
| opt_uint_sub / l1_n1024 | circuit_z | native_batch | 0.211916 | 0.211550 | 0.997691 [0.991313, 1.005262] | inconclusive |
| opt_uint_sub / l2_n16 | circuit_z | word_carry | 0.011572 | 0.011892 | 1.032067 [1.023388, 1.038297] | regression |
| opt_uint_sub / l2_n1024 | circuit_z | word_carry | 0.661361 | 0.663170 | 1.003189 [0.997086, 1.007329] | inconclusive |
| opt_uint_sub / l4_n16 | circuit_z | word_carry | 0.022019 | 0.021995 | 0.996135 [0.992169, 1.002377] | pass |
| opt_uint_sub / l4_n16 | circuit_z | native_batch | 0.022019 | 0.021929 | 0.994978 [0.990080, 1.001341] | pass |
| opt_uint_sub / l4_n1024 | circuit_z | word_carry | 1.341583 | 1.340307 | 0.994946 [0.985149, 1.012532] | inconclusive |
| opt_uint_sub / l4_n1024 | circuit_z | native_batch | 1.341583 | 1.349635 | 1.005400 [0.998064, 1.009645] | inconclusive |
| opt_uint_sub / l9_n16 | circuit_z | word_carry | 0.062153 | 0.063689 | 1.018854 [0.999762, 1.036603] | inconclusive |
| opt_uint_sub / l9_n16 | circuit_z | native_batch | 0.062153 | 0.061083 | 0.974174 [0.958493, 0.997174] | inconclusive |
| opt_uint_sub / l9_n1024 | circuit_z | word_carry | 3.978048 | 3.908488 | 0.986823 [0.976414, 0.989616] | pass |
| opt_uint_sub / l9_n1024 | circuit_z | native_batch | 3.978048 | 3.902486 | 0.978871 [0.972928, 0.989207] | inconclusive |
| opt_uint_checked_add / l1_n16 | vendor_option | word_option | 0.019398 | 0.008993 | 0.465260 [0.456652, 0.467880] | pass |
| opt_uint_checked_add / l1_n16 | vendor_option | native_option | 0.019398 | 0.008716 | 0.449957 [0.446313, 0.452416] | pass |
| opt_uint_checked_add / l1_n1024 | vendor_option | word_option | 0.489138 | 0.489621 | 1.003002 [0.994651, 1.009851] | inconclusive |
| opt_uint_checked_add / l1_n1024 | vendor_option | native_option | 0.489138 | 0.474805 | 0.965351 [0.960301, 0.979735] | pass |
| opt_uint_checked_add / l1_n1024 | vendor_option | limb_option | 0.489138 | 0.479220 | 0.981688 [0.972681, 0.986148] | pass |
| opt_uint_checked_add / l2_n16 | vendor_option | word_option | 0.014073 | 0.022263 | 1.591291 [1.490171, 1.605322] | regression |
| opt_uint_checked_add / l2_n16 | vendor_option | native_option | 0.014073 | 0.016078 | 1.152014 [1.072078, 1.162701] | regression |
| opt_uint_checked_add / l2_n1024 | vendor_option | word_option | 0.726862 | 1.237963 | 1.701688 [1.677722, 1.717579] | regression |
| opt_uint_checked_add / l2_n1024 | vendor_option | native_option | 0.726862 | 0.912740 | 1.254180 [1.250176, 1.261414] | regression |
| opt_uint_checked_add / l4_n16 | vendor_option | word_option | 0.044944 | 0.029524 | 0.637930 [0.626268, 0.673707] | pass |
| opt_uint_checked_add / l4_n16 | vendor_option | native_option | 0.044944 | 0.024356 | 0.545385 [0.513410, 0.560891] | pass |
| opt_uint_checked_add / l4_n1024 | vendor_option | word_option | 3.616679 | 1.505920 | 0.415415 [0.411308, 0.432580] | pass |
| opt_uint_checked_add / l4_n1024 | vendor_option | native_option | 3.616679 | 1.468912 | 0.406356 [0.401222, 0.421580] | pass |
| opt_uint_checked_add / l9_n16 | vendor_option | word_option | 0.083754 | 0.060892 | 0.724793 [0.697543, 0.735503] | pass |
| opt_uint_checked_add / l9_n16 | vendor_option | native_option | 0.083754 | 0.059141 | 0.702244 [0.686316, 0.712679] | pass |
| opt_uint_checked_add / l9_n1024 | vendor_option | word_option | 7.181192 | 4.066772 | 0.564818 [0.552227, 0.579914] | pass |
| opt_uint_checked_add / l9_n1024 | vendor_option | native_option | 7.181192 | 4.054199 | 0.562991 [0.551138, 0.578264] | pass |
| opt_uint_checked_sub / l1_n16 | vendor_option | word_option | 0.018950 | 0.009288 | 0.487853 [0.484427, 0.493866] | pass |
| opt_uint_checked_sub / l1_n16 | vendor_option | limb_option | 0.018950 | 0.008745 | 0.459604 [0.457948, 0.463208] | pass |
| opt_uint_checked_sub / l1_n1024 | vendor_option | word_option | 0.464996 | 0.497113 | 1.065423 [1.059524, 1.074695] | regression |
| opt_uint_checked_sub / l1_n1024 | vendor_option | native_option | 0.464996 | 0.449725 | 0.966517 [0.959835, 0.973199] | pass |
| opt_uint_checked_sub / l1_n1024 | vendor_option | limb_option | 0.464996 | 0.449631 | 0.966369 [0.954216, 0.973152] | pass |
| opt_uint_checked_sub / l2_n16 | vendor_option | word_option | 0.021338 | 0.021991 | 1.028866 [0.983552, 1.036571] | inconclusive |
| opt_uint_checked_sub / l2_n16 | vendor_option | limb_option | 0.021338 | 0.020139 | 0.946206 [0.891909, 0.950317] | pass |
| opt_uint_checked_sub / l2_n1024 | vendor_option | word_option | 1.192993 | 1.214126 | 1.017377 [1.012217, 1.023315] | regression |
| opt_uint_checked_sub / l2_n1024 | vendor_option | limb_option | 1.192993 | 1.158239 | 0.968636 [0.961805, 0.974685] | pass |
| opt_uint_checked_sub / l4_n16 | vendor_option | word_option | 0.043251 | 0.028973 | 0.663181 [0.648150, 0.675794] | pass |
| opt_uint_checked_sub / l4_n16 | vendor_option | limb_option | 0.043251 | 0.028081 | 0.646008 [0.635263, 0.655750] | pass |
| opt_uint_checked_sub / l4_n1024 | vendor_option | word_option | 3.382873 | 1.808431 | 0.530957 [0.518345, 0.545584] | pass |
| opt_uint_checked_sub / l4_n1024 | vendor_option | limb_option | 3.382873 | 1.698385 | 0.505965 [0.493442, 0.507660] | pass |
| opt_uint_checked_sub / l9_n16 | vendor_option | word_option | 0.092745 | 0.066894 | 0.714554 [0.708943, 0.733312] | pass |
| opt_uint_checked_sub / l9_n16 | vendor_option | limb_option | 0.092745 | 0.074712 | 0.798783 [0.792417, 0.817856] | pass |
| opt_uint_checked_sub / l9_n1024 | vendor_option | word_option | 7.376139 | 4.148967 | 0.561136 [0.556186, 0.568580] | pass |
| opt_uint_checked_sub / l9_n1024 | vendor_option | limb_option | 7.376139 | 4.708618 | 0.636108 [0.632126, 0.640992] | pass |
| opt_int_checked_add / l1_n16 | vendor_option | native_option | 0.021777 | 0.011777 | 0.544815 [0.532383, 0.566984] | pass |
| opt_int_checked_add / l1_n1024 | vendor_option | native_option | 1.223745 | 0.699931 | 0.574097 [0.555608, 0.580514] | pass |
| opt_int_checked_add / l2_n16 | vendor_option | native_option | 0.020930 | 0.013154 | 0.629345 [0.621609, 0.635725] | pass |
| opt_int_checked_add / l2_n1024 | vendor_option | native_option | 1.628591 | 0.763408 | 0.466893 [0.459584, 0.484855] | pass |
| opt_int_checked_add / l4_n16 | vendor_option | native_option | 0.047930 | 0.020107 | 0.428667 [0.392103, 0.444661] | pass |
| opt_int_checked_add / l4_n1024 | vendor_option | native_option | 3.588297 | 1.186401 | 0.332724 [0.326965, 0.337979] | pass |
| opt_int_checked_add / l9_n16 | vendor_option | native_option | 0.094306 | 0.053412 | 0.565800 [0.558563, 0.574718] | pass |
| opt_int_checked_add / l9_n1024 | vendor_option | native_option | 7.410889 | 3.376709 | 0.453650 [0.448797, 0.461138] | pass |
| opt_int_checked_sub / l1_n16 | vendor_option | native_option | 0.017546 | 0.011688 | 0.666520 [0.656778, 0.673651] | pass |
| opt_int_checked_sub / l1_n1024 | vendor_option | native_option | 1.077952 | 0.706177 | 0.671176 [0.621651, 0.684089] | pass |
| opt_int_checked_sub / l2_n16 | vendor_option | native_option | 0.020790 | 0.013040 | 0.629152 [0.621622, 0.635269] | pass |
| opt_int_checked_sub / l2_n1024 | vendor_option | native_option | 1.414515 | 0.749532 | 0.514111 [0.501119, 0.564109] | pass |
| opt_int_checked_sub / l4_n16 | vendor_option | native_option | 0.051541 | 0.023245 | 0.452899 [0.445501, 0.455718] | pass |
| opt_int_checked_sub / l4_n1024 | vendor_option | native_option | 4.017924 | 1.382162 | 0.341930 [0.333869, 0.353650] | pass |
| opt_int_checked_sub / l9_n16 | vendor_option | native_option | 0.113877 | 0.064559 | 0.566654 [0.542486, 0.577434] | pass |
| opt_int_checked_sub / l9_n1024 | vendor_option | native_option | 8.861328 | 3.991495 | 0.448606 [0.446148, 0.461347] | pass |
| opt_exact_product / a1_b1_n16 | crypto_bigint | schoolbook | 0.006308 | 0.007403 | 1.175039 [1.162072, 1.189346] | regression |
| opt_exact_product / a1_b1_n1024 | crypto_bigint | schoolbook | 0.334585 | 0.384390 | 1.151774 [1.144143, 1.155658] | regression |
| opt_exact_product / a2_b2_n16 | crypto_bigint | schoolbook | 0.023075 | 0.023352 | 1.009986 [1.003495, 1.017870] | inconclusive |
| opt_exact_product / a2_b2_n1024 | crypto_bigint | schoolbook | 1.342687 | 1.345332 | 1.006804 [0.996399, 1.010600] | inconclusive |
| opt_exact_product / a4_b4_n16 | crypto_bigint | schoolbook | 0.107291 | 0.109066 | 1.019275 [1.009916, 1.027369] | inconclusive |
| opt_exact_product / a4_b4_n1024 | crypto_bigint | schoolbook | 6.428101 | 6.590251 | 1.025234 [1.015498, 1.031639] | regression |
| opt_exact_product / a9_b9_n16 | crypto_bigint | schoolbook | 0.795125 | 0.977218 | 1.243282 [1.201845, 1.264628] | regression |
| opt_exact_product / a9_b9_n1024 | crypto_bigint | schoolbook | 50.940430 | 63.678383 | 1.224127 [1.197453, 1.282187] | regression |
| opt_exact_product / a2_b9_n16 | crypto_bigint | schoolbook | 0.116384 | 0.116415 | 1.004793 [0.995602, 1.008132] | pass |
| opt_exact_product / a2_b9_n1024 | crypto_bigint | schoolbook | 7.591552 | 7.105591 | 0.929088 [0.924545, 0.938968] | pass |
| opt_exact_product / a32_b32_n16 | crypto_bigint | schoolbook | 10.564494 | 13.624064 | 1.287638 [1.279112, 1.293237] | regression |
| opt_exact_product / a32_b32_n1024 | crypto_bigint | schoolbook | 677.486937 | 875.020813 | 1.296695 [1.287810, 1.302836] | regression |
| opt_exact_product / a64_b64_n16 | crypto_bigint | schoolbook | 37.287274 | 61.928387 | 1.671530 [1.648078, 1.685086] | regression |
| opt_exact_product / a64_b64_n1024 | crypto_bigint | schoolbook | 2394.156250 | 3976.708250 | 1.658626 [1.646945, 1.680966] | regression |
| opt_exact_signed_mac / l9_n16 | wide_then_add | fused_exact | 1.087987 | 1.465545 | 1.351595 [1.342792, 1.357314] | regression |
| opt_exact_signed_mac / l9_n1024 | wide_then_add | fused_exact | 69.583343 | 92.610351 | 1.337235 [1.322555, 1.343029] | regression |
| opt_exact_unsigned_mac / l2_n16 | wide_then_add | fused_exact | 0.040706 | 0.036657 | 0.901397 [0.895753, 0.916878] | pass |
| opt_exact_unsigned_mac / l2_n1024 | wide_then_add | fused_exact | 2.474752 | 2.172699 | 0.873813 [0.869101, 0.878452] | pass |
| opt_exact_unsigned_mac / l4_n16 | wide_then_add | fused_exact | 0.149676 | 0.153582 | 1.030449 [1.018879, 1.039614] | regression |
| opt_exact_unsigned_mac / l4_n1024 | wide_then_add | fused_exact | 9.287313 | 9.569173 | 1.032921 [1.026442, 1.040097] | regression |
| opt_exact_unsigned_mac / l9_n16 | wide_then_add | fused_exact | 0.896433 | 1.291184 | 1.445792 [1.433870, 1.453305] | regression |
| opt_exact_unsigned_mac / l9_n1024 | wide_then_add | fused_exact | 57.395844 | 80.696938 | 1.403541 [1.397448, 1.414268] | regression |
| opt_prime_dot / q100_n16 | one_acc | acc4 | 0.059916 | 0.062751 | 1.045779 [1.039876, 1.058381] | regression |
| opt_prime_dot / q100_n16 | one_acc | length_dispatch | 0.059916 | 0.060025 | 1.001877 [0.988227, 1.008530] | inconclusive |
| opt_prime_dot / q100_n1024 | one_acc | acc4 | 2.439931 | 2.405121 | 0.982926 [0.973633, 0.989968] | pass |
| opt_prime_dot / q100_n1024 | one_acc | length_dispatch | 2.439931 | 2.405671 | 0.982354 [0.974371, 0.993687] | pass |
| opt_prime_dot / q100_n65536 | one_acc | acc4 | 155.315750 | 153.112610 | 0.988959 [0.978859, 0.996329] | pass |
| opt_prime_dot / q100_n65536 | one_acc | length_dispatch | 155.315750 | 153.080735 | 0.986595 [0.981752, 0.997763] | pass |
| opt_prime_dot / q128_n16 | one_acc | acc4 | 0.059498 | 0.062454 | 1.052353 [1.038812, 1.061383] | regression |
| opt_prime_dot / q128_n16 | one_acc | length_dispatch | 0.059498 | 0.059578 | 1.001677 [0.995962, 1.006836] | inconclusive |
| opt_prime_dot / q128_n1024 | one_acc | acc4 | 2.460815 | 2.422069 | 0.983782 [0.976771, 0.990798] | pass |
| opt_prime_dot / q128_n1024 | one_acc | length_dispatch | 2.460815 | 2.420339 | 0.982910 [0.976624, 0.986611] | pass |
| opt_prime_dot / q128_n65536 | one_acc | acc4 | 155.715500 | 153.848312 | 0.988680 [0.977291, 1.003459] | pass |
| opt_prime_dot / q128_n65536 | one_acc | length_dispatch | 155.715500 | 153.373703 | 0.988962 [0.976203, 0.998893] | pass |
| opt_prime_linear / q100_n16 | one_acc | acc4 | 0.040971 | 0.041100 | 1.002246 [0.992192, 1.009807] | inconclusive |
| opt_prime_linear / q128_n16 | one_acc | acc4 | 0.040746 | 0.040835 | 0.998248 [0.994022, 1.005434] | pass |
| opt_batch_inverse / q100_nonzero_n16 | production_one_inverse | compact_vartime | 1.169597 | 1.155818 | 0.985015 [0.968890, 0.993770] | pass |
| opt_batch_inverse / q100_nonzero_n16 | production_one_inverse | compact_ct | 1.169597 | 1.501684 | 1.278725 [1.273392, 1.292802] | regression |
| opt_batch_inverse / q100_nonzero_n1024 | production_one_inverse | compact_vartime | 27.006024 | 26.150554 | 0.969979 [0.961768, 0.978731] | pass |
| opt_batch_inverse / q100_nonzero_n1024 | production_one_inverse | compact_ct | 27.006024 | 26.370605 | 0.978700 [0.966109, 0.990898] | pass |
| opt_batch_inverse / q100_mixed_n16 | production_one_inverse | compact_vartime | 1.037979 | 1.160375 | 1.118416 [1.103543, 1.137564] | regression |
| opt_batch_inverse / q100_mixed_n16 | production_one_inverse | compact_ct | 1.037979 | 1.508941 | 1.452728 [1.443649, 1.473121] | regression |
| opt_batch_inverse / q100_mixed_n1024 | production_one_inverse | compact_vartime | 21.199707 | 25.968504 | 1.228075 [1.220316, 1.238102] | regression |
| opt_batch_inverse / q100_mixed_n1024 | production_one_inverse | compact_ct | 21.199707 | 26.408774 | 1.250370 [1.239085, 1.257496] | regression |
| opt_batch_inverse / q100_zero_n16 | production_one_inverse | compact_vartime | 0.084921 | 0.978580 | 11.494392 [11.345045, 11.576141] | regression |
| opt_batch_inverse / q100_zero_n16 | production_one_inverse | compact_ct | 0.084921 | 1.495864 | 17.653257 [17.386089, 17.693954] | regression |
| opt_batch_inverse / q100_zero_n1024 | production_one_inverse | compact_vartime | 4.267882 | 25.584575 | 6.053523 [5.983867, 6.075027] | regression |
| opt_batch_inverse / q100_zero_n1024 | production_one_inverse | compact_ct | 4.267882 | 26.074116 | 6.175111 [6.100439, 6.195593] | regression |
| opt_batch_inverse / q128_nonzero_n16 | production_one_inverse | compact_vartime | 1.304260 | 1.301193 | 0.989347 [0.983991, 1.002315] | inconclusive |
| opt_batch_inverse / q128_nonzero_n16 | production_one_inverse | compact_ct | 1.304260 | 1.493749 | 1.145415 [1.136859, 1.151243] | regression |
| opt_batch_inverse / q128_nonzero_n1024 | production_one_inverse | compact_vartime | 26.723629 | 25.930015 | 0.970583 [0.965262, 0.977285] | pass |
| opt_batch_inverse / q128_nonzero_n1024 | production_one_inverse | compact_ct | 26.723629 | 25.980301 | 0.974708 [0.968142, 0.981902] | pass |
| opt_batch_inverse / q128_mixed_n16 | production_one_inverse | compact_vartime | 1.172806 | 1.299743 | 1.108856 [1.102304, 1.119782] | regression |
| opt_batch_inverse / q128_mixed_n16 | production_one_inverse | compact_ct | 1.172806 | 1.497324 | 1.280707 [1.272025, 1.287526] | regression |
| opt_batch_inverse / q128_mixed_n1024 | production_one_inverse | compact_vartime | 20.996988 | 25.949301 | 1.238579 [1.229992, 1.246118] | regression |
| opt_batch_inverse / q128_mixed_n1024 | production_one_inverse | compact_ct | 20.996988 | 26.108561 | 1.243799 [1.238391, 1.249056] | regression |
| opt_batch_inverse / q128_zero_n16 | production_one_inverse | compact_vartime | 0.084754 | 1.301028 | 15.369169 [15.233504, 15.446334] | regression |
| opt_batch_inverse / q128_zero_n16 | production_one_inverse | compact_ct | 0.084754 | 1.493271 | 17.627691 [17.493801, 17.764873] | regression |
| opt_batch_inverse / q128_zero_n1024 | production_one_inverse | compact_vartime | 4.243184 | 25.903137 | 6.134808 [6.089411, 6.151907] | regression |
| opt_batch_inverse / q128_zero_n1024 | production_one_inverse | compact_ct | 4.243184 | 26.054830 | 6.172156 [6.130594, 6.189238] | regression |
| opt_batch_inverse_reuse / q100_nonzero_n16 | compact_vartime | compact_ct | 1.085357 | 1.436783 | 1.326698 [1.318904, 1.335312] | regression |
| opt_batch_inverse_reuse / q100_nonzero_n1024 | compact_vartime | compact_ct | 24.718422 | 24.890789 | 1.011600 [1.003306, 1.020920] | inconclusive |
| opt_batch_inverse_reuse / q100_mixed_n16 | compact_vartime | compact_ct | 1.091818 | 1.448339 | 1.330342 [1.319414, 1.337469] | regression |
| opt_batch_inverse_reuse / q100_mixed_n1024 | compact_vartime | compact_ct | 24.510094 | 24.978352 | 1.013265 [1.007349, 1.021440] | inconclusive |
| opt_batch_inverse_reuse / q100_zero_n16 | compact_vartime | compact_ct | 0.909226 | 1.437968 | 1.587890 [1.580242, 1.603596] | regression |
| opt_batch_inverse_reuse / q100_zero_n1024 | compact_vartime | compact_ct | 24.254231 | 24.875812 | 1.028976 [1.008082, 1.032951] | regression |
| opt_batch_inverse_reuse / q128_nonzero_n16 | compact_vartime | compact_ct | 1.224421 | 1.433054 | 1.171402 [1.161889, 1.181067] | regression |
| opt_batch_inverse_reuse / q128_nonzero_n1024 | compact_vartime | compact_ct | 24.424481 | 24.854329 | 1.017667 [1.005034, 1.035585] | inconclusive |
| opt_batch_inverse_reuse / q128_mixed_n16 | compact_vartime | compact_ct | 1.236008 | 1.443589 | 1.170484 [1.162153, 1.181266] | regression |
| opt_batch_inverse_reuse / q128_mixed_n1024 | compact_vartime | compact_ct | 24.253906 | 24.736004 | 1.013058 [1.005370, 1.020089] | inconclusive |
| opt_batch_inverse_reuse / q128_zero_n16 | compact_vartime | compact_ct | 1.224808 | 1.430807 | 1.170087 [1.163761, 1.174792] | regression |
| opt_batch_inverse_reuse / q128_zero_n1024 | compact_vartime | compact_ct | 24.335449 | 24.873371 | 1.017912 [1.007262, 1.025884] | inconclusive |
| opt_gf_fixed / zero_n16 | actual_fixed_gf | prepared_formula | 0.012737 | 0.012873 | 1.010289 [1.003871, 1.020072] | inconclusive |
| opt_gf_fixed / zero_n1024 | actual_fixed_gf | prepared_formula | 0.742579 | 0.742287 | 0.997775 [0.993593, 1.003859] | inconclusive |
| opt_gf_fixed / zero_n65536 | actual_fixed_gf | prepared_formula | 47.634118 | 47.983719 | 1.007181 [0.998751, 1.013949] | inconclusive |
| opt_gf_fixed / half_n16 | actual_fixed_gf | prepared_formula | 0.012753 | 0.012909 | 1.011181 [1.004831, 1.017193] | inconclusive |
| opt_gf_fixed / half_n1024 | actual_fixed_gf | prepared_formula | 0.743047 | 0.747548 | 1.003390 [0.998783, 1.009113] | inconclusive |
| opt_gf_fixed / half_n65536 | actual_fixed_gf | prepared_formula | 47.674484 | 47.934894 | 1.003263 [0.999323, 1.016627] | inconclusive |
| opt_gf_fixed / full_n16 | actual_fixed_gf | prepared_formula | 0.012781 | 0.012931 | 1.010176 [1.004954, 1.016405] | inconclusive |
| opt_gf_fixed / full_n16 | actual_fixed_gf | public_scalar_dispatch | 0.012781 | 0.015076 | 1.176998 [1.171611, 1.186496] | regression |
| opt_gf_fixed / full_n1024 | actual_fixed_gf | prepared_formula | 0.744197 | 0.748139 | 1.005786 [0.999107, 1.012307] | inconclusive |
| opt_gf_fixed / full_n1024 | actual_fixed_gf | public_scalar_dispatch | 0.744197 | 0.827896 | 1.113041 [1.105582, 1.119992] | regression |
| opt_gf_fixed / full_n65536 | actual_fixed_gf | prepared_formula | 48.031898 | 48.379227 | 1.004772 [0.997852, 1.016025] | inconclusive |
| opt_gf_fixed / full_n65536 | actual_fixed_gf | public_scalar_dispatch | 48.031898 | 53.539071 | 1.112427 [1.103936, 1.125583] | regression |
| opt_gf_butterfly / full_n16 | actual_fixed_gf | public_scalar_dispatch | 0.015666 | 0.019110 | 1.222673 [1.213879, 1.226428] | regression |
| opt_gf_butterfly / full_n1024 | actual_fixed_gf | public_scalar_dispatch | 0.921844 | 1.071722 | 1.162063 [1.157035, 1.169757] | regression |
| opt_gf_butterfly / full_n65536 | actual_fixed_gf | public_scalar_dispatch | 59.861328 | 69.510422 | 1.157185 [1.153040, 1.178492] | regression |
| opt_gf_round / 16 | production_fused | wide2 | 0.092904 | 0.081162 | 0.874610 [0.868728, 0.879263] | pass |
| opt_gf_round / 1024 | production_fused | wide2 | 5.663025 | 4.792419 | 0.846588 [0.841117, 0.850218] | pass |
| opt_gf_round / 65536 | production_fused | wide2 | 362.427094 | 311.097625 | 0.858519 [0.852265, 0.864127] | pass |
| opt_phi8 / 16 | table_public_input | fixed_basis | 0.008868 | 0.094897 | 10.728789 [9.609085, 11.247432] | regression |
| opt_phi8 / 1024 | table_public_input | fixed_basis | 0.376347 | 5.946086 | 15.947072 [15.772075, 16.038001] | regression |
| opt_phi8 / 65536 | table_public_input | fixed_basis | 23.765707 | 381.799725 | 16.062473 [15.919455, 16.232543] | regression |
| opt_b127_mul / 16 | production | karatsuba | 0.019532 | 0.025378 | 1.299323 [1.294928, 1.303541] | regression |
| opt_b127_mul / 16 | production | pfold | 0.019532 | 0.025255 | 1.293526 [1.286804, 1.297284] | regression |
| opt_b127_mul / 1024 | production | karatsuba | 1.241150 | 1.598826 | 1.292054 [1.282210, 1.299844] | regression |
| opt_b127_mul / 1024 | production | pfold | 1.241150 | 1.612345 | 1.303911 [1.291478, 1.313383] | regression |
| opt_b127_mul / 65536 | production | karatsuba | 80.039383 | 103.813469 | 1.297744 [1.286062, 1.311720] | regression |
| opt_b127_mul / 65536 | production | pfold | 80.039383 | 105.080070 | 1.308095 [1.293991, 1.326448] | regression |
| opt_ood / log10 | production_blocked | indexed_products | 3.098206 | 2.310018 | 0.746846 [0.740321, 0.751257] | pass |
| opt_ood / log10 | production_blocked | collect_products | 3.098206 | 2.344809 | 0.757384 [0.750730, 0.762819] | pass |
| opt_ood / log10 | production_blocked | vector_products | 3.098206 | 2.340190 | 0.753341 [0.747839, 0.762998] | pass |
| opt_ood / log10 | production_blocked | prepared_products | 3.098206 | 2.306620 | 0.743972 [0.738435, 0.749710] | pass |
| opt_ood / log16 | production_blocked | indexed_products | 68.174476 | 65.766602 | 0.965383 [0.947464, 0.976172] | inconclusive |
| opt_ood / log16 | production_blocked | collect_products | 68.174476 | 65.859711 | 0.965615 [0.956168, 1.002292] | inconclusive |
| opt_ood / log16 | production_blocked | vector_products | 68.174476 | 66.717773 | 0.977432 [0.965793, 1.004383] | inconclusive |
| opt_ood / log16 | production_blocked | prepared_products | 68.174476 | 66.221680 | 0.976589 [0.960664, 0.991455] | inconclusive |
| opt_ood / log18 | production_blocked | indexed_products | 224.131500 | 221.007812 | 0.987991 [0.979336, 0.998049] | inconclusive |
| opt_ood / log18 | production_blocked | collect_products | 224.131500 | 223.127594 | 0.991786 [0.983072, 1.002951] | inconclusive |
| opt_ood / log18 | production_blocked | vector_products | 224.131500 | 224.958312 | 1.001932 [0.994547, 1.011047] | inconclusive |
| opt_ood / log18 | production_blocked | prepared_products | 224.131500 | 221.800781 | 0.984460 [0.979380, 0.994199] | inconclusive |
| opt_ood_reuse / log10 | allocate | indexed_scratch | 2.209371 | 1.902903 | 0.859833 [0.852841, 0.863510] | pass |
| opt_ood_reuse / log16 | allocate | scratch | 60.748695 | 58.767579 | 0.965470 [0.931937, 0.982153] | inconclusive |
| opt_ood_reuse / log16 | allocate | indexed_scratch | 60.748695 | 64.158203 | 1.054248 [1.021491, 1.068988] | regression |
| opt_ood_reuse / log18 | allocate | scratch | 200.240875 | 199.459625 | 0.990485 [0.978334, 1.005905] | inconclusive |
| opt_ood_reuse / log18 | allocate | indexed_scratch | 200.240875 | 222.669281 | 1.099258 [1.092663, 1.125032] | regression |
| opt_ntt / log8_lanes32 | production | depth_first | 46.258301 | 40.083985 | 0.865965 [0.862154, 0.869310] | pass |
| opt_ntt / log8_lanes32 | production | tiled_half | 46.258301 | 30.681480 | 0.662376 [0.659134, 0.665846] | pass |
| opt_ntt / log12_lanes8 | production | depth_first | 405.075562 | 366.442687 | 0.904096 [0.891205, 0.922246] | pass |
| opt_ntt / log12_lanes8 | production | tiled_half | 405.075562 | 327.799500 | 0.811855 [0.802922, 0.828473] | pass |
| opt_ntt / log15_lanes8 | production | depth_first | 3798.312000 | 3520.146000 | 0.925117 [0.912987, 0.937237] | pass |
| opt_ntt / log15_lanes8 | production | tiled_half | 3798.312000 | 3252.479000 | 0.858820 [0.847583, 0.867447] | pass |
| opt_ntt / log15_lanes32 | production | depth_first | 11180.792000 | 10369.312500 | 0.925829 [0.915645, 0.937295] | inconclusive |
| opt_ntt / log15_lanes32 | production | tiled_half | 11180.792000 | 9554.624500 | 0.856295 [0.849910, 0.870156] | pass |
| opt_ntt / log16_lanes32 | production | depth_first | 23745.000500 | 22485.062500 | 0.942471 [0.926333, 0.955636] | inconclusive |
| opt_ntt / log16_lanes32 | production | tiled_half | 23745.000500 | 20931.666500 | 0.876970 [0.867383, 0.887703] | pass |
| opt_ntt / log18_lanes32 | production | depth_first | 106120.083500 | 100078.917000 | 0.943522 [0.936164, 0.952280] | pass |
| opt_ntt / log18_lanes32 | production | tiled_half | 106120.083500 | 94356.083500 | 0.886743 [0.878563, 0.902937] | pass |
| opt_ntt / log8_lanes1 | production | depth_first | 2.661682 | 3.606293 | 1.358853 [1.326132, 1.385345] | regression |
| opt_ntt / log8_lanes1 | production | tiled_half | 2.661682 | 3.020487 | 1.140282 [1.092531, 1.158510] | regression |
| opt_ntt / log8_lanes1 | production | half_depth | 2.661682 | 3.008341 | 1.136911 [1.100778, 1.150532] | regression |
| opt_pack / logs9_9 | production | single_write | 24.981281 | 0.731323 | 0.033901 [0.025615, 0.130310] | pass |
| opt_pack / logs16_14 | production | single_write | 127.649078 | 95.916657 | 0.739079 [0.722241, 0.841442] | pass |
| opt_pack / logs18_18 | production | single_write | 407.171875 | 444.645875 | 1.077862 [1.069566, 1.157250] | regression |
| opt_packed_ood / logs9_9 | production | single_write_ood | 34.391113 | 2.963256 | 0.089760 [0.076072, 0.344780] | pass |
| opt_packed_ood / logs16_14 | production | single_write_ood | 263.328156 | 287.212250 | 1.099243 [0.894661, 1.116491] | inconclusive |
| opt_packed_ood / logs18_18 | production | single_write_ood | 848.640500 | 947.453125 | 1.080130 [1.034070, 1.192102] | regression |
| opt_gf_grid / 16 | production_fused | chunked | 0.827921 | 0.837794 | 1.011917 [1.005593, 1.022001] | inconclusive |
| opt_gf_grid / 1024 | production_fused | chunked | 53.807289 | 55.748047 | 1.039452 [1.026745, 1.049870] | regression |
| opt_f2_poly_dot / a1_b1_dense_n16 | production_zero_skip | fixed_schedule | 0.012180 | 0.005840 | 0.482119 [0.474886, 0.485493] | pass |
| opt_f2_poly_dot / a1_b1_dense_n16 | production_zero_skip | public_fused | 0.012180 | 0.011791 | 0.974700 [0.961887, 0.983016] | pass |
| opt_f2_poly_dot / a1_b1_dense_n16 | production_zero_skip | public_adaptive | 0.012180 | 0.007863 | 0.648060 [0.639428, 0.653297] | pass |
| opt_f2_poly_dot / a1_b1_dense_n16 | production_zero_skip | row_density | 0.012180 | 0.011649 | 0.961583 [0.951218, 0.970100] | pass |
| opt_f2_poly_dot / a1_b1_dense_n1024 | production_zero_skip | fixed_schedule | 0.659856 | 0.254413 | 0.385484 [0.379831, 0.387314] | pass |
| opt_f2_poly_dot / a1_b1_dense_n1024 | production_zero_skip | public_fused | 0.659856 | 0.641792 | 0.974226 [0.967356, 0.979086] | pass |
| opt_f2_poly_dot / a1_b1_dense_n1024 | production_zero_skip | public_adaptive | 0.659856 | 0.255593 | 0.388826 [0.385283, 0.392531] | pass |
| opt_f2_poly_dot / a1_b1_sparse_n16 | production_zero_skip | fixed_schedule | 0.012778 | 0.005880 | 0.459386 [0.395348, 0.467237] | pass |
| opt_f2_poly_dot / a1_b1_sparse_n16 | production_zero_skip | public_fused | 0.012778 | 0.009626 | 0.749840 [0.665019, 0.758352] | pass |
| opt_f2_poly_dot / a1_b1_sparse_n16 | production_zero_skip | public_adaptive | 0.012778 | 0.010894 | 0.854826 [0.748914, 0.859177] | pass |
| opt_f2_poly_dot / a1_b1_sparse_n16 | production_zero_skip | row_density | 0.012778 | 0.009612 | 0.751198 [0.668246, 0.754401] | pass |
| opt_f2_poly_dot / a1_b1_sparse_n1024 | production_zero_skip | fixed_schedule | 0.740550 | 0.254095 | 0.347522 [0.305936, 0.363563] | pass |
| opt_f2_poly_dot / a1_b1_sparse_n1024 | production_zero_skip | public_fused | 0.740550 | 0.474187 | 0.643102 [0.563487, 0.689271] | pass |
| opt_f2_poly_dot / a1_b1_sparse_n1024 | production_zero_skip | public_adaptive | 0.740550 | 0.475527 | 0.643201 [0.564683, 0.686091] | pass |
| opt_f2_poly_dot / a3_b7_dense_n16 | production_zero_skip | fixed_schedule | 0.142014 | 0.085949 | 0.605925 [0.598825, 0.610196] | pass |
| opt_f2_poly_dot / a3_b7_dense_n16 | production_zero_skip | public_fused | 0.142014 | 0.156542 | 1.108791 [1.098581, 1.114988] | regression |
| opt_f2_poly_dot / a3_b7_dense_n16 | production_zero_skip | public_adaptive | 0.142014 | 0.088635 | 0.625343 [0.617048, 0.629415] | pass |
| opt_f2_poly_dot / a3_b7_dense_n16 | production_zero_skip | row_density | 0.142014 | 0.089528 | 0.633216 [0.627369, 0.644322] | pass |
| opt_f2_poly_dot / a3_b7_dense_n1024 | production_zero_skip | fixed_schedule | 8.337483 | 6.645834 | 0.796123 [0.787436, 0.804658] | pass |
| opt_f2_poly_dot / a3_b7_dense_n1024 | production_zero_skip | public_fused | 8.337483 | 9.808105 | 1.173745 [1.165614, 1.185913] | regression |
| opt_f2_poly_dot / a3_b7_dense_n1024 | production_zero_skip | public_adaptive | 8.337483 | 6.657307 | 0.797973 [0.794401, 0.807065] | pass |
| opt_f2_poly_dot / a3_b7_dense_n1024 | production_zero_skip | row_density | 8.337483 | 5.599610 | 0.672300 [0.662435, 0.676241] | pass |
| opt_f2_poly_dot / a3_b7_sparse_n16 | production_zero_skip | fixed_schedule | 0.058285 | 0.085694 | 1.474170 [1.200852, 1.711905] | regression |
| opt_f2_poly_dot / a3_b7_sparse_n16 | production_zero_skip | public_fused | 0.058285 | 0.050162 | 0.882910 [0.849347, 0.899232] | pass |
| opt_f2_poly_dot / a3_b7_sparse_n16 | production_zero_skip | public_adaptive | 0.058285 | 0.050779 | 0.890290 [0.857641, 0.903375] | pass |
| opt_f2_poly_dot / a3_b7_sparse_n16 | production_zero_skip | row_density | 0.058285 | 0.058345 | 1.016619 [0.938024, 1.107907] | inconclusive |
| opt_f2_poly_dot / a3_b7_sparse_n1024 | production_zero_skip | fixed_schedule | 3.622009 | 6.639750 | 1.873571 [1.792126, 1.882986] | regression |
| opt_f2_poly_dot / a3_b7_sparse_n1024 | production_zero_skip | public_fused | 3.622009 | 3.060527 | 0.839523 [0.833844, 0.854835] | pass |
| opt_f2_poly_dot / a3_b7_sparse_n1024 | production_zero_skip | public_adaptive | 3.622009 | 3.073893 | 0.853875 [0.843752, 0.860268] | pass |
| opt_f2_poly_dot / a3_b7_sparse_n1024 | production_zero_skip | row_density | 3.622009 | 5.317423 | 1.459415 [1.437438, 1.494551] | regression |
| opt_f2_poly_dot / a9_b9_dense_n16 | production_zero_skip | fixed_schedule | 1.970682 | 0.837575 | 0.425093 [0.421933, 0.428582] | pass |
| opt_f2_poly_dot / a9_b9_dense_n16 | production_zero_skip | public_fused | 1.970682 | 1.940298 | 0.985681 [0.968820, 0.999502] | pass |
| opt_f2_poly_dot / a9_b9_dense_n16 | production_zero_skip | public_adaptive | 1.970682 | 0.843332 | 0.428817 [0.422274, 0.431415] | pass |
| opt_f2_poly_dot / a9_b9_dense_n1024 | production_zero_skip | fixed_schedule | 126.206391 | 53.540359 | 0.426023 [0.419438, 0.429722] | pass |
| opt_f2_poly_dot / a9_b9_dense_n1024 | production_zero_skip | public_fused | 126.206391 | 125.936844 | 0.993187 [0.986626, 1.011293] | inconclusive |
| opt_f2_poly_dot / a9_b9_dense_n1024 | production_zero_skip | public_adaptive | 126.206391 | 53.548828 | 0.422292 [0.419485, 0.429268] | pass |
| opt_f2_poly_dot / a9_b9_sparse_n16 | production_zero_skip | fixed_schedule | 0.224161 | 0.842762 | 3.732852 [3.460264, 4.413094] | regression |
| opt_f2_poly_dot / a9_b9_sparse_n16 | production_zero_skip | public_fused | 0.224161 | 0.215538 | 0.962196 [0.955546, 0.976588] | pass |
| opt_f2_poly_dot / a9_b9_sparse_n16 | production_zero_skip | public_adaptive | 0.224161 | 0.204313 | 0.912229 [0.843350, 0.932461] | pass |
| opt_f2_poly_dot / a9_b9_sparse_n1024 | production_zero_skip | fixed_schedule | 20.235272 | 54.013183 | 2.677366 [2.486838, 2.813358] | regression |
| opt_f2_poly_dot / a9_b9_sparse_n1024 | production_zero_skip | public_fused | 20.235272 | 25.087973 | 1.189083 [1.125352, 1.305113] | regression |
| opt_f2_poly_dot / a9_b9_sparse_n1024 | production_zero_skip | public_adaptive | 20.235272 | 14.814129 | 0.715380 [0.669248, 0.803070] | pass |
| opt_f2_poly_dot / a1_b1_zero_n16 | production_zero_skip | fixed_schedule | 0.012870 | 0.005892 | 0.455837 [0.453583, 0.460865] | pass |
| opt_f2_poly_dot / a1_b1_zero_n16 | production_zero_skip | public_fused | 0.012870 | 0.008365 | 0.651149 [0.646423, 0.657537] | pass |
| opt_f2_poly_dot / a1_b1_zero_n16 | production_zero_skip | public_adaptive | 0.012870 | 0.009651 | 0.753695 [0.744620, 0.758915] | pass |
| opt_f2_poly_dot / a1_b1_zero_n16 | production_zero_skip | row_density | 0.012870 | 0.008342 | 0.649822 [0.643682, 0.654256] | pass |
| opt_f2_poly_dot / a1_b1_zero_n1024 | production_zero_skip | fixed_schedule | 0.468617 | 0.254234 | 0.541531 [0.528128, 0.546560] | pass |
| opt_f2_poly_dot / a1_b1_zero_n1024 | production_zero_skip | public_fused | 0.468617 | 0.336619 | 0.721603 [0.705548, 0.726368] | pass |
| opt_f2_poly_dot / a1_b1_zero_n1024 | production_zero_skip | public_adaptive | 0.468617 | 0.337209 | 0.723575 [0.706202, 0.729704] | pass |
| opt_f2_poly_dot / a1_b1_dense_prefix_n16 | production_zero_skip | fixed_schedule | 0.011232 | 0.005893 | 0.530665 [0.506533, 0.538068] | pass |
| opt_f2_poly_dot / a1_b1_dense_prefix_n16 | production_zero_skip | public_fused | 0.011232 | 0.009537 | 0.824472 [0.798064, 0.884044] | pass |
| opt_f2_poly_dot / a1_b1_dense_prefix_n16 | production_zero_skip | public_adaptive | 0.011232 | 0.007942 | 0.722026 [0.679944, 0.725408] | pass |
| opt_f2_poly_dot / a1_b1_dense_prefix_n16 | production_zero_skip | row_density | 0.011232 | 0.009518 | 0.822071 [0.753120, 0.880617] | pass |
| opt_f2_poly_dot / a1_b1_dense_prefix_n1024 | production_zero_skip | fixed_schedule | 0.778875 | 0.253184 | 0.329183 [0.306734, 0.352788] | pass |
| opt_f2_poly_dot / a1_b1_dense_prefix_n1024 | production_zero_skip | public_fused | 0.778875 | 0.480641 | 0.628615 [0.579553, 0.668139] | pass |
| opt_f2_poly_dot / a1_b1_dense_prefix_n1024 | production_zero_skip | public_adaptive | 0.778875 | 0.255387 | 0.332735 [0.314176, 0.358908] | pass |
| opt_f2_poly_dot / a1_b1_sparse_prefix_n16 | production_zero_skip | fixed_schedule | 0.011962 | 0.005922 | 0.497184 [0.493585, 0.502512] | pass |
| opt_f2_poly_dot / a1_b1_sparse_prefix_n16 | production_zero_skip | public_fused | 0.011962 | 0.010708 | 0.899983 [0.891787, 0.908640] | pass |
| opt_f2_poly_dot / a1_b1_sparse_prefix_n16 | production_zero_skip | public_adaptive | 0.011962 | 0.011305 | 0.946628 [0.941036, 0.954337] | inconclusive |
| opt_f2_poly_dot / a1_b1_sparse_prefix_n16 | production_zero_skip | row_density | 0.011962 | 0.010772 | 0.895821 [0.890569, 0.907116] | inconclusive |
| opt_f2_poly_dot / a1_b1_sparse_prefix_n1024 | production_zero_skip | fixed_schedule | 0.666957 | 0.257843 | 0.387870 [0.382833, 0.392277] | pass |
| opt_f2_poly_dot / a1_b1_sparse_prefix_n1024 | production_zero_skip | public_fused | 0.666957 | 0.647006 | 0.975575 [0.963030, 0.984186] | inconclusive |
| opt_f2_poly_dot / a1_b1_sparse_prefix_n1024 | production_zero_skip | public_adaptive | 0.666957 | 0.650394 | 0.979177 [0.959522, 0.986139] | inconclusive |
| opt_f2_poly_dot / a1_b1_alternating_n16 | production_zero_skip | fixed_schedule | 0.012532 | 0.005937 | 0.473055 [0.468228, 0.479594] | pass |
| opt_f2_poly_dot / a1_b1_alternating_n16 | production_zero_skip | public_fused | 0.012532 | 0.013674 | 1.091172 [1.079472, 1.105625] | regression |
| opt_f2_poly_dot / a1_b1_alternating_n16 | production_zero_skip | public_adaptive | 0.012532 | 0.012471 | 0.999169 [0.986308, 1.009892] | inconclusive |
| opt_f2_poly_dot / a1_b1_alternating_n16 | production_zero_skip | row_density | 0.012532 | 0.013726 | 1.103060 [1.081743, 1.112954] | regression |
| opt_f2_poly_dot / a1_b1_alternating_n1024 | production_zero_skip | fixed_schedule | 0.668152 | 0.254120 | 0.380243 [0.377521, 0.383483] | pass |
| opt_f2_poly_dot / a1_b1_alternating_n1024 | production_zero_skip | public_fused | 0.668152 | 0.325071 | 0.487270 [0.484536, 0.490704] | pass |
| opt_f2_poly_dot / a1_b1_alternating_n1024 | production_zero_skip | public_adaptive | 0.668152 | 0.326711 | 0.490693 [0.487216, 0.495684] | pass |
| opt_f2_poly_dot / a3_b7_zero_n16 | production_zero_skip | fixed_schedule | 0.020271 | 0.086723 | 4.246550 [4.224825, 4.359545] | regression |
| opt_f2_poly_dot / a3_b7_zero_n16 | production_zero_skip | public_fused | 0.020271 | 0.018870 | 0.930515 [0.922637, 0.935742] | pass |
| opt_f2_poly_dot / a3_b7_zero_n16 | production_zero_skip | public_adaptive | 0.020271 | 0.019520 | 0.963055 [0.956103, 0.968215] | pass |
| opt_f2_poly_dot / a3_b7_zero_n16 | production_zero_skip | row_density | 0.020271 | 0.008772 | 0.432453 [0.426494, 0.435625] | pass |
| opt_f2_poly_dot / a3_b7_zero_n1024 | production_zero_skip | fixed_schedule | 1.116567 | 6.625514 | 5.943733 [5.921901, 5.979265] | regression |
| opt_f2_poly_dot / a3_b7_zero_n1024 | production_zero_skip | public_fused | 1.116567 | 0.990830 | 0.883544 [0.879016, 0.893141] | pass |
| opt_f2_poly_dot / a3_b7_zero_n1024 | production_zero_skip | public_adaptive | 1.116567 | 0.995056 | 0.895386 [0.885214, 0.902087] | pass |
| opt_f2_poly_dot / a3_b7_zero_n1024 | production_zero_skip | row_density | 1.116567 | 0.419480 | 0.374897 [0.371672, 0.378361] | pass |
| opt_f2_poly_dot / a3_b7_dense_prefix_n16 | production_zero_skip | fixed_schedule | 0.085714 | 0.086983 | 1.004378 [0.988939, 1.150771] | inconclusive |
| opt_f2_poly_dot / a3_b7_dense_prefix_n16 | production_zero_skip | public_fused | 0.085714 | 0.078454 | 0.936423 [0.913679, 0.943979] | pass |
| opt_f2_poly_dot / a3_b7_dense_prefix_n16 | production_zero_skip | public_adaptive | 0.085714 | 0.088721 | 1.039457 [1.020802, 1.177979] | regression |
| opt_f2_poly_dot / a3_b7_dense_prefix_n16 | production_zero_skip | row_density | 0.085714 | 0.062484 | 0.746728 [0.705563, 0.792257] | pass |
| opt_f2_poly_dot / a3_b7_dense_prefix_n1024 | production_zero_skip | fixed_schedule | 3.584615 | 6.608541 | 1.867167 [1.727539, 1.901271] | regression |
| opt_f2_poly_dot / a3_b7_dense_prefix_n1024 | production_zero_skip | public_fused | 3.584615 | 3.066671 | 0.858082 [0.848121, 0.862895] | pass |
| opt_f2_poly_dot / a3_b7_dense_prefix_n1024 | production_zero_skip | public_adaptive | 3.584615 | 6.632588 | 1.877245 [1.744560, 1.906534] | regression |
| opt_f2_poly_dot / a3_b7_dense_prefix_n1024 | production_zero_skip | row_density | 3.584615 | 5.220621 | 1.472975 [1.449972, 1.481777] | regression |
| opt_f2_poly_dot / a3_b7_sparse_prefix_n16 | production_zero_skip | fixed_schedule | 0.106307 | 0.086571 | 0.807499 [0.795761, 0.823798] | pass |
| opt_f2_poly_dot / a3_b7_sparse_prefix_n16 | production_zero_skip | public_fused | 0.106307 | 0.118806 | 1.115691 [1.108429, 1.125522] | regression |
| opt_f2_poly_dot / a3_b7_sparse_prefix_n16 | production_zero_skip | public_adaptive | 0.106307 | 0.118737 | 1.113748 [1.107965, 1.123778] | regression |
| opt_f2_poly_dot / a3_b7_sparse_prefix_n16 | production_zero_skip | row_density | 0.106307 | 0.068902 | 0.647399 [0.644852, 0.655308] | pass |
| opt_f2_poly_dot / a3_b7_sparse_prefix_n1024 | production_zero_skip | fixed_schedule | 8.266195 | 6.637369 | 0.803426 [0.794355, 0.807689] | pass |
| opt_f2_poly_dot / a3_b7_sparse_prefix_n1024 | production_zero_skip | public_fused | 8.266195 | 9.650755 | 1.169369 [1.158472, 1.174603] | regression |
| opt_f2_poly_dot / a3_b7_sparse_prefix_n1024 | production_zero_skip | public_adaptive | 8.266195 | 9.669596 | 1.168195 [1.162188, 1.176538] | regression |
| opt_f2_poly_dot / a3_b7_sparse_prefix_n1024 | production_zero_skip | row_density | 8.266195 | 5.585369 | 0.673064 [0.668355, 0.678653] | pass |
| opt_f2_poly_dot / a3_b7_alternating_n16 | production_zero_skip | fixed_schedule | 0.078976 | 0.086934 | 1.091167 [1.081901, 1.116573] | regression |
| opt_f2_poly_dot / a3_b7_alternating_n16 | production_zero_skip | public_fused | 0.078976 | 0.080807 | 1.024750 [1.011569, 1.030092] | regression |
| opt_f2_poly_dot / a3_b7_alternating_n16 | production_zero_skip | public_adaptive | 0.078976 | 0.081001 | 1.024249 [1.020395, 1.029068] | regression |
| opt_f2_poly_dot / a3_b7_alternating_n16 | production_zero_skip | row_density | 0.078976 | 0.049283 | 0.620146 [0.615679, 0.626651] | pass |
| opt_f2_poly_dot / a3_b7_alternating_n1024 | production_zero_skip | fixed_schedule | 4.671671 | 6.620687 | 1.417554 [1.408749, 1.426269] | regression |
| opt_f2_poly_dot / a3_b7_alternating_n1024 | production_zero_skip | public_fused | 4.671671 | 4.903768 | 1.050763 [1.046439, 1.057042] | regression |
| opt_f2_poly_dot / a3_b7_alternating_n1024 | production_zero_skip | public_adaptive | 4.671671 | 4.908610 | 1.047346 [1.044070, 1.058817] | regression |
| opt_f2_poly_dot / a3_b7_alternating_n1024 | production_zero_skip | row_density | 4.671671 | 3.017965 | 0.644820 [0.642807, 0.653662] | pass |
| opt_f2_poly_dot / a9_b9_zero_n16 | production_zero_skip | fixed_schedule | 0.084987 | 0.842648 | 9.884835 [9.838433, 9.934989] | regression |
| opt_f2_poly_dot / a9_b9_zero_n16 | production_zero_skip | public_fused | 0.084987 | 0.073195 | 0.860071 [0.853194, 0.864427] | pass |
| opt_f2_poly_dot / a9_b9_zero_n16 | production_zero_skip | public_adaptive | 0.084987 | 0.061440 | 0.723857 [0.715017, 0.726629] | pass |
| opt_f2_poly_dot / a9_b9_zero_n1024 | production_zero_skip | fixed_schedule | 4.984924 | 53.925659 | 10.845533 [10.762651, 10.880244] | regression |
| opt_f2_poly_dot / a9_b9_zero_n1024 | production_zero_skip | public_fused | 4.984924 | 4.271382 | 0.858126 [0.851163, 0.865597] | pass |
| opt_f2_poly_dot / a9_b9_zero_n1024 | production_zero_skip | public_adaptive | 4.984924 | 3.288269 | 0.660939 [0.654800, 0.665054] | pass |
| opt_f2_poly_dot / a9_b9_dense_prefix_n16 | production_zero_skip | fixed_schedule | 0.639033 | 0.843020 | 1.325849 [1.304589, 1.363862] | regression |
| opt_f2_poly_dot / a9_b9_dense_prefix_n16 | production_zero_skip | public_fused | 0.639033 | 0.635630 | 0.996652 [0.986368, 1.001695] | inconclusive |
| opt_f2_poly_dot / a9_b9_dense_prefix_n16 | production_zero_skip | public_adaptive | 0.639033 | 0.849940 | 1.329401 [1.312729, 1.376112] | regression |
| opt_f2_poly_dot / a9_b9_dense_prefix_n1024 | production_zero_skip | fixed_schedule | 21.928062 | 53.811115 | 2.169619 [1.896452, 2.894574] | regression |
| opt_f2_poly_dot / a9_b9_dense_prefix_n1024 | production_zero_skip | public_fused | 21.928062 | 25.501953 | 1.054071 [0.884484, 1.390716] | inconclusive |
| opt_f2_poly_dot / a9_b9_dense_prefix_n1024 | production_zero_skip | public_adaptive | 21.928062 | 53.791095 | 2.166803 [1.895255, 2.898585] | regression |
| opt_f2_poly_dot / a9_b9_sparse_prefix_n16 | production_zero_skip | fixed_schedule | 1.495443 | 0.836395 | 0.557458 [0.552016, 0.563704] | pass |
| opt_f2_poly_dot / a9_b9_sparse_prefix_n16 | production_zero_skip | public_fused | 1.495443 | 1.454214 | 0.970191 [0.959081, 0.981678] | pass |
| opt_f2_poly_dot / a9_b9_sparse_prefix_n16 | production_zero_skip | public_adaptive | 1.495443 | 0.776642 | 0.520893 [0.515030, 0.525135] | pass |
| opt_f2_poly_dot / a9_b9_sparse_prefix_n1024 | production_zero_skip | fixed_schedule | 125.953781 | 53.546219 | 0.425933 [0.422968, 0.429743] | pass |
| opt_f2_poly_dot / a9_b9_sparse_prefix_n1024 | production_zero_skip | public_fused | 125.953781 | 125.476562 | 0.998088 [0.989548, 1.008056] | pass |
| opt_f2_poly_dot / a9_b9_sparse_prefix_n1024 | production_zero_skip | public_adaptive | 125.953781 | 65.095703 | 0.519333 [0.514427, 0.525659] | pass |
| opt_f2_poly_dot / a9_b9_alternating_n16 | production_zero_skip | fixed_schedule | 0.988297 | 0.835455 | 0.843771 [0.836596, 0.852528] | pass |
| opt_f2_poly_dot / a9_b9_alternating_n16 | production_zero_skip | public_fused | 0.988297 | 0.975179 | 0.984379 [0.975235, 0.996993] | pass |
| opt_f2_poly_dot / a9_b9_alternating_n16 | production_zero_skip | public_adaptive | 0.988297 | 0.534160 | 0.535371 [0.531119, 0.543537] | pass |
| opt_f2_poly_dot / a9_b9_alternating_n1024 | production_zero_skip | fixed_schedule | 63.608398 | 54.152992 | 0.852322 [0.842556, 0.858162] | pass |
| opt_f2_poly_dot / a9_b9_alternating_n1024 | production_zero_skip | public_fused | 63.608398 | 63.877281 | 0.999772 [0.991771, 1.014303] | inconclusive |
| opt_f2_poly_dot / a9_b9_alternating_n1024 | production_zero_skip | public_adaptive | 63.608398 | 34.159500 | 0.539144 [0.532539, 0.541785] | pass |

## 10 thread(s): repair-arm-10cores-03

Saved run: `2026-09-14T10:17:11-0700`. Five processes × 32 samples initially; the single-thread run also includes the prescribed five-process × 64-sample retry for selected inconclusive workloads. Full measurements: [source](/Users/johnwu/code/zk/f2z-pcs/experiments/field-regressions/results/repair-arm-10cores-03/measurements.md). Frozen binary SHA-256: `9d118faf1c2cb03e28e8cc23cadd3825780642e96eb98b6151ffc3fdf6d74815`.

19 selected checks; 7 retain their actual baseline.

### Frozen selections

| Operation / workload | Baseline kernel | Candidate kernel | Baseline median (µs) | Candidate median (µs) | Paired time ratio [95% CI] | Status |
|---|---|---|---:|---:|---|---|
| opt_ood / log10 | production_blocked | reuse_products | 3.091594 | 2.237773 | 0.721778 [0.695237, 0.728608] | pass |
| opt_ood / log16 | production_blocked | production_blocked | 87.587570 | 87.587570 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_ood / log18 | production_blocked | production_blocked | 129.783187 | 129.783187 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_ood_reuse / log10 | allocate | scratch | 2.245382 | 1.855519 | 0.825829 [0.820142, 0.833285] | pass |
| opt_ood_reuse / log16 | allocate | allocate | 102.325516 | 102.325516 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_ood_reuse / log18 | allocate | allocate | 141.712906 | 141.712906 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_ntt / log8_lanes32 | production | half_depth | 46.358886 | 30.510094 | 0.660517 [0.653103, 0.664521] | pass |
| opt_ntt / log12_lanes8 | production | half_depth | 429.059875 | 224.924500 | 0.530048 [0.499610, 0.572420] | pass |
| opt_ntt / log15_lanes8 | production | half_depth | 1181.682375 | 810.463625 | 0.695543 [0.660363, 0.725151] | pass |
| opt_ntt / log15_lanes32 | production | production | 2608.020750 | 2608.020750 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_ntt / log16_lanes32 | production | production | 5103.687500 | 5103.687500 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_ntt / log18_lanes32 | production | half_depth | 21528.146000 | 20542.020500 | 0.942535 [0.908767, 0.984643] | pass |
| opt_ntt / log8_lanes1 | production | production | 2.635193 | 2.635193 | 1.000000 [1.000000, 1.000000] | retained baseline |
| opt_pack / logs9_9 | production | tiled_write | 33.745445 | 0.591957 | 0.017363 [0.016894, 0.018031] | pass |
| opt_pack / logs16_14 | production | tiled_write | 194.899718 | 75.160156 | 0.405061 [0.292199, 0.424029] | pass |
| opt_pack / logs18_18 | production | tiled_write | 427.877625 | 162.822937 | 0.376062 [0.343024, 0.412456] | pass |
| opt_packed_ood / logs9_9 | production | tiled_indexed_ood | 60.454429 | 2.930656 | 0.047822 [0.046391, 0.053193] | pass |
| opt_packed_ood / logs16_14 | production | tiled_indexed_ood | 304.080750 | 172.730500 | 0.564327 [0.485341, 0.608750] | pass |
| opt_packed_ood / logs18_18 | production | tiled_indexed_ood | 609.903625 | 345.231750 | 0.567567 [0.540834, 0.581421] | pass |

### Other measured candidates

| Operation / workload | Baseline kernel | Candidate kernel | Baseline median (µs) | Candidate median (µs) | Paired time ratio [95% CI] | Status |
|---|---|---|---:|---:|---|---|
| opt_ood / log10 | production_blocked | indexed_products | 3.091594 | 2.327250 | 0.748951 [0.726051, 0.758002] | pass |
| opt_ood / log10 | production_blocked | collect_products | 3.091594 | 2.358357 | 0.758935 [0.730220, 0.765623] | pass |
| opt_ood / log10 | production_blocked | vector_products | 3.091594 | 2.357238 | 0.755736 [0.734052, 0.768881] | pass |
| opt_ood / log10 | production_blocked | prepared_products | 3.091594 | 2.328979 | 0.750172 [0.724086, 0.762004] | pass |
| opt_ood / log16 | production_blocked | reuse_products | 87.587570 | 103.848633 | 1.192681 [1.130912, 1.243887] | regression |
| opt_ood / log16 | production_blocked | indexed_products | 87.587570 | 104.188805 | 1.192506 [1.128036, 1.293276] | regression |
| opt_ood / log16 | production_blocked | collect_products | 87.587570 | 102.360680 | 1.184968 [1.108199, 1.262744] | regression |
| opt_ood / log16 | production_blocked | vector_products | 87.587570 | 105.021484 | 1.206617 [1.149754, 1.281574] | regression |
| opt_ood / log16 | production_blocked | prepared_products | 87.587570 | 104.017906 | 1.188503 [1.133421, 1.278419] | regression |
| opt_ood / log18 | production_blocked | reuse_products | 129.783187 | 141.539047 | 1.079291 [1.064321, 1.181963] | regression |
| opt_ood / log18 | production_blocked | indexed_products | 129.783187 | 144.261735 | 1.135651 [1.074284, 1.182995] | regression |
| opt_ood / log18 | production_blocked | collect_products | 129.783187 | 142.927750 | 1.094426 [1.070607, 1.185318] | regression |
| opt_ood / log18 | production_blocked | vector_products | 129.783187 | 144.483719 | 1.129758 [1.086880, 1.196091] | regression |
| opt_ood / log18 | production_blocked | prepared_products | 129.783187 | 143.637359 | 1.118900 [1.081533, 1.180113] | regression |
| opt_ood_reuse / log10 | allocate | indexed_scratch | 2.245382 | 1.929759 | 0.859385 [0.853034, 0.864628] | pass |
| opt_ood_reuse / log16 | allocate | scratch | 102.325516 | 104.733079 | 1.017032 [0.983019, 1.053308] | inconclusive |
| opt_ood_reuse / log16 | allocate | indexed_scratch | 102.325516 | 104.292953 | 1.030155 [0.985217, 1.182404] | inconclusive |
| opt_ood_reuse / log18 | allocate | scratch | 141.712906 | 145.442703 | 1.039865 [0.999392, 1.059042] | inconclusive |
| opt_ood_reuse / log18 | allocate | indexed_scratch | 141.712906 | 148.031250 | 1.046286 [1.001878, 1.074978] | inconclusive |
| opt_ntt / log8_lanes32 | production | depth_first | 46.358886 | 40.135582 | 0.864877 [0.855597, 0.871853] | pass |
| opt_ntt / log8_lanes32 | production | tiled_half | 46.358886 | 30.511883 | 0.658924 [0.654301, 0.662374] | pass |
| opt_ntt / log12_lanes8 | production | depth_first | 429.059875 | 241.815063 | 0.560963 [0.527836, 0.614263] | pass |
| opt_ntt / log12_lanes8 | production | tiled_half | 429.059875 | 272.033813 | 0.633775 [0.597733, 0.656620] | pass |
| opt_ntt / log15_lanes8 | production | depth_first | 1181.682375 | 839.953250 | 0.725722 [0.686534, 0.759611] | pass |
| opt_ntt / log15_lanes8 | production | tiled_half | 1181.682375 | 1039.760375 | 0.909169 [0.812816, 0.947866] | pass |
| opt_ntt / log15_lanes32 | production | depth_first | 2608.020750 | 2495.135250 | 0.944821 [0.916797, 0.989839] | inconclusive |
| opt_ntt / log15_lanes32 | production | tiled_half | 2608.020750 | 2586.104250 | 1.003305 [0.942746, 1.030935] | inconclusive |
| opt_ntt / log15_lanes32 | production | half_depth | 2608.020750 | 2397.364500 | 0.934357 [0.880404, 0.963583] | inconclusive |
| opt_ntt / log16_lanes32 | production | depth_first | 5103.687500 | 5144.937500 | 1.010665 [0.961417, 1.056227] | inconclusive |
| opt_ntt / log16_lanes32 | production | tiled_half | 5103.687500 | 5196.041500 | 1.007554 [0.974151, 1.059520] | inconclusive |
| opt_ntt / log16_lanes32 | production | half_depth | 5103.687500 | 4984.104000 | 0.977291 [0.912897, 1.011033] | inconclusive |
| opt_ntt / log18_lanes32 | production | depth_first | 21528.146000 | 21412.312500 | 0.962001 [0.942121, 1.031557] | inconclusive |
| opt_ntt / log18_lanes32 | production | tiled_half | 21528.146000 | 20995.583500 | 0.975724 [0.937904, 0.996806] | inconclusive |
| opt_ntt / log8_lanes1 | production | depth_first | 2.635193 | 3.573812 | 1.361131 [1.323834, 1.394548] | regression |
| opt_ntt / log8_lanes1 | production | tiled_half | 2.635193 | 3.022318 | 1.144560 [1.121376, 1.166868] | regression |
| opt_ntt / log8_lanes1 | production | half_depth | 2.635193 | 3.007507 | 1.137557 [1.115790, 1.173847] | regression |
| opt_pack / logs9_9 | production | single_write | 33.745445 | 0.724121 | 0.021443 [0.020642, 0.021968] | pass |
| opt_pack / logs16_14 | production | single_write | 194.899718 | 93.826813 | 0.507455 [0.389005, 0.525744] | pass |
| opt_pack / logs18_18 | production | single_write | 427.877625 | 442.403625 | 1.065185 [0.899646, 1.108834] | inconclusive |
| opt_packed_ood / logs9_9 | production | single_write_ood | 60.454429 | 2.988938 | 0.049060 [0.047238, 0.053913] | pass |
| opt_packed_ood / logs16_14 | production | single_write_ood | 304.080750 | 255.920562 | 0.843067 [0.817907, 0.868028] | inconclusive |
| opt_packed_ood / logs18_18 | production | single_write_ood | 609.903625 | 714.916625 | 1.165826 [1.112718, 1.215305] | regression |
