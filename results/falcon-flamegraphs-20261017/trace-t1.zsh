#!/bin/zsh
# Span-traced single-thread BitZ runs (wall time per prover phase) to check the
# sampled flamegraphs' phase totals. Three proves each, no sampler attached.
set -u
D=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-flamegraph/bench_results/flamegraph-20261017
W=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-flamegraph
OUT=$D/trace
mkdir -p $OUT
BBIN=$W/target-native/release/deps/falcon_algebraic-8edd41f39df7f30c
export BITZ_FALCON_CASE_CACHE=$D/falcon-cases BITZ_PROFILE_IN_POOL=1 RAYON_NUM_THREADS=1
B=(--degree 512 --batch 32768 --security 128 --threads 1 --warmup 0 --iterations 3 --trace)
sleep 15
$BBIN $B > $OUT/bitz-shipped.jsonl 2> $OUT/bitz-shipped.stderr
echo "$(date +%T) bitz-shipped trace exit=$?" >> $OUT/clock.log
sleep 15
env BITZ_FALCON_GRINDING_MODEL=per-site BITZ_FALCON_PRIME=small $BBIN $B > $OUT/bitz-onelimb.jsonl 2> $OUT/bitz-onelimb.stderr
echo "$(date +%T) bitz-onelimb trace exit=$?" >> $OUT/clock.log
echo DONE > $OUT/DONE
