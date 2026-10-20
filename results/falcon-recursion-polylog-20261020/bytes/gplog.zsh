#!/bin/zsh
# Traced single-rep polylog runs of the bench binary for every campaign cell
# (10 threads): the outer proof's byte breakdown per part.
W=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-recursion
T=/Users/albertgarretafontelles/.claude/jobs/f60cc82e/tmp
BIN=$W/target-bench/release/deps/falcon_recursion-4f2a12645512d21d
export BITZ_FALCON_CASE_CACHE=$W/bench_results/falcon-recursion-20261018/cases
export BITZ_FALCON_GRINDING_MODEL=per-site BITZ_FALCON_PRIME=small BITZ_ARITHMETIC_RING=keyed
export BITZ_FALCON_BRIDGE_ROWS=16 BITZ_ARITHMETIC_BINDING_STRATEGY=optimized
for sec in 128 100; do
  lig=udrg:3:4
  [ $sec = 100 ] && lig=custom:6:4
  for b in 1024 4096 32768; do
    BITZ_TRACE=1 BITZ_REC_OPENER=log BITZ_REC_SECURITY=$sec BITZ_REC_LIG=$lig BITZ_REC_FALCON_BATCH=$b BITZ_REC_REPS=1 \
      RAYON_NUM_THREADS=10 $BIN > $T/gplog-s$sec-b$b.out 2>&1
    echo "s$sec b$b exit $?"
  done
done
