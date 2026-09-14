# ARM review of the frozen repair executable

Binary SHA-256: `9d118faf1c2cb03e28e8cc23cadd3825780642e96eb98b6151ffc3fdf6d74815`. Captures and per-file hashes are in
[index.json](index.json) and [ntt-index.json](ntt-index.json).

- **Exact unsigned columns, 2/4/9 limbs:** the captured branches compare slice
  lengths or iterate over public rows/limbs. Carries use arithmetic flags and
  `cinc`/`adc`, with no input-dependent carry branch. No input trimming occurs.
- **Four-limb wrapping MAC and signed projections:** the new executable still
  has public length/loop branches only in these captured bodies. Projection
  retains the earlier `CtSelect` sign correction; no private borrow branch was
  reintroduced.
- **Fixed-schedule 3x7 and 9x9 polynomial kernels:** loops/indexing depend on
  declared widths and lengths. There is no coefficient-zero skip. These remain
  separate from the explicitly variable-time public polynomial candidates.
- **NTT `half_depth` and `serial_tree` (ntt-03/04):** the twiddle lookup branches
  on public block bits. Zero/half-width dispatch uses that public twiddle.
  Other branches use sizes, loop counters, buffer-address overlap checks, or
  worker-pool state. Witness coefficients feed the straight-line multiply/XOR
  loops, not these decisions. Extra Rayon support captures are included for
  context, not presented as a complete dependency review.
- **Karatsuba OOD dot diagnostic:** three fixed polynomial multiplications per
  pair, with public length/loop decisions. Its existing callers provide nonempty
  chunks; LLVM specializes away the empty-loop entry check at that private
  call boundary. This candidate was not selected for promotion.
- **Masked prime batch inverse diagnostic:** public length/loop branches and
  fixed zero masks remain. The inverse-validity unwrap is always valid under
  the accepted prime-modulus/nonzero-masked-product precondition. This argument
  does not apply to a general composite ring.

No integer/floating division instruction occurs in the 12 primary captured
bodies. Checked `Option` helpers and public polynomial density dispatch have
intentional validity/value-dependent control flow; this review does not claim
that they implement the separate private `CtValue`/fixed-schedule contracts.

The packing spare-capacity writes were reviewed at source level: public shape
checks establish complete 16-slot groups; each value or padding slot is written
exactly once; length is set only after all workers complete. An unwind leaves
length zero, and copied field values have no destructor.

This is a bounded source/instruction review for the measured ARM build, not a
whole-prover or microarchitectural constant-time certification.
