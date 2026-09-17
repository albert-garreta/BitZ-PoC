# Integer and BabyBear PCS comparisons

PCS experiments are modes of the two multiplication targets. All configuration,
workers, raw records, and report aggregation are shared. See the
[multiplication benchmark guide](native-mul-compare.md).

```sh
cargo bench --bench mul_compare --features native-mul-compare,bench-internals -- \
  pcs --workload u32-full,baby-bear --backends all --log-n 15..=20 \
  --threads 1,8 --reps 21 --skip-unsupported --out results/pcs
python3 scripts/mul_report.py results/pcs --out reports/pcs
```

The same deterministic integer witness is materialized for each backend.
The u32 tuple is encoded as 32/32/64 bits and BabyBear as 31/31/31/31 bits.
F2Z opens a scaled assignment MLE at the fixed prime 2^100-15; the u32
adapter retains Paper binding and BabyBear retains Custom assignment binding.
WHIR uses Goldilocks for u32 and BabyBear for BabyBear, with the respective
challenge fields. Binius BaseFold and the binary Ligerito opener open the same
packed bit-MLE claim in GF(2^128).

Setup and witness creation are excluded from PCS time. Each trial records
materialization, commitment (including serialization/transcript seeding), claim
setup, opening, and verification. PCS time is that trial's materialization plus
commitment plus opening. Artifact byte accounting and codec checks happen
outside the timers. The report consumes these verified sizes directly.

Defaults are one warmup and 21 samples, exponents 15..25 for u32 and 15..24 for
BabyBear. Fixed-prime F2Z PCS currently uses W=1, split=0, and Lambda100;
virtual terminal openings are outside this benchmark refactor.
