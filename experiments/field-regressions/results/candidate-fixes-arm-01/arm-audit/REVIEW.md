# Instruction review: signed correction rejected

The performance gate passed, and independent arithmetic tests passed. This executable is nevertheless rejected as a private-arithmetic replacement for the new signed-column MAC.

The four-limb and nine-limb `exact_signed_columns` functions contain `tbnz`/`tbz` on bit 63 of loaded top operand limbs. These branches skip signed correction work when operand signs are nonnegative. See `kernel-05.asm` (0x1000389f0, 0x1000389f4) and `kernel-06.asm` (0x100038de4, 0x100038de8), whose corresponding top-limb loads and correction paths establish operand dependence. Plain source masks were insufficient to prevent this compiler optimization.

The follow-up candidate uses crypto-bigint CtSelect to construct masks through its conditional-selection backend. It requires a new instruction audit and fresh performance confirmation. These original measurements and assembly are preserved unchanged as evidence.
