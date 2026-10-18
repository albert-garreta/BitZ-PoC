#!/bin/zsh
# Full-Falcon (hybrid) library tests: default switches, then with the
# bench-only bridge rows and small prime. The hybrid tests compile for the
# Falcon-1024 K=11 instantiation only; the code is generic in the degree.
set -u
W=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-flamegraph
D=$W/bench_results/full-falcon-20261018
cd $W
label=${1:-1}
export CARGO_TARGET_DIR=$W/target-native RUSTFLAGS='-C target-cpu=native'
cargo test --offline --release --lib --features falcon-hybrid,bench-internals --no-run > $D/test-$label-build.log 2>&1 || exit 1
filter=piop::spartan::falcon_profiles::n1024_k11
cargo test --offline --release --lib --features falcon-hybrid,bench-internals -- $filter > $D/test-$label-default.log 2>&1
echo "default exit=$?" >> $D/test-$label.status
BITZ_FALCON_GRINDING_MODEL=per-site cargo test --offline --release --lib --features falcon-hybrid,bench-internals -- $filter > $D/test-$label-persite.log 2>&1
echo "persite exit=$?" >> $D/test-$label.status
BITZ_FALCON_BRIDGE_ROWS=16 cargo test --offline --release --lib --features falcon-hybrid,bench-internals -- $filter > $D/test-$label-rows16.log 2>&1
echo "rows16 exit=$?" >> $D/test-$label.status
BITZ_FALCON_PRIME=small BITZ_FALCON_GRINDING_MODEL=per-site BITZ_FALCON_BRIDGE_ROWS=16 \
  cargo test --offline --release --lib --features falcon-hybrid,bench-internals -- $filter > $D/test-$label-small16.log 2>&1
echo "small16 exit=$?" >> $D/test-$label.status
echo DONE >> $D/test-$label.status
