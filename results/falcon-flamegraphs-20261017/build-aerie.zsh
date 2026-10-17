#!/bin/zsh
# Aerie v0.2.0 bench, built as published (cargo bench profile, no LTO, no
# target-cpu), plus its honest Falcon-512 fixtures for 32768 signatures.
set -u
D=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-flamegraph/bench_results/flamegraph-20261017
export CARGO_TARGET_DIR=$D/aerie/target
cargo +1.95 bench --locked --manifest-path $D/aerie/Cargo.toml -p aerie-bench --features falcon-bench --bench falcon --no-run > $D/build-aerie.log 2>&1
echo "BUILD-EXIT=$?" >> $D/build-aerie.log
BIN=$(ls -t $D/aerie/target/release/deps/falcon-* | grep -v '\.d$' | head -1)
echo "BIN=$BIN" >> $D/build-aerie.log
export AERIE_HONEST_FIXTURE_DIR=$D/aerie-fixtures
mkdir -p $AERIE_HONEST_FIXTURE_DIR
RAYON_NUM_THREADS=10 $BIN --generate-honest-fixtures-only 32768 > $D/aerie-fixtures.log 2>&1
echo "FIXTURES-EXIT=$?" >> $D/build-aerie.log
