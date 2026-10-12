# Where the V4 Falcon proof bytes go

The next major targets are the integer column-sum table and Merkle
authentication. Together they occupy 77.6% of the 100-bit proof and 80.9% of
the 128-bit proof. Further HashToPoint forest compression has little scope:
its field messages already occupy only 6,016 bytes.

These are measured **complete stored proof payloads**, batch 1,024, seed 42,
current shared-prime V4. Both fresh proofs verified and matched the previous
input digest, proof Debug digest, and exact payload size. The diagnostic
does not alter proof generation or its transcript. Payload excludes Falcon
transport framing and the public statement; Falcon currently has no complete
transport codec. MB below means decimal megabytes; 0.4 MiB is 419,430 bytes.

## Measured breakdown

| Component | 100 bits | Share | 128 bits | Share |
| --- | ---: | ---: | ---: | ---: |
| Integer column sums | 262,144 B | 29.4% | 524,288 B | 40.5% |
| Authentication of three source commitments | 286,656 B | 32.1% | 348,480 B | 26.9% |
| Authentication in subsequent PCS rounds | 143,616 B | 16.1% | 175,168 B | 13.5% |
| Field values opened by the PCS | 136,832 B | 15.3% | 175,360 B | 13.5% |
| All remaining messages and framing | 63,218 B | 7.1% | 71,578 B | 5.5% |
| **Total** | **892,466 B** | | **1,294,874 B** | |

All authentication hashes together are **430,272 / 523,648 bytes**. The
entire shared-opening component, including source authentication, field
values, folding messages, ring switch and auxiliary nonces, is
**577,480 / 711,936 bytes**. One opening still authenticates three distinct
source trees and a sequence of recursive commitments.

The other 63–72 KB includes the 23,818-byte ring certificate/reduction,
9,504/11,528-byte arithmetic PIOP (including the joint HashToPoint forest),
8,736/9,152-byte binary bridge GKR messages, and 8,448/9,040-byte Keccak
PIOPs. Grinding nonces are small; grinding work is not a size bottleneck.

The PCS field openings split into 61,696/79,104 bytes at the initial level,
61,952/79,360 bytes across recursive levels, and 13,184/16,896 bytes at the
final level. Its bincode length prefixes cost 6,776/8,640 bytes. The raw
[28-component records](seed42-b1024.jsonl) retain every byte category.

## First target: the integer column table

Current geometry has 2^27 arithmetic-source bits, 2^13 rows, and 2^14
columns. At 100 bits, one integer per column costs 16,384 × 16 bytes. At
128 bits, two integers per column double that to 512 KiB.

**Straightforward encoding improvement at 128 bits:** each upper-limb sum is
at most `2^26 - 8192 = 67,100,672`, but currently occupies 16 bytes in the
stored-payload metric. A canonical four-byte encoding saves **196,608 bytes**,
reducing the column table from 524,288 to 327,680 bytes. Decode to the
existing u128 values and preserve the existing transcript and tighter
public-bound checks. This is an encoding change to implement, not permission
to change the accounting counter without changing the representation.

At the current row geometry, four columns per signature are entirely public
or zero: local indices 0, 13, 14 and 15. Their sums can be reconstructed from
the existing public statement, then absorbed and verified exactly as today.
Combined with four-byte upper sums, this would reduce the 128-bit table to
**245,760 bytes**; at 100 bits, public-column reconstruction alone leaves
**196,608 bytes**. This relies on the current statement containing the
signatures themselves. It would need reconsideration for private signatures.

**The larger improvement is to change the matrix geometry.** With 65,536 rows,
the same arithmetic source has only 2,048 columns. Keep the current prime
families and use two bounded limbs at both targets:

| Target | Current table | Proposed table at 65,536 rows | Encoding |
| --- | ---: | ---: | --- |
| 100 | 262,144 B | 38,912 B | 16-byte low + 3-byte upper |
| 128 | 524,288 B | 40,960 B | 16-byte low + 4-byte upper |

Split row weights at 110 bits. Low sums are strictly below 2^126. Upper sums
are below 2^21 at 100 bits and below 2^32 at 128 bits. These bounds retain
injectivity in the multiplicative group of GF(2^128). The bridge forest
messages grow by 1,760 bytes at 100 bits, or 1,344 bytes plus 336 bytes of
nonces at 128 bits. These figures concern the bridge component; they are not
new measured full proofs.

At 100 bits, simply increasing rows while keeping the current **unsplit**
bridge is invalid: it violates `(R+1)(p-1) < 2^128-1`. The two-limb change is
essential. The new row count requires parameterizing existing layout
assumptions, recertifying the full security ledger, and benchmarking the
larger row-image tables. More row work is a tradeoff to measure. Public-column
omission at the old geometry and this larger-row layout are alternative
savings calculations; do not add them together.

Relevant implementation: [bridge](../../../src/piop/spartan/falcon1024_ct/hybrid_bridge.rs),
[layout](../../../src/piop/spartan/falcon1024_ct/layout.rs),
[source offsets](../../../src/piop/spartan/falcon1024_ct/source.rs), and
[existing limb bounds](../../../src/piop/spartan/falcon1024_ct/SHARED_PRIME_V3.md).

