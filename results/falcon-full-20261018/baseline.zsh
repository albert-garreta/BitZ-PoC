#!/bin/zsh
# Full Falcon-512 (John's falcon_hybrid) at 1024 signatures, 128-bit, 10 threads.
# HEAD b9a30ee66 (per-site grinding already wired into the hybrid).
#   U   union grinding, binder as shipped (macOS never enables the optimized replay)
#   UB  union, optimized binder replay forced (BITZ_BENCH_BINDER_REPLAY=on)
#   P   per-site grinding
#   PB  per-site, optimized binder replay forced
#   PB-trace  PB with span stage timings (phase breakdown, timings biased)
set -u
D=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-flamegraph/bench_results/full-falcon-20261018
OUT=$D/baseline
mkdir -p $OUT
BIN=$D/bin/hybrid-head
echo "$BIN $(md5 -q $BIN)" > $OUT/binaries.txt
pmset -g batt | tail -1 >> $OUT/binaries.txt
export RAYON_NUM_THREADS=10
ghz() { awk '/ user /{u=$3} /cycles elapsed/{c=$1} END{if(u>0) printf "%.2f", c/u/1e9; else print 0}' $1; }
run() {
  local name=$1 nap=$2; shift 2
  sleep $nap
  /usr/bin/time -l "$@" > $OUT/$name.jsonl 2> $OUT/$name.err
  local rc=$?
  echo "$(date +%T) $name exit=$rc ghz=$(ghz $OUT/$name.err) rss=$(awk '/maximum resident set size/{printf "%.0f", $1/1048576}' $OUT/$name.err)MiB $(pmset -g batt | grep -o '[0-9]*%; [a-z ]*')" >> $OUT/clock.log
}
B=(--degree 512 --batch 1024 --security 128 --threads 10 --warmup 1 --iterations 5)
run U 10 $BIN $B
run UB 10 env BITZ_BENCH_BINDER_REPLAY=on $BIN $B
run P 10 env BITZ_FALCON_GRINDING_MODEL=per-site $BIN $B
run PB 10 env BITZ_FALCON_GRINDING_MODEL=per-site BITZ_BENCH_BINDER_REPLAY=on $BIN $B
run U-r2 10 $BIN $B
run PB-r2 10 env BITZ_FALCON_GRINDING_MODEL=per-site BITZ_BENCH_BINDER_REPLAY=on $BIN $B
run PB-report 5 env BITZ_FALCON_GRINDING_MODEL=per-site BITZ_BENCH_BINDER_REPLAY=on BITZ_FALCON_PRINT_GRINDING=1 \
  $BIN --degree 512 --batch 1024 --security 128 --threads 10 --warmup 0 --iterations 1
run PB-trace 10 env BITZ_FALCON_GRINDING_MODEL=per-site BITZ_BENCH_BINDER_REPLAY=on BITZ_FALCON_STAGE_TIMINGS=1 \
  $BIN --degree 512 --batch 1024 --security 128 --threads 10 --warmup 1 --iterations 3
echo DONE > $OUT/DONE
