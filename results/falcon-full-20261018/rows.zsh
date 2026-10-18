#!/bin/zsh
# Small prime at intermediate bridge rows (t = 14, 15) on full Falcon-512,
# 1024 signatures, 10 threads, with the grinding reports; the small prime at
# 2^13 rows and the default prime at 2^16 rows repeated alongside for reference.
set -u
D=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-flamegraph/bench_results/full-falcon-20261018
OUT=$D/rows
mkdir -p $OUT
BIN=$D/bin/hybrid-ports
echo "$BIN $(md5 -q $BIN)" > $OUT/binaries.txt
cat $D/build-ports.rev >> $OUT/binaries.txt
ghz() { awk '/ user /{u=$3} /cycles elapsed/{c=$1} END{if(u>0) printf "%.2f", c/u/1e9; else print 0}' $1; }
run() {
  local name=$1 nap=$2; shift 2
  sleep $nap
  /usr/bin/time -l "$@" > $OUT/$name.jsonl 2> $OUT/$name.err
  local rc=$?
  echo "$(date +%T) $name exit=$rc ghz=$(ghz $OUT/$name.err) rss=$(awk '/maximum resident set size/{printf "%.0f", $1/1048576}' $OUT/$name.err)MiB $(pmset -g batt | grep -o '[0-9]*%; [a-z ]*')" >> $OUT/clock.log
}
BASE=(BITZ_BENCH_BINDER_REPLAY=on BITZ_FALCON_GRINDING_MODEL=per-site)
B10=(--degree 512 --batch 1024 --security 128 --threads 10 --warmup 1 --iterations 5)
for rep in 1 2; do
  for t in 13 14 15; do
    run small-t$t-r$rep 10 env RAYON_NUM_THREADS=10 $BASE BITZ_FALCON_PRIME=small BITZ_FALCON_BRIDGE_ROWS=$t $BIN $B10
  done
  run default-t16-r$rep 10 env RAYON_NUM_THREADS=10 $BASE BITZ_FALCON_BRIDGE_ROWS=16 $BIN $B10
  run default-t13-r$rep 10 env RAYON_NUM_THREADS=10 $BASE $BIN $B10
  run small-t16-r$rep 10 env RAYON_NUM_THREADS=10 $BASE BITZ_FALCON_PRIME=small BITZ_FALCON_BRIDGE_ROWS=16 $BIN $B10
done
for t in 13 14 15 16; do
  run small-t$t-report 5 env RAYON_NUM_THREADS=10 $BASE BITZ_FALCON_PRIME=small BITZ_FALCON_BRIDGE_ROWS=$t \
    BITZ_FALCON_PRINT_GRINDING=1 $BIN --degree 512 --batch 1024 --security 128 --threads 10 --warmup 0 --iterations 1
done
run small-t13-t1 15 env RAYON_NUM_THREADS=1 $BASE BITZ_FALCON_PRIME=small BITZ_FALCON_BRIDGE_ROWS=13 $BIN \
  --degree 512 --batch 1024 --security 128 --threads 1 --warmup 1 --iterations 3
echo DONE > $OUT/DONE
