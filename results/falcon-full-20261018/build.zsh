#!/bin/zsh
# Build the full-Falcon (hybrid) bench with the same profile and flags as the
# algebraic campaigns: release profile, LTO, target-cpu=native, bench-internals.
set -u
W=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-flamegraph
D=$W/bench_results/full-falcon-20261018
cd $W
label=${1:-head}
git rev-parse HEAD > $D/build-$label.rev
git status --short -- src benches Cargo.toml >> $D/build-$label.rev
CARGO_TARGET_DIR=$W/target-native RUSTFLAGS='-C target-cpu=native' cargo bench --offline --no-run --profile release \
  --features falcon-hybrid,bench-internals --bench falcon_hybrid > $D/build-$label.log 2>&1 || exit 1
bin=$(grep -o 'target-native/release/deps/falcon_hybrid-[0-9a-f]*' $D/build-$label.log | tail -1)
mkdir -p $D/bin
cp $W/$bin $D/bin/hybrid-$label
echo "$(date +%T) built $bin -> bin/hybrid-$label" >> $D/build-$label.log
