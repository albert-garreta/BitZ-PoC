# Native SHA-256 comparison

The comparison has four backends: BitZ, Plonky3-WHIR, Binius64, and
Limber-Brakedown. Each proves the same independent fixed-IV compression
relation, `H[i] = Compress(IV, M[i])`, including its complete PCS opening.
The primary timer starts at native witness generation and stops when the proof
is ready. Setup, parameter selection, serialization, and verification are
reported separately or excluded from that interval.

From the BitZ checkout:

```sh
RAYON_NUM_THREADS=8 \
BITZ_SHA_COMPARE_BACKENDS="bitz plonky3-whir binius64 limber" \
bash scripts/run_native_sha256_compare.sh
```

The default exponents are `7 8 10 11 12 13 14 15 16`, with one warmup and
21 measured repetitions. The ordinary minimum is 128 compressions. The pilot
defaults to the smallest requested exponent, with a floor of 7. To run just
128 compressions, set `BITZ_SHA_COMPARE_EXPONENTS=7`.

The `limber` slug always uses `T256DynPrimeBdEngine`, whose commitment backend
is Brakedown. The pilot searches only Brakedown `k` values, and subprocesses use
the same engine. `BITZ_SHA_COMPARE_LIMBER_K` fixes `k` and skips that search.
The legacy engine setting accepts only `BITZ_SHA_COMPARE_LIMBER_ENGINE=brakedown`;
`hyrax` and the removed `spartan-hyrax` backend are rejected rather than remapped.
The default engine setting alone does not disable `k` selection.

Brakedown uses hash-based commitments. Saved metadata distinguishes its native
classical IntEval, challenge, and column-opening targets; these numbers are
not claims of equal quantum security across the four systems. The default
Brakedown column-opening target is 114 bits, and weaker `BDLAMBDA` overrides
are rejected. Other Brakedown geometry overrides are recorded in the metadata.

Fresh runs go to `PerfRuns/<timestamp>-native-sha256-brakedown/`, with JSON,
CSV, and trace artifacts under `artifacts/`. The summary schema is
`native-sha256-comparison/v3`. Existing Hyrax measurements are not relabeled.
Optional HTML reports use `ZK_TRACE_SCRIPT`; no private profiler path is needed
to run the benchmark.

Validation:

```sh
RUSTFLAGS=-Ctarget-cpu=native cargo test --release \
  --test native_sha256_compare \
  --features bench-internals,native-sha256-compare
python3 -B -m unittest discover -s scripts -p test_native_sha256_runner.py
```

The tests cover the four-backend policy, rejection of Hyrax selections,
Brakedown proof verification and rejection of altered public SHA inputs and
outputs, and the WHIR security schedules and proof rejection checks.
