#!/bin/zsh
# Hybrid tests at 150f87658: default switches and the tight linear numerator.
set -u
W=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-flamegraph
D=$W/bench_results/full-falcon-20261018
cd $W
export CARGO_TARGET_DIR=$W/target-native RUSTFLAGS='-C target-cpu=native'
filter=piop::spartan::falcon_profiles::n1024_k11
cargo test --offline --release --lib --features falcon-hybrid,bench-internals -- $filter > $D/test-3-default.log 2>&1
echo "default exit=$?" >> $D/test-3.status
BITZ_FALCON_LINEAR_NUMERATOR=tight cargo test --offline --release --lib --features falcon-hybrid,bench-internals -- $filter > $D/test-3-tight.log 2>&1
echo "tight exit=$?" >> $D/test-3.status
echo DONE >> $D/test-3.status
