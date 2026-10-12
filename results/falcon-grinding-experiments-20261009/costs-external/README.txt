External-entry grinding calibration, 2026-10-09

Use curves.json -> curves_unconstrained[threads][spartan|pcs][bits], in ns.
FROZEN SHA256:
bc66780fb414400468e5587537944b932b5f032aa0b8fa6a85f2f0cb6d2f132a
The original costs/curves.json was preserved byte-for-byte.

WHY THIS SECOND CALIBRATION
Both Falcon benchmark drivers invoke prepared.prove directly on a non-Rayon
main thread pinned to CPU6. The first calibration called grinders from inside
pool.install on Rayon worker0. In rayon-core1.13.0, broadcast_in constructs a
CountLatch::Stealing when the caller is a Rayon worker; that caller can execute
work while waiting. An external caller instead uses CountLatch::Blocking and
LockLatch::wait. Thus the benchmark includes external dispatch/wakeup/return
costs absent from the first calibration, and main/worker0 share CPU6.

This probe starts one GLOBAL Rayon pool per process, acknowledges every worker
pin, pins main to CPU6, and calls the actual public grinder entrypoints without
pool.install. Every recorded call observes Rayon worker index=-1 and CPU6.
Worker order:6,7,4,5,2,3,0,1,14,15,12,13,10,11,8,9.

MEASUREMENT AND FIT
Same native release library and kernels as the experimental Falcon binaries.
Spartan find_grinding_nonce uses a32-byte prefix. ZincChallenger::grind_pow uses
a16-byte prefix and includes its transcript seed extraction/nonce absorption.
The PCS transcript is plain Blake3Transcript; outer wrapper work is represented
by separate boundary counts, avoiding double counting it in the PCS curve.
No tracing subscriber. Seed construction, correctness checks, output formatting,
pool startup and250ms high-bit clock/pool preconditioning are outside timers.

Threads1/2/4:128 deterministic independent seeds per bit through18,64 above.
Threads8/16:1024 seeds per bit through18,512 above (extended replacements).
Three excluded warmups per cell. Bits1..24 for both kernels, plus PCS bit0.
104,576 measured searches across245 cells; all nonces verified, and matching
seed/kernel/difficulty tuples return identical minimum nonces across threads.

Per-cell nonnegative Huber robust affine regression fits elapsed ns versus
nonce+1 and evaluates at2^bits to remove chance differences in average nonce.
Bits25..32 are extrapolated from the fitted21..24 throughput, anchored at24.
Difficulty0 is modeled as zero grinding cost; actual PCS zero-bit transcript
overhead appears separately in summary.json. curves_unconstrained preserves
the parallel-threshold dips; curves is only an optional monotone upper envelope.
Nonce+1 measures the required minimum prefix, not redundant parallel hashes.

FINAL EXAMPLE EXPECTED SEARCH TIMES (microseconds)
                Spartan                   PCS
threads   bit11  bit12  bit13        bit11  bit12  bit13
   8       7.07   4.74   6.30          7.90   5.08   6.90
  16       7.16   5.68   6.19          7.91   5.83   6.58
Compared with the first worker-entry calibration, external entry adds material
short-search overhead; exact differences vary by difficulty/pool state. Do not
infer one universal dispatch constant from these measurements.

CPU/NOISE OBSERVATIONS
Every tN.resources.json contains per-CPU user,nice,system,idle,iowait,irq,
softirq,steal tick deltas and250ms snapshots. final-provenance.json summarizes
only the canonical final runs and the selected workers' SMT siblings.
Extended8-worker run lasted16.77s; max unused SMT load across all siblings0.42%.
Extended16-worker run lasted9.01s; max unused SMT load1.34%.
The initial short16-worker run saw7 USER ticks on CPU21 over1.5s (~4.67%),
zero system/irq/softirq there. Initial2-worker activity around CPUs26/29 was
mostly user time, but those CPUs are not siblings of its active cores6/7.
These observations do not support attributing the observed bursts solely to
benchmark-generated interrupts/softirqs. They do not identify the responsible
processes either. Tick quantization makes very short interval percentages coarse.

ARTIFACTS AND LIMITS
raw.csv, summary.csv/json, and curves.json include final1/2/4 and extended8/16.
Original short8/16 samples/resources are retained as t8.initial.* and
t16.initial.*; run.json describes the initial campaign, extended-run.json the
replacement runs. Read final-provenance.json when checking final curve inputs.
probe-initial.rs preserves the initial source; probe.rs adds a sample-count
multiplier used only for the extended runs. Both call identical library kernels.

This models repeated isolated searches from the correct caller context. Full
proofs interleave arithmetic/memory work between searches, changing worker sleep,
cache, clock and scheduler state. The cost objective is a useful schedule-search
model, not an end-to-end speedup guarantee. Confirm selected schedules on fresh
proof inputs with the actual prover. No protocol security target changes here.
