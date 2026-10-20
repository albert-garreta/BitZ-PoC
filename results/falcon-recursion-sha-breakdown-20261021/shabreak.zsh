#!/bin/zsh
# Per-stage in-circuit SHA-256 compressions of the Falcon verifier circuit
# (transcript / proof of work / Merkle, and the opening's Merkle split into
# leaves and nodes per level), from one recording per cell; the dev test
# binary of the Reproduce section. usage: zsh shabreak.zsh [out dir]
W=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-recursion
OUT=${1:-$(dirname $0)}
D=$W/target-dev/release/deps
BIN=$D/$(ls -t $D | grep -E '^bitz-[0-9a-f]+$' | head -1)
export BITZ_FALCON_GRINDING_MODEL=per-site BITZ_FALCON_PRIME=small BITZ_ARITHMETIC_RING=keyed
export BITZ_FALCON_BRIDGE_ROWS=16 BITZ_ARITHMETIC_BINDING_STRATEGY=optimized
for sec in 128 100; do
  for b in 1024 4096 32768; do
    BITZ_REC_STAGES=1 BITZ_REC_RECORDS=1 BITZ_REC_SECURITY=$sec BITZ_REC_FALCON_BATCH=$b RAYON_NUM_THREADS=10 \
      $BIN falcon_record_profile --ignored --nocapture > $OUT/shabreak-s$sec-b$b.out 2>&1
    echo "s$sec b$b exit $?"
  done
done
