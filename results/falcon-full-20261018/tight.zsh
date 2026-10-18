#!/bin/zsh
# Accounting experiment: the hybrid's linear-block numerator m+d+6 instead of
# m+d+B+5 (BITZ_FALCON_LINEAR_NUMERATOR=tight), full Falcon-512 at 1024
# signatures, 10 threads, per-site grinding and the binder replay throughout.
set -u
D=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-flamegraph/bench_results/full-falcon-20261018
OUT=$D/tight
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
BASE=(BITZ_BENCH_BINDER_REPLAY=on BITZ_FALCON_GRINDING_MODEL=per-site)
TIGHT=(BITZ_FALCON_LINEAR_NUMERATOR=tight)
B10=(--degree 512 --batch 1024 --security 128 --threads 10 --warmup 1 --iterations 5)
for rep in 1 2; do
  run default-t13-r$rep 10 env RAYON_NUM_THREADS=10 $BASE $BIN $B10
  run tight-default-t13-r$rep 10 env RAYON_NUM_THREADS=10 $BASE $TIGHT $BIN $B10
  run tight-default-t16-r$rep 10 env RAYON_NUM_THREADS=10 $BASE $TIGHT BITZ_FALCON_BRIDGE_ROWS=16 $BIN $B10
  run tight-small-t13-r$rep 10 env RAYON_NUM_THREADS=10 $BASE $TIGHT BITZ_FALCON_PRIME=small $BIN $B10
  run tight-small-t16-r$rep 10 env RAYON_NUM_THREADS=10 $BASE $TIGHT BITZ_FALCON_PRIME=small BITZ_FALCON_BRIDGE_ROWS=16 $BIN $B10
done
for cfg in "default-t13" "small-t13" "small-t16"; do
  case $cfg in
    default-t13) extra=() ;;
    small-t13) extra=(BITZ_FALCON_PRIME=small) ;;
    small-t16) extra=(BITZ_FALCON_PRIME=small BITZ_FALCON_BRIDGE_ROWS=16) ;;
  esac
  run tight-$cfg-report 5 env RAYON_NUM_THREADS=10 $BASE $TIGHT $extra BITZ_FALCON_PRINT_GRINDING=1 $BIN \
    --degree 512 --batch 1024 --security 128 --threads 10 --warmup 0 --iterations 1
done
run union-tight-report 5 env RAYON_NUM_THREADS=10 BITZ_BENCH_BINDER_REPLAY=on $TIGHT BITZ_FALCON_PRINT_GRINDING=1 $BIN \
  --degree 512 --batch 1024 --security 128 --threads 10 --warmup 0 --iterations 1
echo DONE > $OUT/DONE
