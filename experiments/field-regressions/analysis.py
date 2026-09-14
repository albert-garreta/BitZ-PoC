"""One policy for measurement reports and the gate. No inferred case coverage."""
import csv
import json
import math
import random
import statistics
from pathlib import Path

SPEC_PATH = Path(__file__).with_name("required_cases.json")
DEFAULT_MARGIN = json.loads(SPEC_PATH.read_text())["max_slowdown"]


def quantile(values, q):
    xs = sorted(values)
    pos = (len(xs)-1)*q
    i = int(pos)
    return xs[i] + (xs[min(i+1, len(xs)-1)]-xs[i])*(pos-i)


def expanded(spec):
    for arch in spec["architectures"]:
        for group in spec["families"]:
            variants = group["variants"] + (group["arm_variants"] if arch == "aarch64" else [])
            for size in group["sizes"]:
                for variant in variants:
                    yield dict(arch=arch, family=group["name"], size=size, variant=variant,
                               baseline=group["baseline"], selected=(variant in group["selected"].get(arch, []) if isinstance(group["selected"].get(arch), list) else group["selected"].get(arch)==variant),
                               diagnostic=variant=="reset_only" or group["name"]=="fixed_prepare" or group.get("diagnostic",False))


def classify(row, margin=DEFAULT_MARGIN):
    if not row.get("complete"):
        return "unmeasured"
    if row.get("diagnostic"):
        return "unmeasured"
    if not row.get("correctness") or not row.get("allocations_ok"):
        return "regression"
    if row["variant"] == row["baseline"]:
        return "pass"
    limit = 1+margin
    if row["median_ci_low"] > limit or row["p95_ci_low"] > limit:
        return "regression"
    if (row["median_ci_high"] <= limit and row["p95_ci_high"] <= limit
            and max(row["per_run_medians"]) <= limit):
        return "pass"
    return "inconclusive"


def paired_stats(candidate, baseline, draws=2000):
    """Resample fresh processes, then paired batches within each process.

    P95 is a ratio of latency quantiles, NOT the P95 of elementwise ratios.
    The estimand is the median process-specific ratio for each statistic.
    """
    count = len(candidate)
    medians = [statistics.median(c)/statistics.median(b) for c,b in zip(candidate,baseline)]
    p95s = [quantile(c,.95)/quantile(b,.95) for c,b in zip(candidate,baseline)]
    rng = random.Random(817261)
    # Bootstrap each process independently, then resample process identities.
    within = []
    for c,b in zip(candidate,baseline):
        boot=[]
        for _ in range(draws):
            ids=rng.choices(range(len(c)),k=len(c))
            cs=[c[i] for i in ids];bs=[b[i] for i in ids]
            boot.append((statistics.median(cs)/statistics.median(bs),quantile(cs,.95)/quantile(bs,.95)))
        within.append(boot)
    median_boot=[];p95_boot=[]
    for _ in range(draws):
        selected=[within[r][rng.randrange(draws)] for r in rng.choices(range(count),k=count)]
        median_boot.append(statistics.median(x[0] for x in selected))
        p95_boot.append(statistics.median(x[1] for x in selected))
    return dict(median_ratio=statistics.median(medians),p95_ratio=statistics.median(p95s),
                median_ci_low=quantile(median_boot,.025),median_ci_high=quantile(median_boot,.975),
                p95_ci_low=quantile(p95_boot,.025),p95_ci_high=quantile(p95_boot,.975),per_run_medians=medians)


