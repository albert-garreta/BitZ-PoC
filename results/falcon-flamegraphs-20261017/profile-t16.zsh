#!/bin/zsh
# BitZ on origin/falcon-ring-succinct d0ed8cfc1 (+ bench-only profiling switch):
# small prime, keyed ring, 2^16 bridge rows, per-site grinding. samply profile
# (1 thread, 250 Hz, five proves), an unprofiled single-thread clock check, and
# a 10-thread prove for the proof-size breakdown.
set -u
D=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-flamegraph/bench_results/flamegraph-20261017
W=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-flamegraph
BBIN=$W/target-native/release/deps/falcon_algebraic-8edd41f39df7f30c
echo "$BBIN $(md5 -q $BBIN) $(git -C $W rev-parse HEAD)" > $D/samply/bitz-t16.binary.txt
export BITZ_FALCON_CASE_CACHE=$D/falcon-cases
export BITZ_FALCON_GRINDING_MODEL=per-site BITZ_FALCON_PRIME=small
export BITZ_ARITHMETIC_RING=keyed BITZ_FALCON_BRIDGE_ROWS=16
B=(--degree 512 --batch 32768 --security 128 --warmup 0)
ghz() { awk '/ user /{u=$3} /cycles elapsed/{c=$1} END{if(u>0) printf "%.2f", c/u/1e9; else print 0}' $1; }
sleep 20
(
  export BITZ_PROFILE_IN_POOL=1 RAYON_NUM_THREADS=1
  samply record --save-only --unstable-presymbolicate --rate 250 -o $D/samply/bitz-t16.samply.json -- \
    $BBIN $B --threads 1 --iterations 5 > $D/samply/bitz-t16.out 2> $D/samply/bitz-t16.err
  echo "$(date +%T) bitz-t16 samply exit=$?" >> $D/samply/clock.log
  sleep 20
  /usr/bin/time -l $BBIN $B --threads 1 --iterations 2 > $D/samply/bitz-t16.clock.out 2> $D/samply/bitz-t16.clock.err
  echo "$(date +%T) bitz-t16 clock exit=$? ghz=$(ghz $D/samply/bitz-t16.clock.err)" >> $D/samply/clock.log
)
(
  export RAYON_NUM_THREADS=10 BITZ_ARITHMETIC_BINDING_STRATEGY=optimized
  $BBIN $B --threads 10 --iterations 1 > $D/sizes/bitz-t16.out 2> $D/sizes/bitz-t16.err
  echo "$(date +%T) bitz-t16 size exit=$?" >> $D/sizes/log
)
