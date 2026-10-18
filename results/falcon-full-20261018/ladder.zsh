#!/bin/zsh
# Full Falcon-512 (falcon_hybrid) at 1024 signatures, 128-bit: each change of
# the algebraic Falcon work added in turn. Binary: bin/hybrid-ports (HEAD plus
# the hybrid bridge-row and small-prime ports). Two interleaved repetitions of
# medians of 5 on 10 threads, then one thread for the first and last steps.
#   S0  as shipped: union grinding, reference binder (macOS never enables the replay)
#   S1  + optimized binder replay (BITZ_BENCH_BINDER_REPLAY=on)
#   S2  + per-site grinding
#   S3  + 2^16 bridge rows (default prime, two limbs)
#   S4  + small prime at 2^16 rows (unsplit bridge)
#   S5  small prime at 2^13 rows (unsplit bridge), for the t trade-off
set -u
D=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-flamegraph/bench_results/full-falcon-20261018
OUT=$D/ladder
mkdir -p $OUT
BIN=$D/bin/hybrid-ports
echo "$BIN $(md5 -q $BIN)" > $OUT/binaries.txt
cat $D/build-ports.rev >> $OUT/binaries.txt
pmset -g batt | tail -1 >> $OUT/binaries.txt
ghz() { awk '/ user /{u=$3} /cycles elapsed/{c=$1} END{if(u>0) printf "%.2f", c/u/1e9; else print 0}' $1; }
run() {
  local name=$1 nap=$2; shift 2
  sleep $nap
  /usr/bin/time -l "$@" > $OUT/$name.jsonl 2> $OUT/$name.err
  local rc=$?
  echo "$(date +%T) $name exit=$rc ghz=$(ghz $OUT/$name.err) rss=$(awk '/maximum resident set size/{printf "%.0f", $1/1048576}' $OUT/$name.err)MiB $(pmset -g batt | grep -o '[0-9]*%; [a-z ]*')" >> $OUT/clock.log
}
S0=()
S1=(BITZ_BENCH_BINDER_REPLAY=on)
S2=($S1 BITZ_FALCON_GRINDING_MODEL=per-site)
S3=($S2 BITZ_FALCON_BRIDGE_ROWS=16)
S4=($S3 BITZ_FALCON_PRIME=small)
S5=($S2 BITZ_FALCON_PRIME=small BITZ_FALCON_BRIDGE_ROWS=13)
B10=(--degree 512 --batch 1024 --security 128 --threads 10 --warmup 1 --iterations 5)
B1=(--degree 512 --batch 1024 --security 128 --threads 1 --warmup 1 --iterations 3)
for rep in 1 2; do
  for step in 0 1 2 3 4 5; do
    eval "vars=(\${S$step})"
    run S$step-t10-r$rep 10 env RAYON_NUM_THREADS=10 $vars $BIN $B10
  done
done
for step in 0 2 3 4; do
  eval "vars=(\${S$step})"
  run S$step-t1 15 env RAYON_NUM_THREADS=1 $vars $BIN $B1
done
run S4-report 5 env RAYON_NUM_THREADS=10 $S4 BITZ_FALCON_PRINT_GRINDING=1 $BIN \
  --degree 512 --batch 1024 --security 128 --threads 10 --warmup 0 --iterations 1
run S3-report 5 env RAYON_NUM_THREADS=10 $S3 BITZ_FALCON_PRINT_GRINDING=1 $BIN \
  --degree 512 --batch 1024 --security 128 --threads 10 --warmup 0 --iterations 1
echo DONE > $OUT/DONE
