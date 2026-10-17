#!/bin/zsh
# Re-profile the t=16 BitZ setup (one prove of the first run was disturbed).
set -u
D=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-flamegraph/bench_results/flamegraph-20261017
W=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-flamegraph
BBIN=$W/target-native/release/deps/falcon_algebraic-8edd41f39df7f30c
export BITZ_FALCON_CASE_CACHE=$D/falcon-cases
export BITZ_FALCON_GRINDING_MODEL=per-site BITZ_FALCON_PRIME=small
export BITZ_ARITHMETIC_RING=keyed BITZ_FALCON_BRIDGE_ROWS=16
export BITZ_PROFILE_IN_POOL=1 RAYON_NUM_THREADS=1
B=(--degree 512 --batch 32768 --security 128 --warmup 0 --threads 1)
ghz() { awk '/ user /{u=$3} /cycles elapsed/{c=$1} END{if(u>0) printf "%.2f", c/u/1e9; else print 0}' $1; }
for f in samply.json samply.syms.json samply.folded out err clock.out clock.err; do
  [ -e $D/samply/bitz-t16.$f ] && mv $D/samply/bitz-t16.$f $D/samply/bitz-t16.disturbed.$f
done
sleep 20
samply record --save-only --unstable-presymbolicate --rate 250 -o $D/samply/bitz-t16.samply.json -- \
  $BBIN $B --iterations 5 > $D/samply/bitz-t16.out 2> $D/samply/bitz-t16.err
echo "$(date +%T) bitz-t16 samply (rerun) exit=$?" >> $D/samply/clock.log
sleep 20
/usr/bin/time -l $BBIN $B --iterations 2 > $D/samply/bitz-t16.clock.out 2> $D/samply/bitz-t16.clock.err
echo "$(date +%T) bitz-t16 clock (rerun) exit=$? ghz=$(ghz $D/samply/bitz-t16.clock.err)" >> $D/samply/clock.log
