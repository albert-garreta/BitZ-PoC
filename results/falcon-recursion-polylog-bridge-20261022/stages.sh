#!/bin/zsh
# Per-stage in-circuit SHA-256 compressions of the Falcon circuit for the
# standard and the polylog bridge (configuration G, ext:3:4 standard
# terminal, λ=128) at 1K/4K/32K: one recording each with BITZ_REC_STAGES=1,
# from the dev test binary of docs/falcon-recursion.md's Reproduce section.
# usage: zsh stages.sh [out dir]   then   python3 stages_table.py <out dir>
W=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-recursion
OUT=${1:-$(dirname $0)/stages}
mkdir -p $OUT
export BITZ_FALCON_GRINDING_MODEL=per-site BITZ_FALCON_PRIME=small BITZ_ARITHMETIC_RING=keyed
export BITZ_FALCON_BRIDGE_ROWS=16 BITZ_ARITHMETIC_BINDING_STRATEGY=optimized
export BITZ_FALCON_LIGERITO=ext:3:4 BITZ_FALCON_TERMINAL=standard
export BITZ_REC_SECURITY=128 BITZ_REC_LIG=ext:3:4 BITZ_REC_OPENER=std BITZ_REC_T=18
D=$W/target-dev/release/deps
BIN=$D/$(ls -t $D | grep -E '^bitz-[0-9a-f]+$' | head -1)
for b in 1024 4096 32768; do
  for bridge in standard polylog; do
    BITZ_FALCON_BRIDGE=$bridge BITZ_REC_STAGES=1 BITZ_REC_RECORDS=1 BITZ_REC_FALCON_BATCH=$b RAYON_NUM_THREADS=10 \
      $BIN falcon_record_profile --ignored --nocapture > $OUT/stages-$bridge-b$b.out 2>&1
    echo "$bridge b$b exit $?"
  done
done
