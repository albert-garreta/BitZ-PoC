# Binius64 protocol adapters

The prover and verifier crates are copied from
`https://github.com/wu-s-john/binius64` at
`2b27daea4a893fab930259cc7ad59d0a37c2ef95`. Other Binius crates remain pinned
to that revision through Git dependencies. Upstream copyright notices and
licenses are retained.

Local changes expose the witness evaluation immediately before ring switching.
The existing end-to-end APIs use the same reduction and retain their original
ring-switch and BaseFold behavior. The hybrid protocol discharges the exposed
claim against its original commitment through the shared opening instead.
