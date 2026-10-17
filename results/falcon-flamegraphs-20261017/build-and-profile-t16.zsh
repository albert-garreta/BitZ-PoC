#!/bin/zsh
# Rebuild with the keyed-ring prover frames kept (inline(never)), then
# re-profile the t=16 setup: samply (1 thread, five proves) and a clock check.
set -u
D=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-flamegraph/bench_results/flamegraph-20261017
W=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-flamegraph
cd $W
CARGO_TARGET_DIR=$W/target-native RUSTFLAGS='-C target-cpu=native' cargo bench --offline --no-run --profile release \
  --features falcon-hybrid,bench-internals --bench falcon_algebraic > $D/build-ring-frames.log 2>&1 || exit 1
sleep 30
exec $D/profile-t16-rerun.zsh
