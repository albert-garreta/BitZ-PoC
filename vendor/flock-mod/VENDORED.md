# Flock Ligerito initial-oracle adapter

`flock-core` is copied from `https://github.com/albert-garreta/flock-mod` at
`ed4c0cd9aceeae15248bf4f24a70d96fe37b2395`. Copyright notices and licenses
are retained.

Local changes let Ligerito authenticate its initial interleaved oracle using
two existing Merkle trees. Recursive levels retain the ordinary single-root
protocol. The initial layout fixes the lane placement, zero padding, leaf
widths, position domain, and hash before any challenges are sampled.

`flock-prover/src` is copied unchanged from the same revision for the SHA-256
R1CS witness generator and chain-shift reduction. Its manifest omits upstream
benches, examples, and development dependencies; the library uses the same
local `flock-core`. This source is preparation for a future selectable Flock
SHA prover. It is not connected to the hybrid runner yet; the current hybrid
uses Binius64 for SHA constraint proving and Flock/Ligerito for the shared
commitment and opening.
