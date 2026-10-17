#!/bin/zsh
# Re-profile one BitZ setup (a concurrent build overlapped its first run).
set -u
D=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-flamegraph/bench_results/flamegraph-20261017
W=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-flamegraph
OUT=$D/samples
BBIN=$W/target-native/release/deps/falcon_algebraic-8edd41f39df7f30c
export BITZ_FALCON_CASE_CACHE=$D/falcon-cases BITZ_PROFILE_IN_POOL=1 RAYON_NUM_THREADS=1
name=bitz-shipped
for f in out err sample.txt sample.log; do [ -e $OUT/$name.$f ] && mv $OUT/$name.$f $OUT/$name.overlapped.$f; done
sleep 20
$BBIN --degree 512 --batch 32768 --security 128 --threads 1 --warmup 0 --iterations 5 > $OUT/$name.out 2> $OUT/$name.err &
pid=$!
/usr/bin/sample $pid 400 1 -mayDie -file $OUT/$name.sample.txt > $OUT/$name.sample.log 2>&1
wait $pid
echo "$(date +%T) $name profiled (rerun) exit=$? $(pmset -g batt | grep -o '[0-9]*%; [a-z ]*')" >> $OUT/clock.log
