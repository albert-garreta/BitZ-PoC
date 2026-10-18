#!/bin/zsh
# Record the resolved Ligerito shape (rates, queries) of the algebraic prover at
# 32,768 signatures for both page setups, via the bench-only grinding report.
set -u
W=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-flamegraph
D=$W/bench_results/flamegraph-20261017
OUT=$D/rate
mkdir -p $OUT
cd $W
git rev-parse HEAD > $OUT/build.rev
git status --short -- src benches >> $OUT/build.rev
CARGO_TARGET_DIR=$W/target-native RUSTFLAGS='-C target-cpu=native' cargo bench --offline --no-run --profile release \
  --features falcon-hybrid,bench-internals --bench falcon_algebraic > $OUT/build.log 2>&1 || exit 1
BIN=$W/$(grep -o 'target-native/release/deps/falcon_algebraic-[0-9a-f]*' $OUT/build.log | tail -1)
export BITZ_FALCON_CASE_CACHE=$D/falcon-cases BITZ_FALCON_PRINT_GRINDING=1 RAYON_NUM_THREADS=10
B=(--degree 512 --batch 32768 --security 128 --threads 10 --warmup 0 --iterations 1)
$BIN $B > $OUT/shipped.jsonl 2> $OUT/shipped.err
env BITZ_FALCON_GRINDING_MODEL=per-site BITZ_FALCON_PRIME=small BITZ_ARITHMETIC_RING=keyed BITZ_FALCON_BRIDGE_ROWS=16 \
  BITZ_ARITHMETIC_BINDING_STRATEGY=optimized $BIN $B > $OUT/t16.jsonl 2> $OUT/t16.err
echo DONE > $OUT/DONE
