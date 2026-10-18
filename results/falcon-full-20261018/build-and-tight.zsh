#!/bin/zsh
# Build the hybrid bench with the linear-numerator experiment, cool down, run it.
set -u
D=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-flamegraph/bench_results/full-falcon-20261018
$D/build.zsh tight || exit 1
sleep 60
exec $D/tight.zsh