## Second target: authenticate one joint source tree

The initial opening contains three Merkle multiproofs over the same query
positions: arithmetic, first 16 Keccak permutations, and last four Keccak
permutations. Each costs 95,552 bytes at 100 bits or 116,160 bytes at 128 bits.

Commit to a single tree of the combined source rows **before any proof
challenges**, binding source order, geometry, protocol version and statement.
Then one multiproof authenticates all three sources. At the current query
positions, this removes **191,104 / 232,320 bytes**. Actual new transcripts
will produce different positions and slightly different path sizes.

This requires a real joint commitment. Hashing the three existing roots
together does not remove their three authentication paths, and the existing
implementation already uses multiproofs. Further same-tree path sharing is
not an unimplemented easy win.

The shared virtual row has 16 field lanes, of which only 11 are occupied by
source data. Hashing all 16 lanes retains the current SIMD leaf-hash kernel;
the five required zero lanes can be omitted in a codec and reconstructed by
the verifier. This saves another **19,280 / 24,720 bytes** at the current
241/309 initial queries. Building one initial tree instead of three also
avoids approximately 256 MiB of tree storage at the current code rate, if the
old trees are never materialized. This is a storage-layout calculation, not
a measured whole-prover RSS improvement.

Relevant implementation: [shared opening](../../../src/hybrid/opening.rs),
[existing multiproofs](../../../vendor/flock-mod/crates/flock-core/src/pcs/ligerito.rs),
and [Merkle leaf kernels](../../../vendor/flock-mod/crates/flock-core/src/merkle.rs).

## What would reaching approximately 0.4 MB require?

Even deleting the entire integer column table for free leaves **630,322 bytes
at 100 bits and 770,586 bytes at 128 bits**. Reducing that table alone cannot
reach 0.4 MB. The following projections combine the two major changes above:

| Prospective design | 100 bits | 128 bits |
| --- | ---: | ---: |
| Larger-row bridge + joint source root + implicit zeros, current query positions | 460,610 B | 556,186 B |
| Also change initial RS rate from 1/2 to 1/4, expected authentication sizes | about 412 KB | about 498 KB |
| Rate 1/8 instead, expected authentication sizes | about 408 KB | about 493 KB |

These are **analytical size budgets, not implemented proofs**. The parameter
probe uses the actual PCS resolver and grinding planner. Rate 1/4 reduces
initial queries from 241 to 148 at 100 bits, and 309 to 189 at 128 bits,
while retaining the PCS security allocation. The full changed-geometry
Falcon ledger and implementation still require validation. Expected path
sizes use uniform distinct queries; changing transcript domains changes exact
lengths. [project.py](project.py) records the assumptions and recomputes
[the projections](projections.json).

Rate 1/4 is the more attractive first experiment: the estimated proof is
only about 4–5 KB larger than rate 1/8, but the codeword/tree storage is about
half as large. Approximate initial codeword-plus-tree storage is 480 MiB for
a joint root at rate 1/2, 960 MiB at rate 1/4, and 1,920 MiB at rate 1/8,
excluding witness and other working tables. Proving time must be measured.

Thus **around 0.4 MiB looks plausible at 100 bits with several concrete
changes**. The same examined changes budget roughly 0.50 MB at 128 bits,
still above both 0.4 MB and 0.4 MiB. This does not verify Albert's original
estimate: I could not locate that calculation. The paper's existing
[310 KB raw-PCS result at 2^30 bits](../../../paper/main.tex) is a 100-bit,
single-source Johnson/OOD measurement with different matrix geometry; it is
not a complete Falcon proof under the current profile.

Committing the column-sum bits and proving their read-off could eliminate
the table more completely, but is a larger redesign. The historical
[u-suppression note](../../../docs/u-suppression-prompt.md) describes that
direction and warns that a standalone extra PCS opening can erase the
savings. At 100 bits, blindly committing 128 sum bits is insufficient:
`2^128-1` aliases zero as an exponent. Strict range binding or bounded limbs
is necessary. Larger-row tuning and a joint source root are more direct
next experiments.

## Reproduction

```sh
RUSTFLAGS='-C target-cpu=native' CARGO_TARGET_DIR=target/falcon-native \
  cargo run --offline --locked --release --features falcon-hybrid \
  --example falcon_size_breakdown
python3 results/falcon-joint-forest-20261006/size-analysis/project.py
```

The new [diagnostic method](../../../src/piop/spartan/falcon1024_ct/hybrid_size.rs)
partitions the existing scalar-payload accounting without changing its total.
The [example](../../../examples/falcon_size_breakdown.rs) generates distinct
upstream Falcon inputs, verifies both full proofs, checks component sums, and
emits digests for comparison with the prior results. The independently rerun
PCS probe's source, parameter output, compile command and dependency hashes
are included here. Build/source provenance and validation are in
[manifest.json](manifest.json).
