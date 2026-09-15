# Draft reply to Albert

Albert — I checked the source and ran fresh comparisons of standard Binius64
and the Binius–Ligerito adapter at initial rates 1/2, 1/4, and 1/8.

The original four configurations appear to mean BaseFold and Ligerito, each at
1/2 and 1/8. I could not verify a historical rate-1/4 result corresponding to
the screenshot’s labels. We should correct that inconsistency. The fresh
rate-1/4 runs are additional experiments, so the expanded comparison has six
configurations.

The adapter retains the supported Binius constraint system and PIOP and changes
the PCS path to ring switching plus Johnson-regime Ligerito. The configured
implementations also differ in hashing/transcript (SHA-256 versus BLAKE3) and
folding parameters. Their “100-bit” labels have different scopes: BaseFold’s
target covers the FRI query phase, whereas the adapter reports a modeled
composition bound including grinding. I would describe those scopes explicitly
rather than assert identical complete-protocol statistical security.

These runs do not support a blanket claim that prover or memory changes are
negligible. At 2^22 multiplications, the changes from BaseFold to Ligerito were:

| Initial rate | Proof bytes, both sweeps | Witness-to-proof, focused sweep | Witness-to-proof, broader sweep |
| --- | ---: | ---: | ---: |
| 1/2 | −21% | +34% | +30% |
| 1/4 | −29% | +8% | +36% |
| 1/8 | −37% | −45% | +21% |

These proof-size reductions are specific to that size; for example, the 2^15
rate-1/2 Ligerito proof was larger. The full report includes prover stages and
peak RSS. There was background OS activity and significant timing variation;
the rate-1/8 prover comparison even changes direction between sweeps.
The repeated rate-1/4 BaseFold verifier median
changed from about 428 ms to 60 ms, so verifier speedups in particular need an
idle-machine confirmation before being presented as precise performance claims.
The largest Plonky3 case also ran with substantial memory compression and swap;
RSS alone understates its full footprint.

For SHA+ECDSA, the measured statement is one chain of 128 total SHA-256
compressions, including padding, over 8,128 message bytes, followed by one
P-256 ECDSA verification. Both backends constrain the SHA digest to the signature
input; the message/signature/public-input negative tests passed. “4 KB / 64
compressions” does not describe this workload: 64 total compressions correspond
to 4,032 message bytes, while exactly 4,096 bytes requires 65. This comparison
uses standard Binius64; the Ligerito adapter does not support the composed
circuit’s BMUL constraints.

For multiplication, the BitZ CLI really computes u32 × u32 → u64. Its wrapping
comparison still proves the same full-product relation, keeping the high limb
as auxiliary data, so the BitZ relation cost is the same. The two command families
use different input generators and timing drivers; they do not by themselves
establish identical runtime or a full-width-u32 comparison across every prover.
All 24 wide/wrapping pairs had identical recorded Ligerito configurations and
matrix geometry.

Supporting measurements, configurations and caveats:
[campaign report](/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-requested-_f62mead/REPORT.md).

This is a local draft only; it has not been sent to Slack.
