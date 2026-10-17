#!/bin/zsh
# 10-thread prove/verify/peak memory at 32768 Falcon-512 signatures, 128-bit.
#   Aerie v0.2.0 as published: one batch per process (as its CI), rep 0 warm-up.
#   BitZ as shipped: John's falcon-poc e140a3f3 (+ bench-only cap lift); the
#     optimized binder forced (John enables it only at 1/8/12/16 workers), plus
#     one run with the default 10-worker policy.
#   BitZ experiment: falcon-ring-succinct d0ed8cfc1, per-site grinding, small
#     prime, keyed ring, 2^16 bridge rows, optimized binder.
set -u
D=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-flamegraph/bench_results/flamegraph-20261017
OUT=$D/t10
mkdir -p $OUT
ABIN=$D/aerie/target/release/deps/falcon-6efadfd380be1a7d
SBIN=$D/bin/bitz-shipped
EBIN=$D/bin/bitz-ring-t16
for b in $ABIN $SBIN $EBIN; do echo "$b $(md5 -q $b)"; done > $OUT/binaries.txt
pmset -g batt | tail -1 >> $OUT/binaries.txt
export AERIE_HONEST_FIXTURE_DIR=$D/aerie-fixtures BITZ_FALCON_CASE_CACHE=$D/falcon-cases RAYON_NUM_THREADS=10
ghz() { awk '/ user /{u=$3} /cycles elapsed/{c=$1} END{if(u>0) printf "%.2f", c/u/1e9; else print 0}' $1; }
run() {
  local name=$1 nap=$2; shift 2
  sleep $nap
  /usr/bin/time -l "$@" > $OUT/$name.out 2> $OUT/$name.err
  local rc=$?
  echo "$(date +%T) $name exit=$rc ghz=$(ghz $OUT/$name.err) rss=$(awk '/maximum resident set size/{printf "%.0f", $1/1048576}' $OUT/$name.err)MiB $(pmset -g batt | grep -o '[0-9]*%; [a-z ]*')" >> $OUT/clock.log
}
B=(--degree 512 --batch 32768 --security 128 --threads 10)
for rep in 0 1 2 3 4 5; do
  run aerie-r$rep 12 $ABIN 32768
done
run bitz-shipped 15 env BITZ_ARITHMETIC_BINDING_STRATEGY=optimized $SBIN $B --warmup 1 --iterations 5
run bitz-t16 15 env BITZ_FALCON_GRINDING_MODEL=per-site BITZ_FALCON_PRIME=small BITZ_ARITHMETIC_RING=keyed \
  BITZ_FALCON_BRIDGE_ROWS=16 BITZ_ARITHMETIC_BINDING_STRATEGY=optimized $EBIN $B --warmup 1 --iterations 5
run bitz-shipped-default 15 $SBIN $B --warmup 1 --iterations 3
echo DONE > $OUT/DONE
