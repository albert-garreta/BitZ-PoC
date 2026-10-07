# Aligned Falcon layout validation

The change preserves coefficient widths and accepted Falcon statements. It changes source addressing, transcript domains and PCS geometry for the algebraic prover. Existing proofs must be regenerated.

- Algebraic: signed 15-bit coefficients in 16-position blocks; 27 norm-slack bits occupy the first spare lanes. Remaining holes and inactive signatures must be zero. Minimum capacity is 16 to retain the PCS minimum.
- Full: aligned S1/S2/C/H blocks; bounded14, signed signature-width, unsigned14 and unsigned14 encodings remain unchanged. Exact supplied CT signature bytes map to aligned S2 bits. Full arithmetic and Keccak source domain sizes remain unchanged.
- Native extension-field aliases are reduced before canonical coordinate projection into the arithmetic prime.
- Full arithmetic padding is now authenticated through a randomized binary zero check merged into the existing joint opening. Active holes and inactive signatures have disjoint supports. The binary link error numerator increases from 128 to 256 and is bound into the profile digest and grinding schedule.
- No additional source commitment or proof field is introduced. The single-pass witness preparation path remains in place.

Validation scope: release unit tests for both Falcon implementations, dense coefficient/folding equivalence, shared joint sumcheck interval tests, all Falcon-512/1024 profile roundtrips, targeted one-thread tests, and matched before/after benchmarks at batches 32 and 1024, security targets 100 and 128, and 1/2/4/8/16 threads. Traced binding timings are separate diagnostic runs.

Initial test pass: 26 algebraic tests passed; full tests exposed two stale assertions (old 14-bit-stride addresses and old v4 transcript-label counting), both corrected. A subsequent test attempt used the older binary while the corrected rebuild was queued; those results are not the final qualification.

Final release qualification passed: 147 Falcon unit tests, 13 joint-sumcheck tests, 14 integer-bridge tests, 11 profile integration tests, and the explicitly enabled algebraic large-batch test (32/1024 at both targets): 186 distinct tests. One-thread repeats passed another 26 algebraic tests and the full 128-bit roundtrip test. The new full recommitted-padding regression passed all internal/trailing/inactive mutations at both targets.

Benchmarks completed: all 480 untraced proofs and 96 separate diagnostic proofs verified. Every main run checked input digests, the configured security bound, actual CPU affinities, payload breakdown totals, and expected source geometry. Main timings use five measured runs after one warm-up on a Ryzen 9 9950X3D, seed 42, physical CPUs 0 through thread_count-1.