def summarize_round(directory, metadata, spec, draws=2000):
    directory=Path(directory);groups={};finished=[];runtimes=[]
    for run in range(metadata["runs"]):
        log=directory/f"run-{run}.log"
        content=log.read_text() if log.exists() else ""
        finished.append("CORRECTNESS_COMPLETE" in content)
        runtime=[json.loads(line[8:]) for line in content.splitlines() if line.startswith("RUNTIME ")]
        runtimes.append(runtime[-1] if runtime else {})
        path=directory/f"run-{run}.csv"
        if not path.exists():continue
        with path.open() as stream:
            for raw in csv.DictReader(stream):
                key=(raw["family"],raw["size"],raw["variant"])
                raw={k:(float(v) if k=="ns" else int(v) if k in ("rep","iterations","bytes","allocations","allocated_bytes","correctness") else v) for k,v in raw.items()}
                rep=(run,raw["rep"])
                if rep in groups.setdefault(key,{}):raise ValueError(f"duplicate sample {key}/{rep}")
                groups[key][rep]=raw
    actual_arch=metadata["arch"]
    def runtime_valid(r):
        common=(r.get("arch")==actual_arch and r.get("caller_threads")==metadata["threads"]
                and all(r.get(feature) for feature in spec["required_features"][actual_arch]))
        if spec.get("suite")=="arithmetic":
            return common and r.get("arithmetic_campaign")
        return (common and r.get("candidate_snapshot")
                and r.get("baseline_all_core_threads")==r.get("candidate_all_core_threads")==metadata["threads"])
    runtime_ok=all(runtime_valid(r) for r in runtimes)
    output=[]
    minimum_runs=spec["confirmation_runs"];minimum_samples=spec["confirmation_samples"]
    for item in expanded(spec):
        row=dict(item,complete=False,status="unmeasured",round=directory.name)
        key=(item["family"],item["size"],item["variant"])
        base_key=(*key[:2],item["baseline"])
        samples=groups.get(key,{}) if item["arch"]==actual_arch else {}
        base=groups.get(base_key,{})
        needed={(r,i) for r in range(metadata["runs"]) for i in range(metadata["samples"])}
        if (set(samples)==needed and set(base)==needed and all(finished) and runtime_ok
                and metadata["runs"]>=minimum_runs and metadata["samples"]>=minimum_samples
                and all(math.isfinite(v["ns"]) and v["ns"]>0 for v in [*samples.values(),*base.values()])):
            cand=[[samples[(r,i)]["ns"] for i in range(metadata["samples"])] for r in range(metadata["runs"])]
            ref=[[base[(r,i)]["ns"] for i in range(metadata["samples"])] for r in range(metadata["runs"])]
            row.update(complete=True,correctness=all(v["correctness"]==1 for v in samples.values()),
                       allocations_ok=all(samples[k]["allocations"]<=base[k]["allocations"] and samples[k]["allocated_bytes"]<=base[k]["allocated_bytes"] for k in needed),
                       allocation_calls=max(v["allocations"] for v in samples.values()),
                       allocation_bytes=max(v["allocated_bytes"] for v in samples.values()),
                       baseline_allocation_calls=max(v["allocations"] for v in base.values()),
                       payload_bytes=max(v["bytes"] for v in samples.values()),
                       median_ns=statistics.median(v["ns"] for v in samples.values()),
                       p95_ns=quantile([v["ns"] for v in samples.values()],.95),
                       p10_ns=quantile([v["ns"] for v in samples.values()],.1),
                       p90_ns=quantile([v["ns"] for v in samples.values()],.9),
                       p99_ns=quantile([v["ns"] for v in samples.values()],.99),samples=len(needed))
            if item["variant"]==item["baseline"]:
                row.update(median_ratio=1.,p95_ratio=1.,median_ci_low=1.,median_ci_high=1.,p95_ci_low=1.,p95_ci_high=1.,per_run_medians=[1.]*metadata["runs"])
            else:row.update(paired_stats(cand,ref,draws))
            row["status"]=classify(row,metadata["margin"])
        output.append(row)
    return output,runtimes


def write_report(out, rows, margin=DEFAULT_MARGIN, title="GF128 and NTT regression measurements"):
    out=Path(out)
    (out/"summary.json").write_text(json.dumps(rows,indent=2)+"\n")
    lines=["# "+title,"",
           f"Allowed slowdown: {margin:.1%}. Ratios are candidate / baseline. Intervals are pointwise 95% hierarchical paired bootstrap CIs.",
           "P95 describes timed-batch averages, not individual-call tails. Missing architectures/cases are unmeasured. Diagnostics cannot be selected.","",
           "| Arch | Operation | Size | Variant | Selected | Median ratio [CI] | P95 ratio [CI] | P50 / P95 (µs) | Payload (MiB) | Alloc calls / bytes | Status |",
           "|---|---|---|---|---|---|---|---:|---:|---:|---|"]
    for r in rows:
        values=[r["arch"],r["family"],r["size"],r["variant"],"yes" if r["selected"] else ""]
        if r["complete"]:
            values += [f'{r["median_ratio"]:.3f} [{r["median_ci_low"]:.3f}, {r["median_ci_high"]:.3f}]',
                       f'{r["p95_ratio"]:.3f} [{r["p95_ci_low"]:.3f}, {r["p95_ci_high"]:.3f}]',f'{r["median_ns"]/1000:.3f} / {r["p95_ns"]/1000:.3f}',f'{r["payload_bytes"]/2**20:.3f}',f'{r["allocation_calls"]} / {r["allocation_bytes"]}']
        else:values += ["—","—","—","—","—"]
        values += [r["status"] + (" (diagnostic)" if r["diagnostic"] else "")]
        lines.append("| "+" | ".join(values)+" |")
    (out/"measurements.md").write_text("\n".join(lines)+"\n")
