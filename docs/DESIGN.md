# BitZ design

Wfbitz is the integer PCS used by every retained relation. Its optimized
exponent-fold/GKR implementation remains in `src/wfbitz/forest.rs`. The former
chunked PCS engine, virtual batching, dual-basis openings, extension-field,
tap, RLC and word-packing APIs have been removed.

## Proof flow

1. Commit the binary witness through the selected Ligerito configuration.
2. Bind the relation, layout, commitment, security policy and public statement.
   Johnson configurations bind an OOD claim before sampling the runtime prime.
3. Run Spartan and bind its terminal scaled claim.
4. Open with Wfbitz: integer column folds, exponent images and GKR, binary
   inner-product reduction, ring switching and Ligerito verification.

Multiplication uses the committed tensor directly. CM and linear SHA use the
shared virtual-opening adapter. SHA+ECDSA retains its structured opening and
checked dense fallback. MultiSwap retains its integer lift and second-prime
reduction. Hybrid shares a binary opening between SHA and multiplication.

`IntegerMatrixLayout` contains only row and column dimensions; cells are bits. Ordinary u32/u64/u128 operand widths remain supported.

## Geometry and security

Existing prime-selection rules and explicit layouts remain. Native parameters
check `(q - 1)(2^t + 1) < 2^128 - 1`, canonical residues and a full-order
generator. Explicit Ligerito configurations support smaller shapes than the
embedded ladder window. Map shortcuts check their complete eligibility rules.

`wfbitz/grinding.rs` enumerates native fold batching, GKR rounds and closing
challenges, binary sumchecks, ring-switch batching and OOD batching. Positive
work blocks authenticate stage/round nonces before challenges; zero-work blocks
add no transcript events. Work uses `3k / 2^128` and applicable profile minima.
Prover and verifier must complete the same schedule, which also drives reports.

Flock's challenge stream and internal work remain separate. The Ligerito
adapter validates additional work against its actual configuration.
`Lambda128` targets 128 bits for controllable terms and reports the existing
approximately 126.4-bit Flock floor.

## Formats and validation

Proof types contain concrete Wfbitz payloads. Backend selectors and empty host
nonce/sum fields are removed. Changed codecs reject old versions, malformed
lengths and trailing bytes. Upstream proof-byte parity is not a compatibility
contract: comparisons cover commitments and kernels; local roundtrips cover
the executed protocol.

See [the opening API](wfbitz-opener.md) and the reference/rejection tests.
The [archived design](history/forest-pcs-design.md) describes the retired PCS;
its historical measurements are not current performance evidence.
