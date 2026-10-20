#!/bin/zsh
# Waits for AC power (without holding the shared bench lock), then runs the
# polylog campaign under the bench gate.
W=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-recursion
OUT=$W/bench_results/falcon-recursion-polylog-20261020
mkdir -p $OUT
until pmset -g batt | grep -q "AC Power"; do
  echo "$(date +%T) waiting for AC power before the gate" >> $OUT/gate.log
  sleep 60
done
cd $W
python3 scripts/bench_gate.py run --label falcon-recursion-polylog -- zsh results/falcon-recursion-polylog-20261020/campaign.zsh >> $OUT/gate.log 2>&1
echo "$(date +%T) gate exit $?" >> $OUT/gate.log
