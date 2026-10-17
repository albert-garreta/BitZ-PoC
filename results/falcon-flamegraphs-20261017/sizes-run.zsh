#!/bin/zsh
# One 32768-signature prove per BitZ setup, for the proof-size breakdown
# (including the new opening split). Sizes do not depend on the thread count.
set -u
D=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-flamegraph/bench_results/flamegraph-20261017
W=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-flamegraph
OUT=$D/sizes
mkdir -p $OUT
BBIN=$(ls -t $W/target-native/release/deps/falcon_algebraic-* | grep -v '\.d$' | head -1)
export BITZ_FALCON_CASE_CACHE=$D/falcon-cases RAYON_NUM_THREADS=10
B=(--degree 512 --batch 32768 --security 128 --threads 10 --warmup 0 --iterations 1)
$BBIN $B > $OUT/bitz-shipped.out 2> $OUT/bitz-shipped.err
echo "$(date +%T) bitz-shipped exit=$?" >> $OUT/log
(
  export BITZ_FALCON_GRINDING_MODEL=per-site BITZ_FALCON_PRIME=small BITZ_ARITHMETIC_BINDING_STRATEGY=optimized
  $BBIN $B > $OUT/bitz-onelimb.out 2> $OUT/bitz-onelimb.err
  echo "$(date +%T) bitz-onelimb exit=$?" >> $OUT/log
)
cp $D/samply/aerie.out $OUT/aerie.out
echo DONE >> $OUT/log
