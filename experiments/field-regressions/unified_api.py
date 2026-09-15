#!/usr/bin/env python3
"""Measure the actual typed API with the existing paired per-case gate.

Run only after the build has finished. This deliberately does not build inside
the measurement process. No result from this micro suite promotes a consumer.
"""
import argparse
import csv
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import tarfile
import shutil

from analysis import paired_stats, classify

ROOT = Path(__file__).resolve().parents[2]
FAMILIES = ["unified_mul", "unified_dot", "unified_linear"]
SIZES = [f"q{bits}_n{n}" for bits in [100,128] for n in [16,1024,65536]]

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary",type=Path)
    parser.add_argument("output",type=Path)
    parser.add_argument("--aa",action="store_true")
    parser.add_argument("--runs", type=int, choices=[5, 10], default=5)
    parser.add_argument("--samples", type=int, choices=[32, 64, 128], default=32)
    parser.add_argument("--seed-offset", type=int, default=1501)
    parser.add_argument("--calibration-ms", type=float, default=10.0)
    parser.add_argument("--policy", type=Path,
                        help="qualified A/A policy required for candidate measurements")
    args=parser.parse_args()
    if not args.aa and args.policy is None:
        parser.error("candidate measurements require a qualified --policy")
    if args.policy:
        qualified = json.loads(args.policy.read_text())
        if qualified.get("status") != "qualified" or qualified.get("host") != platform.platform():
            parser.error("policy must be qualified on this host")
        if qualified.get("keys") != [f"{family}/{size}" for family in FAMILIES for size in SIZES]:
            parser.error("policy does not cover the complete required case set")
        args.runs = qualified["runs"]
        args.samples = qualified["samples"]
        args.calibration_ms = qualified["calibration_ms"]
    if not 0 < args.calibration_ms < float("inf"):
        parser.error("calibration window must be finite and positive")
    binary=args.binary.resolve()
    out=args.output.resolve()
    out.mkdir(parents=True,exist_ok=False)
    runs=args.runs
    samples=args.samples
    variant="production_repeat" if args.aa else "typed"
    keys=[f"{family}/{size}" for family in FAMILIES for size in SIZES]
    policy={"runs":runs,"samples":samples,"seed_offset":args.seed_offset,"calibration_ms":args.calibration_ms,
            "margin":0.01,"median_and_p95_upper95_ci":1.01,"each_process_median_limit":1.01,
            "allocation_limit":"no increase",
            "qualification_sha256":sha(args.policy) if args.policy else None,
            "scope":"prime microbenchmarks only; consumer and x86 gates remain separate",
            "variant":variant,"keys":keys,"binary":str(binary),"binary_sha256":sha(binary),
            "baseline_provenance":str(binary.with_name("build.json")) if args.aa else None,
            "source_archive_role":"current campaign driver and checkout; A/A executable provenance is the frozen baseline record",
            "compiler":subprocess.check_output(["rustc","-Vv"],text=True),"host":platform.platform(),
            "build":f"cargo build --manifest-path experiments/field-regressions/Cargo.toml --bin {binary.name} --release --offline --features arithmetic-campaign; experiment release profile",
            "source_hashes":{str(p.relative_to(ROOT)):sha(p) for base in [ROOT/"vendor/field/src",ROOT/"experiments/field-regressions/src"] for p in sorted(base.rglob("*.rs"))},
            "production_hashes":{p:sha(ROOT/p) for p in ["src/piop/spartan/raw_monty.rs","src/utils/delayed_reduction.rs"]}}
    # Preserve source content, not just hashes, before later implementation work
    # changes the dirty checkout. The frozen executable can be rerun directly.
    paths=set()
    for base in [ROOT/"src",ROOT/"crates/circuit",ROOT/"vendor/field",ROOT/"vendor/flock-mod",ROOT/"experiments/field-regressions/reference/6271724d",ROOT/"experiments/field-regressions/src"]:
        for directory,dirs,files in os.walk(base):
            dirs[:]=[d for d in dirs if d not in ["target",".git","results","baselines"]]
            for name in files:
                p=Path(directory)/name
                if p.suffix in [".rs",".toml",".lock"] or name.startswith("LICENSE"):
                    paths.add(p)
    paths.update(ROOT/p for p in ["Cargo.toml","Cargo.lock","experiments/field-regressions/Cargo.toml","experiments/field-regressions/Cargo.lock","experiments/field-regressions/build.rs","experiments/field-regressions/unified_api.py","experiments/field-regressions/analysis.py","experiments/field-regressions/required_cases.json"])
    with tarfile.open(out/"source.tar.gz","w:gz") as archive:
        for path in sorted(paths):
            if path.is_file():archive.add(path,arcname=str(path.relative_to(ROOT)))
    shutil.copy2(binary,out/"field-regressions")
    policy["source_archive_sha256"]=sha(out/"source.tar.gz")
    (out/"policy.json").write_text(json.dumps(policy,indent=2)+"\n")
    env=os.environ.copy()
    # Do not inherit a different case/baseline selection from an earlier campaign.
    for key in list(env):
        if key.startswith("FIELD_REGRESSION_"):
            del env[key]
    env.update(FIELD_REGRESSION_CALIBRATION_MS=str(args.calibration_ms),RAYON_NUM_THREADS="1",FIELD_REGRESSION_CASES=",".join(keys),
               FIELD_REGRESSION_VARIANTS=json.dumps({key:["production",variant] for key in keys}))
    records={}
    for run in range(runs):
        with (out/f"run-{run}.csv").open("w") as stdout, (out/f"run-{run}.log").open("w") as stderr:
            subprocess.run([str(binary),"unified",str(samples),str(policy["seed_offset"]+7919*run)],env=env,stdout=stdout,stderr=stderr,check=True)
        assert "CORRECTNESS_COMPLETE" in (out/f"run-{run}.log").read_text()
        for row in csv.DictReader((out/f"run-{run}.csv").open()):
            key=(row["family"],row["size"],row["variant"])
            rep=(run,int(row["rep"]))
            assert rep not in records.setdefault(key,{})
            records[key][rep]=row
        print(f"finished process {run+1}/{runs}",flush=True)
    results=[]
    required={(r,i) for r in range(runs) for i in range(samples)}
    for family in FAMILIES:
        for size in SIZES:
            candidate=records[(family,size,variant)]
            baseline=records[(family,size,"production")]
            assert set(candidate)==set(baseline)==required
            values=lambda rows:[[float(rows[(r,i)]["ns"]) for i in range(samples)] for r in range(runs)]
            stats=paired_stats(values(candidate),values(baseline))
            row={"family":family,"size":size,"variant":variant,"baseline":"production","complete":True,
                 "correctness":all(x["correctness"]=="1" for x in candidate.values()),
                 "allocations_ok":all(int(candidate[k][metric])<=int(baseline[k][metric]) for k in required for metric in ["allocations","allocated_bytes"]),**stats}
            row["status"]=classify(row,0.01)
            results.append(row)
    (out/"results.json").write_text(json.dumps(results,indent=2)+"\n")
    for row in results:
        print(f"{row['family']}/{row['size']}: {row['status']}; median={row['median_ratio']:.4f}, p95={row['p95_ratio']:.4f}")

if __name__=="__main__":
    main()
