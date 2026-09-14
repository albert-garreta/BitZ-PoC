#!/usr/bin/env python3
"""Freeze sources and selections, build once, then measure fresh paired processes."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import shlex
import tarfile
import gzip
import host
import x86_manifest
import subprocess
import tempfile
import time

from analysis import DEFAULT_MARGIN, SPEC_PATH, expanded, summarize_round, write_report

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def save(path, value):
    Path(path).write_text(json.dumps(value, indent=2) + "\n")


def planned_seeds(offset, runs, retry_enabled):
    offsets=[offset,offset+104687] if retry_enabled else [offset]
    return {start+7919*r for start in offsets for r in range(runs)}


def candidate_snapshot(work):
    """Change scalar multiplication only in a private copy of Flock.

    Keeping all NTT sources byte-identical retains fused and cache-blocked
    kernels, including specialized multiplication they already use internally.
    """
    experiment = work / "experiments/field-regressions"
    baseline = work / "vendor/flock-mod"
    candidate = work / "vendor/flock-candidate"
    shutil.copytree(baseline, candidate)
    manifest = candidate / "Cargo.toml"
    manifest.write_text(manifest.read_text().replace(
        'members = ["crates/flock-core", "crates/flock-prover"]',
        'members = ["crates/flock-core"]'))
    core = candidate / "crates/flock-core"
    manifest = core / "Cargo.toml"
    manifest.write_text(manifest.read_text().replace('name = "flock-core"', 'name = "flock-core-candidate"\nworkspace = "../.."', 1).replace(
        '[dependencies]', '[dependencies]\nfield = { path = ' + json.dumps(str(work / 'crates/field')) + ' }', 1))
    source = core / "src/field/gf2_128.rs"
    content = source.read_text()
    begin = content.index("impl Mul for F128 {")
    end = content.index("\nimpl MulAssign", begin)
    content = content[:begin] + '''impl Mul for F128 {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: Self) -> Self {
        #[cfg(field_regression_probe)]
        crate::EXPERIMENT_SCALAR_CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let value = crate::experiment_scalar_kernel::mul(
            field::F128::new(self.lo, self.hi), field::F128::new(rhs.lo, rhs.hi));
        Self::new(value.lo, value.hi)
    }
}
''' + content[end:]
    source.write_text(content)
    shutil.copy2(experiment / "src/scalar_kernel.rs", core / "src/experiment_scalar_kernel.rs")
    library = core / "src/lib.rs"
    library.write_text(library.read_text() + "\nmod experiment_scalar_kernel;\n" +
        "#[cfg(field_regression_probe)]\npub static EXPERIMENT_SCALAR_CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);\n")
    unchanged = {}
    for path in (baseline / "crates/flock-core/src").rglob("*.rs"):
        relative = path.relative_to(baseline / "crates/flock-core")
        if "ntt" in relative.parts or path.name == "ntt.rs":
            assert sha(path) == sha(core / relative), relative
            unchanged[str(relative)] = sha(path)
    assert unchanged, "NTT sources not found"
    manifest = experiment / "Cargo.toml"
    manifest.write_text(manifest.read_text().replace('[dependencies]', '''[dependencies]
flock-candidate = { package = "flock-core-candidate", path = "../../vendor/flock-candidate/crates/flock-core", optional = true }''', 1).replace(
        'ntt-candidate = []', 'ntt-candidate = ["dep:flock-candidate"]'))
    generated = {str(p.relative_to(work)): sha(p) for p in candidate.rglob("*") if p.is_file()}
    generated[str(manifest.relative_to(work))] = sha(manifest)
    return unchanged, generated


def clean_environment(flags, target, threads):
    env = os.environ.copy()
    for key in list(env):
        if key.startswith("CARGO_PROFILE_") or key in {
                "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER",
                "NTT_DEEP_NOFUSE", "NTT_DEEP_PCORES_ONLY", "FIELD_REGRESSION_CASES", "F2Z_FIXED_SCALAR", "FIELD_REGRESSION_BASELINES", "FIELD_REGRESSION_VARIANTS", "FIELD_REGRESSION_CALIBRATION_MS", "FIELD_REGRESSION_STREAM_N"}:
            env.pop(key)
    env.update(RUSTFLAGS=flags, CARGO_TARGET_DIR=str(target), CARGO_BUILD_JOBS="2", RAYON_NUM_THREADS=str(threads))
    return env


def run_round(out, metadata, spec, binary, env, cases=None):
    out.mkdir()
    save(out / "metadata.json", metadata)
    run_env = env.copy()
    run_env["FIELD_REGRESSION_BASELINES"]=json.dumps({g["name"]:g["baseline"] for g in spec["families"]})
    if spec.get("campaign") == "x86":
        run_env["FIELD_REGRESSION_VARIANTS"] = json.dumps({
            g["name"]+"/"+size:g["variants"] for g in spec["families"] for size in g["sizes"]})
        run_env["FIELD_REGRESSION_CALIBRATION_MS"] = str(metadata["calibration_ms"])
        if metadata.get("stream_n"):
            run_env["FIELD_REGRESSION_STREAM_N"] = str(metadata["stream_n"])
    if cases:
        run_env["FIELD_REGRESSION_CASES"] = ",".join(sorted(cases))
    for run in range(metadata["runs"]):
        assert sha(binary) == metadata["binary_sha256"], "Frozen binary changed"
        print(f'{out.name}: process {run+1}/{metadata["runs"]}, {metadata["samples"]} paired samples', flush=True)
        with (out / f"run-{run}.csv").open("w") as data, (out / f"run-{run}.log").open("w") as log:
            command = [str(binary), metadata.get("mode","gf"), str(metadata["samples"]), str(metadata["seed_offset"] + 7919*run)]
            if metadata.get("cpu_set"):
                command = ["taskset", "--cpu-list", metadata["cpu_set"], *command]
            before = host.snapshot(host.cpu_set(metadata["cpu_set"])) if metadata.get("cpu_set") else None
            subprocess.run(command, env=run_env, stdout=data, stderr=log, check=True)
            if before:
                save(out / f"host-{run}.json", dict(before=before, after=host.snapshot(host.cpu_set(metadata["cpu_set"]))))
    print(f"Analyzing {out.name} confidence intervals", flush=True)
    rows, runtimes = summarize_round(out, metadata, spec)
    metadata = dict(metadata, runtimes=runtimes, status="complete",
                    artifacts_sha256={p.name:sha(p) for p in sorted(out.glob("run-*.*"))})
    save(out / "metadata.json", metadata)
    write_report(out, rows, metadata["margin"], metadata.get("report_title","GF128 and NTT regression measurements"))
    return rows


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--suite", choices=["gf-ntt","arithmetic","integer-focus","operation-metrics","optimization","x86"], default="gf-ntt")
    parser.add_argument("--arch", choices=["aarch64","x86_64"], help="Host campaign scope; other architecture is a separate run")
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--runs", type=int, default=5)
    parser.add_argument("--samples", type=int, default=32)
    parser.add_argument("--seed-offset", type=int, default=42)
    parser.add_argument("--margin", type=float, default=DEFAULT_MARGIN)
    parser.add_argument("--threads", type=int, default=1)
    parser.add_argument("--flags", default="-C target-cpu=native")
    parser.add_argument("--target-dir", type=Path, help="Optional experiment-owned build cache")
    parser.add_argument("--manifest", type=Path, help="Explicit frozen case/selection manifest for a new revision")
    parser.add_argument("--cases", help="Comma-separated family/size IDs; omitted cases remain unmeasured")
    parser.add_argument("--no-retry", action="store_true", help="Development run: disable the one inconclusive-case retry")
    parser.add_argument("--cpu-set", help="Linux worker affinity, e.g. 0, 8, or 0-15")
    parser.add_argument("--scope", choices=["all","micro","consumers","regressions","regressions-consumers"], default="all")
    parser.add_argument("--phase", choices=["explore","confirm"], default="explore")
    parser.add_argument("--selection", type=Path, help="Frozen selection.json emitted by rank.py")
    parser.add_argument("--streaming", action="store_true", help="Add active buffers exceeding twice this CPU group's L3")
    parser.add_argument("--calibration-ms", type=float, default=20.0)
    parser.add_argument("--probe-ntt", action="store_true", help="Separate untimed instrumented build recording actual scalar substitution calls")
    args = parser.parse_args()
    if args.probe_ntt:
        if args.suite != "x86" or args.phase != "explore":
            parser.error("--probe-ntt is an x86 exploration diagnostic")
        args.flags += " --cfg field_regression_probe"
        args.scope = "consumers"
        args.runs = args.samples = 1
        args.no_retry = True
    spec_path = {
        "gf-ntt": SPEC_PATH,
        "arithmetic": HERE / "arithmetic_cases.json",
        "integer-focus": HERE / "integer_focus_cases.json",
        "operation-metrics": HERE / "operation_metrics_cases.json",
        "optimization": HERE / "optimization_cases.json",
        "x86": HERE / "arithmetic_cases.json",
    }[args.suite]
    if args.manifest:
        if args.suite == "x86":
            parser.error("x86 generates its host manifest; use --selection for frozen choices")
        spec_path = args.manifest.resolve()
    spec = json.loads(spec_path.read_text())
    host_info = None
    stream_n = None
    selection = None
    if args.suite == "x86":
        if not args.cpu_set:
            parser.error("x86 measurements require an explicit --cpu-set")
        try:
            cpus = host.cpu_set(args.cpu_set)
            host_info = host.snapshot(cpus)
            if args.threads > len(cpus):
                raise ValueError("thread count exceeds worker affinity")
            cfg = subprocess.check_output(["rustc", "--print", "cfg", *shlex.split(args.flags)], text=True)
            features = [line.split('"')[1] for line in cfg.splitlines() if line.startswith('target_feature=')]
            stream_n = host.streaming_terms(host_info) if args.streaming else None
            spec = x86_manifest.build(features, stream_n, args.scope)
            available = {f.replace('sse4_1','sse4.1') for f in host_info['runtime_features']}
            if not set(spec['required_features']['x86_64']) <= available:
                raise ValueError('requested instruction profile is not supported by this host')
            if args.phase == "confirm":
                if not args.selection:
                    raise ValueError("confirmation requires a frozen --selection from exploration")
                selection = json.loads(args.selection.read_text())
                if selection["host_model"] != host_info["model"] or selection["cpu_set"] != args.cpu_set:
                    raise ValueError("selection was made on a different host/core group")
                if selection["flags"] != args.flags or selection["threads"] != args.threads:
                    raise ValueError("selection build/thread configuration differs")
                if planned_seeds(args.seed_offset,args.runs,not args.no_retry) & set(selection["used_seeds"]):
                    raise ValueError("confirmation needs a fresh seed offset")
                spec = x86_manifest.selections(spec, selection["choices"])
            elif args.selection:
                raise ValueError("--selection is only valid for confirmation")
            if not 0 < args.calibration_ms <= 1000:
                raise ValueError("calibration window must be in (0,1000] ms")
        except (ValueError, KeyError) as error:
            parser.error(str(error))
    arch = {"arm64": "aarch64", "AMD64": "x86_64"}.get(platform.machine(), platform.machine())
    if args.arch and arch!=args.arch:
        parser.error("Requested architecture does not match this host")
    if arch not in spec["architectures"]:
        parser.error(f"{arch} is outside this campaign's explicit architecture scope")
    if args.suite!="gf-ntt":
        spec["architectures"]=[arch]
    if min(args.runs, args.samples, args.threads) < 1 or not 0 <= args.margin < 1:
        parser.error("positive run/sample/thread counts and a margin in [0, 1) are required")
    if not 0 <= args.seed_offset < 2**63:
        parser.error("seed offset must fit a nonnegative 63-bit integer")
    if args.probe_ntt:
        spec['families'] = [g for g in spec['families'] if g['name'] == 'ntt']
        spec['probe_ntt'] = True
    cases = set(args.cases.split(",")) if args.cases else None
    valid_cases = {r["family"]+"/"+r["size"] for r in expanded(spec)}
    if cases and not cases <= valid_cases:
        parser.error(f"Unknown cases: {cases-valid_cases}")
    lease = host.exclusive() if args.suite == "x86" else None
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    work = Path(tempfile.mkdtemp(prefix="f2z-field-gated-"))
    print(f"Frozen snapshot: {work}", flush=True)
    hashes = {}
    for relative in ["src", "benches", "tests", "examples", "crates/field", "crates/circuit", "vendor/crypto-primitives", "vendor/flock-mod", "experiments/field-regressions"]:
        dest = work / relative
        shutil.copytree(ROOT / relative, dest, ignore=shutil.ignore_patterns(
            "target", ".git", "results", "__pycache__", "candidate-flock"))
        hashes.update({str(p.relative_to(work)): sha(p) for p in dest.rglob("*") if p.is_file()})
    for relative in ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml"]:
        shutil.copy2(ROOT / relative, work / relative)
        hashes[relative]=sha(work/relative)
    if selection and selection["source_sha256"] != hashes:
        parser.error("sources changed since exploration; repeat exploration before confirmation")
    if args.suite not in ("gf-ntt", "x86"):
        ntt_hashes,generated_hashes={},{}
    else:
        ntt_hashes,generated_hashes=candidate_snapshot(work)
    save(out/"required_cases.json",spec)
    target = args.target_dir.resolve() if args.target_dir else work / "target"
    env = clean_environment(args.flags, target, args.threads)
    hardware = subprocess.run(["sysctl", "machdep.cpu.brand_string", "hw.memsize", "hw.physicalcpu", "hw.logicalcpu"]
                              if platform.system()=="Darwin" else ["lscpu"], capture_output=True, text=True)
    metadata = dict(hardware=hardware.stdout, hardware_error=hardware.stderr, schema=2, mode=args.suite if args.suite!="gf-ntt" else "gf",
                    report_title="Unified arithmetic benchmark measurements" if args.suite!="gf-ntt" else "GF128 and NTT regression measurements", timestamp=time.strftime("%Y-%m-%dT%H:%M:%S%z"), arch=arch,
                    platform=platform.platform(), rustc=subprocess.check_output(["rustc", "-Vv"], text=True),
                    flags=args.flags, threads=args.threads, runs=args.runs, samples=args.samples,
                    margin=args.margin, seed_offset=args.seed_offset, source_sha256=hashes, generated_sha256=generated_hashes,
                    unchanged_ntt_sha256=ntt_hashes, manifest_sha256=sha(out/"required_cases.json"), snapshot=str(work),
                    cases=sorted(cases) if cases else None, retry_enabled=not args.no_retry, status="building",
                    note="Host-specific campaign. Core placement, thermal state and unrelated system load are uncontrolled. No production integration.")
    if args.suite == "x86":
        metadata.update(phase=args.phase, cpu_set=args.cpu_set, host=host_info,
                        stream_n=stream_n, calibration_ms=args.calibration_ms,
                        scope=args.scope, probe_ntt=args.probe_ntt, report_title="Ryzen x86 arithmetic measurements",
                        note="Pinned worker affinity; machine boost/governor unchanged. Host observations accompany each process.")
        if selection:
            save(out / "selection.json", selection)
            metadata["selection_sha256"] = sha(out / "selection.json")
    save(out / "metadata.json", metadata)
    manifest = work / "experiments/field-regressions/Cargo.toml"
    build_start=time.monotonic()
    with (out / "build.log").open("w") as log:
        subprocess.run(["cargo", "build", "--offline", "--release", "--features", "arithmetic-campaign,ntt-candidate" if args.suite=="x86" else "arithmetic-campaign" if args.suite!="gf-ntt" else "ntt-candidate", "--manifest-path", str(manifest)],
                       env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
    shutil.copy2(manifest.with_name("Cargo.lock"), out / "Cargo.lock")
    metadata["lockfile_sha256"] = sha(out / "Cargo.lock")
    # The private candidate adds its package to the experiment lock. Record
    # the built lock separately from the unmodified exploration source hashes.
    generated_hashes[str(manifest.with_name("Cargo.lock").relative_to(work))] = sha(out / "Cargo.lock")
    binary = work / "field-regressions"
    shutil.copy2(target / "release/field-regressions", binary)
    metadata.update(build_seconds=time.monotonic()-build_start, binary_sha256=sha(binary), binary=str(binary), status="confirming")
    save(out / "metadata.json", metadata)
    if args.suite == "x86":
        with tarfile.open(out / "sources.tar.gz", "w:gz") as archive:
            for relative in sorted(set(hashes) | set(generated_hashes)):
                archive.add(work / relative, arcname=relative, recursive=False)
        metadata["sources_archive_sha256"] = sha(out / "sources.tar.gz")
        if shutil.which("objdump"):
            with tempfile.TemporaryFile() as asm:
                result = subprocess.run(["objdump", "-Cd", str(binary)], stdout=asm, stderr=subprocess.PIPE)
                if result.returncode == 0:
                    asm.seek(0)
                    with gzip.open(out / "assembly.txt.gz", "wb") as dest:
                        shutil.copyfileobj(asm, dest)
                    metadata["assembly_sha256"] = sha(out / "assembly.txt.gz")
    # Keep the executable with the results, not only in an ephemeral /tmp snapshot.
    shutil.copy2(binary, out / "benchmark.bin")
    metadata["archived_binary_sha256"] = sha(out / "benchmark.bin")
    save(out / "metadata.json", metadata)
    rows = run_round(out / "initial", metadata, spec, binary, env, cases or (valid_cases if args.suite!="gf-ntt" else None))
    retry = {r["family"]+"/"+r["size"] for r in rows if r["arch"]==arch and r["selected"] and r["status"]=="inconclusive"}
    if args.suite == "x86" and not args.no_retry:
        import rank
        retry |= rank.unresolved_challengers(out, rows, metadata, spec)
    if retry and not args.no_retry:
        retry_meta = dict(metadata, samples=args.samples*spec["retry_multiplier"], seed_offset=args.seed_offset+104687, cases=sorted(retry))
        retried = run_round(out / "retry", retry_meta, spec, binary, env, retry)
        replacements = {(r["arch"], r["family"], r["size"], r["variant"]): r for r in retried
                        if r["arch"]==arch and r["family"]+"/"+r["size"] in retry}
        rows = [replacements.get((r["arch"], r["family"], r["size"], r["variant"]), r) for r in rows]
    else:
        retry = set()
    write_report(out, rows, args.margin, metadata["report_title"])
    if args.suite == "x86":
        import rank
        rank.report(out, rows, metadata, spec)
        metadata["rankings_sha256"] = sha(out / "rankings.json")
        if args.probe_ntt:
            events=[]
            for path in sorted((out / 'initial').glob('run-*.log')):
                events.extend(json.loads(line[len('ACTIVATION '):]) for line in path.read_text().splitlines() if line.startswith('ACTIVATION '))
            save(out / 'activation.json', events)
    metadata.update(status="complete", retried_cases=sorted(retry),
                    round_metadata_sha256={name:sha(out/name/"metadata.json") for name in ["initial","retry"] if (out/name).exists()},
                    summary_sha256=sha(out/"summary.json"))
    save(out / "metadata.json", metadata)
    print(f"Report: {out / 'measurements.md'}", flush=True)


if __name__ == "__main__":
    main()
