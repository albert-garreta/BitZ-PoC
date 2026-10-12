# Falcon cost-reduction investigation

This study concerns the existing batch of 1,024 Falcon-1024 signatures at the
128-bit target. The baseline commits 176 MiB of packed input, including 160 MiB
for 20,480 Keccak permutations. Initial rate-1/2 codewords occupy 352 MiB.
The current proof payload is 2,443,748 bytes. Input size, encoded size, proof
payload, peak RAM, and proving time are different metrics.

The study does not change the Falcon prover. Candidate sizes below are layout
calculations using the vendored encoders' actual block-size constants and the
shared opening's physical-padding rules. They are not measurements of an
integrated optimized Falcon proof or promises of proportional runtime savings.

## Best first candidate: pack three permutations per block

The active encoder allocates 65,536 bits per permutation. Its 24 chi layers
contribute 38,400 AND-output bits; input/output states and the constant bring
the occupied positions to 41,601 bits, with alignment and domain padding making
up the rest. Merely changing the domain to 32,768 bits cannot fit those values.

The repository already has `keccak3.rs`, which packs three independent
permutations into 131,072 bits. The layout can represent consecutive SHAKE
permutations once the composition proves the links between their inputs and
outputs. The encoder alone does not establish these links.

For the existing 20 permutations per signature, use three slabs:

| Source | Current MiB | Proposed MiB |
|---|---:|---:|
| Arithmetic | 16 | 16 |
| First 12 permutations, three per block | — | 64 |
| Next 6 permutations, three per block | — | 32 |
| Final 2 permutations, one per block | — | 16 |
| Current first 16 permutations | 128 | — |
| Current final 4 permutations | 32 | — |
| **Committed input** | **176** | **128** |
| **Initial encoded codewords** | **352** | **256** |
| **Shared virtual opening domain** | **256** | **128** |

This saves 48 MiB of committed input (27.27%) while retaining all 20
permutations. The four sources occupy exactly 16 lanes in the current shared
opening geometry. There would be four source roots instead of three, so proof
size must be measured after integration: extra authentication paths can offset
some benefits of the smaller domain.

A naive single three-wide slab for all 20 permutations only reaches 144 MiB
total and leaves the shared domain at 256 MiB. Power-of-two rounding matters;
the 12+6+2 partition is better for this workload.

Integration requires adapting `PreparedKeccak`, generating the appropriate
BatchMajor witness, updating the fixed three-source/two-prefix arrays and SHAKE
copy/extraction gathers, and re-deriving the grinding/security accounting for
the changed prefixes. The study verified a standalone 49-permutation Keccak3
proof. Its legacy constructor still selects six-bit PCS interleaving while the
current embedded profile requires four; the study selects the matching width.
That smoke check uses the existing standalone profile, not the Falcon hybrid's
128-bit composition profile.

## Further candidate: prove only a sufficient SHAKE prefix

The current relation draws exactly 1,311 samples (1,024 + 287), requiring 20
permutations. Falcon's reference implementation also has a variable-length
HashToPoint procedure that consumes samples until 1,024 have been accepted;
the fixed-length version oversamples for a very small failure probability.
See the [reference HashToPoint code](https://falcon-sign.info/impl/common.c.html).

I regenerated the same 1,024 distinct, natively verified signatures with seed
42 and measured the first prefix containing 1,024 accepted samples:

| Required permutations | Signatures |
|---|---:|
| 16 | 360 |
| 17 | 664 |

The required sample counts ranged from 1,066 to 1,121. Individual minimum
prefixes total 17,048 permutations rather than 20,480 (16.76% less work by
operation count). One 17-permutation profile would perform 17,408 permutations.
This dataset does not establish a universal bound.

A new relation could prove prefix sufficiency and canonical ordered rejection
sampling, with a longer fallback when needed. Merely lowering the existing
fixed sample count is not a complete solution. HashToPoint compaction, linking,
transcript binding, and security accounting must change consistently.

With arithmetic held at its existing 16 MiB, a 17-permutation 12+3+2 layout
would use 112 MiB of committed input and a 128 MiB shared domain. This is a
conditional layout estimate, not an implemented result. Keeping the existing
single-permutation layout would use 160 MiB, because physical PCS padding
rounds the final nominal 8 MiB slab up to 16 MiB.

## Proof-payload reductions are separate

The compaction prover clones one common 11-element terminal point into 2,048
records, and the verifier reconstructs that point and checks every copy.
Those stored copies account for 360,448 bytes. Storing the point once would
save 360,272 bytes and bring the baseline stored payload to 2,083,476 bytes
(about 1.987 MiB), before any framing changes. A future transcript-aware codec
could reconstruct it instead. This does not reduce committed witness bits.

Other worthwhile targets are the 720,896 bytes of per-tree forest evaluations
and 524,288 bytes of bridge column sums. Batching forest reductions across the
signature dimension would require a new protocol argument; no savings are
claimed as validated here. Changing the bridge grid from 2^13 rows / 2^14
columns to 2^14 rows / 2^13 columns would halve the two column-sum vectors,
but would increase other work and require validation and timing measurements.

## Why simply porting SHA-256's trick is unlikely to suffice

The active Keccak encoder already derives intermediate states from the input
and chi AND outputs instead of committing every state. Replacing each AND
output by the next state still needs 1,600 bits per round in that design.
BitZ integer identities could change the verification relation, but do not by
themselves demonstrate a smaller source. No lower bound on alternative Keccak
arithmetizations is asserted here.

## Reproduce

```sh
RUSTFLAGS='-C target-cpu=native' cargo run --locked --release \
  --features falcon-hybrid,ecdsa,bench-internals \
  --example falcon_cost_study -- --batch 1024 --threads 16
```

The program generates inputs, verifies their native signatures, measures SHAKE
prefix lengths, prints the candidate geometries, and verifies the standalone
Keccak3 smoke proof. It does not generate an optimized Falcon proof or change
the existing Falcon implementation. For a smaller study use `--batch 4` or
larger; minimum-size Keccak3 slabs prevent the candidate four-root geometry
from fitting below that capacity.

Recommended order: integrate and benchmark the 12+6+2 packing while preserving
all 20 permutations; separately eliminate duplicated proof points; then assess
whether a prefix/fallback relation warrants the additional protocol work.
