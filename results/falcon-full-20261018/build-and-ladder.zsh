#!/bin/zsh
# Build the hybrid bench at the ports commit, let the machine cool, run the ladder.
set -u
D=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-flamegraph/bench_results/full-falcon-20261018
$D/build.zsh ports || exit 1
sleep 60
exec $D/ladder.zsh
