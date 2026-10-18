#!/bin/zsh
# Grinding luck: with fixed inputs every trial replays one transcript, so each
# large grinding site is one geometric draw. Re-measure the configurations over
# five input seeds (BITZ_FALCON_SEED 1..5); each run is a warm-up plus three
# proves on 10 threads. One binary (hybrid-tight: the ports plus the opt-in
# BITZ_FALCON_LINEAR_NUMERATOR switch).
set -u
D=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-flamegraph/bench_results/full-falcon-20261018
OUT=$D/seeds
mkdir -p $OUT
BIN=$D/bin/hybrid-tight
echo "$BIN $(md5 -q $BIN)" > $OUT/binaries.txt
cat $D/build-tight.rev >> $OUT/binaries.txt
ghz() { awk '/ user /{u=$3} /cycles elapsed/{c=$1} END{if(u>0) printf "%.2f", c/u/1e9; else print 0}' $1; }
run() {
  local name=$1 nap=$2; shift 2
  sleep $nap
  /usr/bin/time -l "$@" > $OUT/$name.jsonl 2> $OUT/$name.err
  local rc=$?
  echo "$(date +%T) $name exit=$rc ghz=$(ghz $OUT/$name.err) rss=$(awk '/maximum resident set size/{printf "%.0f", $1/1048576}' $OUT/$name.err)MiB $(pmset -g batt | grep -o '[0-9]*%; [a-z ]*')" >> $OUT/clock.log
}
B10=(--degree 512 --batch 1024 --security 128 --threads 10 --warmup 1 --iterations 3)
R=(BITZ_BENCH_BINDER_REPLAY=on)
PS=(BITZ_FALCON_GRINDING_MODEL=per-site)
T=(BITZ_FALCON_LINEAR_NUMERATOR=tight)
SP=(BITZ_FALCON_PRIME=small)
T16=(BITZ_FALCON_BRIDGE_ROWS=16)
for seed in 1 2 3 4 5; do
  run S0-s$seed 5 env RAYON_NUM_THREADS=10 BITZ_FALCON_SEED=$seed $BIN $B10
  run S1-s$seed 5 env RAYON_NUM_THREADS=10 BITZ_FALCON_SEED=$seed $R $BIN $B10
  run S2-s$seed 5 env RAYON_NUM_THREADS=10 BITZ_FALCON_SEED=$seed $R $PS $BIN $B10
  run S3-s$seed 5 env RAYON_NUM_THREADS=10 BITZ_FALCON_SEED=$seed $R $PS $T16 $BIN $B10
  run S4-s$seed 5 env RAYON_NUM_THREADS=10 BITZ_FALCON_SEED=$seed $R $PS $T16 $SP $BIN $B10
  run S5-s$seed 5 env RAYON_NUM_THREADS=10 BITZ_FALCON_SEED=$seed $R $PS $SP $BIN $B10
  run T2-s$seed 5 env RAYON_NUM_THREADS=10 BITZ_FALCON_SEED=$seed $R $PS $T $BIN $B10
  run T4-s$seed 5 env RAYON_NUM_THREADS=10 BITZ_FALCON_SEED=$seed $R $PS $T $T16 $SP $BIN $B10
  run T5-s$seed 5 env RAYON_NUM_THREADS=10 BITZ_FALCON_SEED=$seed $R $PS $T $SP $BIN $B10
done
echo DONE > $OUT/DONE
