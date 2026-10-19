#!/bin/zsh
# Lever B: Johnson over GF(2^256) (`ext:R:4`, Round 0 in GF(2^256)) on
# algebraic Falcon-512, 32,768 signatures, 128-bit, 10 threads, setup G.
# Rates 1/2 .. 1/32, plus the UDR rate-1/2 baseline from the same binary;
# two input seeds, warm-up + 3 proves each.
set -u
W=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-flamegraph
D=$W/bench_results/pcs-levers-20261019
OUT=$D/ext
mkdir -p $OUT $D/bin
cd $W
git rev-parse HEAD > $OUT/build.rev
git status --short -- src benches vendor >> $OUT/build.rev
CARGO_TARGET_DIR=$W/target-native RUSTFLAGS='-C target-cpu=native' cargo bench --offline --no-run --profile release \
  --features falcon-hybrid,bench-internals --bench falcon_algebraic > $OUT/build.log 2>&1 || exit 1
cp $W/$(grep -o 'target-native/release/deps/falcon_algebraic-[0-9a-f]*' $OUT/build.log | tail -1) $D/bin/algebraic-ext
BIN=$D/bin/algebraic-ext
echo "$BIN $(md5 -q $BIN)" > $OUT/binaries.txt
sleep 45
ghz() { awk '/ user /{u=$3} /cycles elapsed/{c=$1} END{if(u>0) printf "%.2f", c/u/1e9; else print 0}' $1; }
run() {
  local name=$1 nap=$2; shift 2
  sleep $nap
  /usr/bin/time -l "$@" > $OUT/$name.jsonl 2> $OUT/$name.err
  local rc=$?
  echo "$(date +%T) $name exit=$rc ghz=$(ghz $OUT/$name.err) rss=$(awk '/maximum resident set size/{printf "%.0f", $1/1048576}' $OUT/$name.err)MiB $(pmset -g batt | grep -o '[0-9]*%; [a-z ]*')" >> $OUT/clock.log
}
export BITZ_FALCON_CASE_CACHE=$W/bench_results/flamegraph-20261017/falcon-cases RAYON_NUM_THREADS=10
G=(BITZ_FALCON_GRINDING_MODEL=per-site BITZ_FALCON_PRIME=small BITZ_ARITHMETIC_RING=keyed BITZ_FALCON_BRIDGE_ROWS=16 BITZ_ARITHMETIC_BINDING_STRATEGY=optimized)
for seed in 42 1; do
  run udr1-s$seed 15 env BITZ_FALCON_SEED=$seed $G BITZ_FALCON_LIGERITO=udrg:1:4 BITZ_FALCON_PRINT_GRINDING=1 \
    $BIN --degree 512 --batch 32768 --security 128 --threads 10 --warmup 1 --iterations 3
  for r in 1 2 3 4 5; do
    run ext$r-s$seed 15 env BITZ_FALCON_SEED=$seed $G BITZ_FALCON_LIGERITO=ext:$r:4 BITZ_FALCON_PRINT_GRINDING=1 \
      $BIN --degree 512 --batch 32768 --security 128 --threads 10 --warmup 1 --iterations 3
  done
done
echo DONE > $OUT/DONE
