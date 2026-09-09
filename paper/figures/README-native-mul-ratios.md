# native-mul ratio figures

`native-mul-ratio-{prover,verifier,proof}.{tex,pdf}` plot F2Z divided by
Binius64 and by Plonky3 WHIR for 2^15 … 2^20 u32 multiplications, from the run
`PerfRuns/2026-09-08T20-07-32Z-native-mul` of `benches/mul_e2e_compare.rs`
(five samples per cell after one warm-up, medians). Log-scale y axis; the grey
line is parity, so values below 1 mean F2Z is faster or smaller. The `.png`
files are previews of the PDFs; `native-mul-ratios.csv` holds every value.

Regenerate (from the repo root; the F2Z proof bytes are not recorded by the bench):

```sh
python3 scripts/native_mul_ratio_figures.py PerfRuns/2026-09-08T20-07-32Z-native-mul \
  --f2z-proof-bytes 15=157000,16=185764,17=205848,18=231592,19=265292,20=295800 --compile
```

- **Prover** = the run's `online_prover_ms`: the complete native prover call
  after witness generation (commit + PIOP + PCS opening).
- **Verifier** = `verify_ms`.
- **Proof size**: Binius64 and WHIR bytes are the run's `proof_bytes`; the
  bench leaves F2Z's unset, so the F2Z bytes above were measured separately
  with `f2z --mul <e> --profile udr` (the same relation, Lambda100 profile and
  validated-UDR Ligerito opener that the bench's default constructor uses; its
  prover and verifier times reproduce the run's within noise, and the byte
  counts are identical to the 2026-09-04 table run).
- The Binius64 2^20 cell is out of line with its own trend (prover 2.27 s
  against 0.40 s at 2^19, verifier 269 ms against 29 ms); the run's samples
  say so and the figures show it as measured.
