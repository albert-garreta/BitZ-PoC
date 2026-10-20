#!/bin/zsh
# Waits for AC power (without holding the shared bench lock), then runs the
# GF(2^256) campaign under the bench gate.
W=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-recursion
OUT=$W/bench_results/falcon-recursion-ext-20261021
mkdir -p $OUT
until pmset -g batt | grep -q "AC Power"; do
  echo "$(date +%T) waiting for AC power before the gate" >> $OUT/gate.log
  sleep 60
done
cd $W
python3 scripts/bench_gate.py run --label falcon-recursion-ext -- zsh results/falcon-recursion-ext-20261021/campaign.zsh >> $OUT/gate.log 2>&1
echo "$(date +%T) gate exit $?" >> $OUT/gate.log
