# Public bridge exponentiation validation

The native release field suite passed **119 tests and doc-tests**, with zero failures across 21 executable/doc-test groups. The only ignored test was the isolated timing test, which passed separately. The three focused tests compare the new public byte-prefix calculation with both the frozen previous balanced tree and independent binary exponentiation. They cover zero/one/generator/arbitrary bases, exponent zero, 16/32/64-bit bucket boundaries, full-width values, composite and prime providers, private exponentiation, and generic public window sizes.

The CPU0 microbenchmark measured the 8,192-row split calculation at **0.650197 ms before and 0.348355 ms after** (ratio 0.535769, saving 0.301841 ms). The full 128-bit control measured 0.334170 ms before and 0.327227 ms after. Both methods use the same prepared generator table and identical inputs; setup is excluded. The test uses 13-coordinate prime equality weights and a 113/15-bit split. Current native Falcon upper limbs fit 13 bits, which selects the same short bucket.

This measures the exponentiation kernel only. The bridge now caches its fixed public generator table with `OnceLock`; first-use preparation remains inside its first invocation. No witness, transcript value, or sampled prime is cached. The frozen V7 integration build, cache integration test, full supported profile matrix, and cold proof equivalence checks have passed; see the [integration report](../REPORT.md). End-to-end timing acceptance remains pending the complete V7 campaign.

Integration evidence applies to `/tmp/falcon-compact-opening-candidate-v7` and its recorded binaries. Concurrent Falcon parameterization edits in the current working tree are outside the tested snapshot and are not validated by these results.

`method.json` records exact reproduction commands, compiler details, source/binary hashes, ABBA repetitions, and log hashes. Logs in this directory are unedited captured output.

Independent production review by `/root/joint_source_api_plan` passed with no findings at the recorded source hashes. `git diff --check` also passed.
