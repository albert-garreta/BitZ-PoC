#!/usr/bin/env python3
"""Compare signed SHA-256 chains in fresh processes; retain failures and full samples."""
import argparse
import csv
import hashlib
import itertools
import json
import math
import os
from pathlib import Path
import platform
import resource
import shutil
import signal
import statistics
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
SCHEMA = "f2z/sha256-ecdsa-compare/v1"
DEFAULT_METHODS = ["f2z-split", "f2z-all", "spartan-mc"]
METHODS = [*DEFAULT_METHODS, "zkpassport-honk"]
METRICS = ["setup_ms", "witness_ms", "commit_ms", "protocol_ms", "prove_ms",
           "witness_to_proof_ms", "verify_ms", "codec_ms", "outer_ms", "inner_ms",
           "opening_ms", "folding_ms", "piop_ms", "iop_ms", "proof_object_bytes", "proof_material_bytes"]


def sample_metrics(row):
    """Derive disjoint protocol stages from the original measured sample.

    PIOP includes projection, preparation, folding, sumchecks and transcript
    work outside the timed opening. IOP/PCS is the complete opening path.
    Keep the raw worker records unchanged; derive before taking medians.
    """
    row = dict(row)
    protocol, opening = row.get("protocol_ms"), row.get("opening_ms")
    valid = (type(protocol) in (int, float) and type(opening) in (int, float)
             and math.isfinite(protocol) and math.isfinite(opening) and 0 <= opening <= protocol)
    row["piop_ms"] = protocol - opening if valid else None
    row["iop_ms"] = opening if valid else None
    return row


def cases(spartan_splits, methods, targets, threads, seeds):
    """F2Z depends only on total work; emit it once for each exponent/target."""
    work = sorted({r + c for r, c in spartan_splits})
    for method, workers, seed in itertools.product(methods, threads, seeds):
        if method == "spartan-mc":
            for r, c in sorted(set(spartan_splits)):
                yield dict(method=method, log_compressions=r+c, r=r, c=c,
                           security_target=None, threads=workers, seed=seed)
        elif method == "zkpassport-honk":
            for exponent in work:
                yield dict(method=method, log_compressions=exponent, r=None, c=None,
                           security_target=None, threads=workers, seed=seed)
        else:
            for exponent, target in itertools.product(work, targets):
                yield dict(method=method, log_compressions=exponent, r=None, c=None,
                           security_target=target, threads=workers, seed=seed)


def validate_rows(rows, case, reps):
    """Reject incomplete, mislabelled, unverified or fixture-changing workers."""
    if len(rows) != reps + 1:
        return False
    for sample, row in enumerate(rows):
        row = sample_metrics(row)
        if row.get("schema") != SCHEMA or row.get("verified") is not True:
            return False
        if any(row.get(k) != v for k, v in case.items()):
            return False
        if row.get("sample") != sample or row.get("trial") != ("warmup" if sample == 0 else "sample"):
            return False
        if row.get("compressions") != 1 << case["log_compressions"]:
            return False
        if row.get("message_bytes") != 64 * ((1 << case["log_compressions"]) - 1):
            return False
        if row.get("signatures") != 1 or row.get("statement_bytes") != 129:
            return False
        if not isinstance(row.get("fixture_id"), str) or len(row["fixture_id"]) != 64:
            return False
        if not isinstance(row.get("security"), dict) or "model" not in row["security"]:
            return False
        honk = case["method"] == "zkpassport-honk"
        if honk and (row.get("zk") is not False or row.get("barretenberg_version") != "5.0.0"):
            return False
        revision = row.get("zkpassport_revision" if honk else "spartan_revision")
        if not isinstance(revision, str) or len(revision) != 40 or any(c not in "0123456789abcdef" for c in revision):
            return False
        for key in METRICS:
            value = row.get(key)
            optional = ["folding_ms", "outer_ms", "inner_ms", "opening_ms"]
            if honk:
                optional += ["commit_ms", "protocol_ms", "piop_ms", "iop_ms"]
            if value is None and key in optional:
                continue
            if type(value) not in (int, float) or not math.isfinite(value) or value < 0:
                return False
        if not honk and not math.isclose(row["prove_ms"], row["commit_ms"] + row["protocol_ms"], abs_tol=1e-6):
            return False
        if not math.isclose(row["witness_to_proof_ms"], row["witness_ms"] + row["prove_ms"], abs_tol=1e-6):
            return False
    return len({row["fixture_id"] for row in rows}) == 1


def summarize(directory):
    paths = sorted(directory.glob("*.result.json"))
    results = [json.loads(p.read_text()) for p in paths]
    fields = ["method", "log_compressions", "r", "c", "security_target", "threads", "seed",
              "status", "samples", "peak_rss_bytes", "fixture_id", "security_model",
              "economic_bits", "statistical_bits_lower_bound", *METRICS]
    fixture_ids = {}
    sample_fields = [*fields[:8], "source_file", "trial", "sample", "verified", "peak_rss_bytes",
                     "fixture_id", "spartan_revision", "zkpassport_revision", "artifact_id", "security_model", "economic_bits",
                     "statistical_bits_lower_bound", *METRICS]
    with (directory / "summary.csv").open("w", newline="") as stream, \
            (directory / "samples.csv").open("w", newline="") as sample_stream:
        writer = csv.DictWriter(stream, fieldnames=fields)
        writer.writeheader()
        sample_writer = csv.DictWriter(sample_stream, fieldnames=sample_fields)
        sample_writer.writeheader()
        for path, result in zip(paths, results):
            row = {k: result["case"][k] for k in fields[:7]}
            row.update(status=result["status"], peak_rss_bytes=result["peak_rss_bytes"])
            normalized = [sample_metrics(s) for s in result["rows"]]
            for sample in normalized:
                record = dict(row, source_file=path.name)
                record.update({k: sample.get(k) for k in ["trial", "sample", "verified", "fixture_id",
                                                         "spartan_revision", "zkpassport_revision", "artifact_id", *METRICS]})
                security = sample.get("security")
                if isinstance(security, dict):
                    record.update(security_model=security.get("model"), economic_bits=security.get("economic_bits"),
                                  statistical_bits_lower_bound=security.get("statistical_bits_lower_bound"))
                sample_writer.writerow(record)
            samples = [s for s in normalized if s.get("trial") == "sample"]
            row["samples"] = len(samples)
            if samples:
                row["fixture_id"] = samples[0].get("fixture_id")
                security = samples[0].get("security", {})
                if not isinstance(security, dict):
                    security = {}
                row.update(security_model=security.get("model"), economic_bits=security.get("economic_bits"),
                           statistical_bits_lower_bound=security.get("statistical_bits_lower_bound"))
                for key in METRICS:
                    values = [s[key] for s in samples if type(s.get(key)) in (int, float) and math.isfinite(s[key])]
                    row[key] = statistics.median(values) if values else None
            if result["status"] == "complete":
                key = (row["log_compressions"], row["seed"])
                fixture_ids.setdefault(key, set()).add(row["fixture_id"])
            writer.writerow(row)
    mismatches = [list(key) for key, ids in fixture_ids.items() if len(ids) != 1]
    (directory / "comparison.json").write_text(json.dumps({
        "all_recorded_cases_complete": all(r["status"] == "complete" for r in results),
        "matched_fixtures": not mismatches, "mismatched_exponent_seed_pairs": mismatches,
        "security_note": "F2Z economic targets, Spartan group security, and BN254 KZG use different accounting; see each row.",
        "proof_size_note": "proof_material_bytes includes commitments and auxiliary inputs; statement bytes are separate.",
        "timing_note": "Per sample: IOP/PCS = opening_ms; PIOP/preparation = protocol_ms - opening_ms. Barretenberg exposes only aggregate prove_ms; unavailable components remain null.",
        "memory_note": "peak_rss_bytes is the whole worker maximum, including setup and all trials; repeated in samples.csv, not measured per proof.",
    }, indent=2) + "\n")
    return not mismatches


def build(args, directory):
    command = ["cargo", "build", "--release", "--locked", "--features", "sha256-ecdsa-compare",
               "--bench", "sha256_ecdsa_compare", "--message-format=json-render-diagnostics"]
    if args.offline:
        command.append("--offline")
    env = dict(os.environ)
    env.setdefault("RUSTFLAGS", "-C target-cpu=native")
    result = subprocess.run(command, cwd=ROOT, env=env, text=True, capture_output=True)
    (directory / "build.jsonl").write_text(result.stdout)
    (directory / "build.log").write_text(result.stderr)
    if result.returncode:
        raise RuntimeError(f"build failed; see {directory / 'build.log'}")
    for line in reversed(result.stdout.splitlines()):
        try:
            artifact = json.loads(line)
        except ValueError:
            continue
        if artifact.get("target", {}).get("name") == "sha256_ecdsa_compare" and artifact.get("executable"):
            return Path(artifact["executable"]).resolve()
    raise RuntimeError("Cargo did not return the benchmark executable")


def metadata(binary):
    def output(command):
        return subprocess.run(command, cwd=ROOT, text=True, capture_output=True, check=True).stdout.strip()
    cpu = platform.processor() or platform.machine()
    if Path("/proc/cpuinfo").exists():
        cpu = next((s.partition(":")[2].strip() for s in Path("/proc/cpuinfo").read_text().splitlines()
                    if s.startswith("model name")), cpu)
    diff = subprocess.run(["git", "diff", "HEAD"], cwd=ROOT, capture_output=True, check=True).stdout
    fingerprint = (binary.parent.parent / ".fingerprint" /
                   binary.name.replace("sha256_ecdsa_compare-", "f2z-", 1) /
                   "test-bench-sha256_ecdsa_compare.json")
    build_info = json.loads(fingerprint.read_text()) if fingerprint.exists() else None
    return dict(binary=str(binary), binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                revision=output(["git", "rev-parse", "HEAD"]), git_status=output(["git", "status", "--short"]),
                tracked_diff_sha256=hashlib.sha256(diff).hexdigest(), rustc=output(["rustc", "-Vv"]),
                build=build_info, cpu=cpu, platform=platform.platform(), logical_cpus=os.cpu_count(),
                runtime_env={k: v for k, v in os.environ.items() if k.startswith("F2Z_") or
                             k in ["RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RAYON_NUM_THREADS", "CARGO_TARGET_DIR"]})


def run_case(binary, case, args, directory):
    shape = f"r{case['r']}-c{case['c']}" if case["r"] is not None else "total"
    name = (f"{case['method']}-i{case['log_compressions']}-{shape}-s{case['security_target']}"
            f"-t{case['threads']}-seed{case['seed']}-reps{args.reps}")
    path = directory / f"{name}.result.json"
    if path.exists():
        existing = json.loads(path.read_text())
        if existing["status"] == "complete" or not args.retry_failed:
            print(f"skip {name}: {existing['status']}", flush=True)
            return existing["status"] == "complete"
    r = case["r"] if case["r"] is not None else case["log_compressions"]
    c = case["c"] if case["c"] is not None else 0
    worker = args.zkpassport_worker if case["method"] == "zkpassport-honk" else binary
    command = [str(worker), "--method", case["method"], "--r", str(r), "--c", str(c),
               "--threads", str(case["threads"]), "--reps", str(args.reps), "--seed", str(case["seed"])]
    if case["security_target"] is not None:
        command.extend(["--target", str(case["security_target"])])
    expected_fixture = None
    if getattr(args, "with_zkpassport", False):
        fixture = directory / "fixtures" / f"i{case['log_compressions']}-seed{case['seed']}.json"
        expected_fixture = json.loads(fixture.read_text())["id"]
        command.extend(["--fixture", str(fixture)])
    if case["method"] == "zkpassport-honk":
        exponent = case["log_compressions"]
        if exponent in args.zkpassport_errors:
            result = dict(case=case, status="preparation_failed", returncode=None, rows=[],
                          peak_rss_bytes=None, error=args.zkpassport_errors[exponent], elapsed_seconds=0)
            path.write_text(json.dumps(result, indent=2) + "\n")
            print(f"{name}: preparation_failed", flush=True)
            return False
        command.extend(["--artifact", str(args.zkpassport_artifacts[exponent]),
                        "--srs-cache", str(ROOT / "benchmarks/zkpassport/.cache/srs"),
                        "--revision", args.zkpassport_revision])
        if args.offline:
            command.append("--offline")
    rss_path = directory / f"{name}.rss-kib"
    if Path("/usr/bin/time").exists() and sys.platform.startswith("linux"):
        command = ["/usr/bin/time", "-f", "%M", "-o", str(rss_path), *command]

    def memory_limit():
        limit = args.memory_gib * 1024**3
        resource.setrlimit(resource.RLIMIT_AS, (limit, limit))

    started = time.monotonic()
    status, returncode = "failed", None
    with (directory / f"{name}.stdout").open("w") as stdout, (directory / f"{name}.stderr").open("w") as stderr:
        try:
            process = subprocess.Popen(command, stdout=stdout, stderr=stderr, cwd=ROOT,
                                       start_new_session=True, preexec_fn=memory_limit,
                                       env=dict(os.environ, RAYON_NUM_THREADS=str(case["threads"]),
                                                HARDWARE_CONCURRENCY=str(case["threads"])))
        except OSError as exc:
            stderr.write(str(exc) + "\n")
        else:
            try:
                returncode = process.wait(timeout=args.timeout)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()
                status = "timeout"
    rows = []
    for line in (directory / f"{name}.stdout").read_text().splitlines():
        try:
            row = json.loads(line)
        except ValueError:
            continue
        if isinstance(row, dict) and row.get("schema") == SCHEMA:
            rows.append(row)
    if (returncode == 0 and validate_rows(rows, case, args.reps)
            and (expected_fixture is None or all(row["fixture_id"] == expected_fixture for row in rows))):
        status = "complete"
    rss = None
    if rss_path.exists():
        lines = rss_path.read_text().splitlines()
        if lines and lines[-1].isdigit():
            rss = int(lines[-1]) * 1024
    result = dict(case=case, status=status, returncode=returncode, rows=rows,
                  peak_rss_bytes=rss, elapsed_seconds=time.monotonic()-started)
    temporary = path.with_suffix(".tmp")
    temporary.write_text(json.dumps(result, indent=2)+"\n")
    temporary.replace(path)
    summarize(directory)
    print(f"{name}: {status} ({result['elapsed_seconds']:.1f}s)", flush=True)
    return status == "complete"


def file_hash(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def noir_source_hash(checkout):
    paths = subprocess.check_output(
        ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"], cwd=checkout
    ).decode().split("\0")
    files = {p: file_hash(checkout / p) for p in paths if p and
             (p.endswith(".nr") or p.endswith("Nargo.toml") or p.startswith("benchmarks/sha256-ecdsa/"))}
    return hashlib.sha256(json.dumps(files, sort_keys=True).encode()).hexdigest()


def compile_noir(args, directory, exponent):
    log = directory / f"noir-compile-i{exponent}.log"
    command = [sys.executable, str(args.zkpassport_checkout / "benchmarks/sha256-ecdsa/compile.py"),
               "--nargo", str(args.nargo), "--exponents", str(exponent)]
    if args.offline:
        command.append("--offline")
    def limit():
        size = args.memory_gib * 1024**3
        resource.setrlimit(resource.RLIMIT_AS, (size, size))
    with log.open("w") as output:
        process = subprocess.Popen(command, stdout=output, stderr=output, start_new_session=True,
                                   preexec_fn=limit)
        try:
            code = process.wait(timeout=args.timeout)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait()
            raise RuntimeError(f"Noir compilation timed out; see {log.name}")
    if code:
        raise RuntimeError(f"Noir compilation failed ({code}); see {log.name}")


def prepare_zkpassport(args, directory, binary, splits):
    args.zkpassport_checkout = args.zkpassport_checkout.resolve(strict=True)
    source_hash = noir_source_hash(args.zkpassport_checkout)
    old_manifest = directory / "manifest.json"
    previous = {}
    if old_manifest.exists():
        previous = json.loads(old_manifest.read_text()).get("zkpassport", {})
        if previous.get("source_hash") != source_hash:
            raise RuntimeError("Noir sources changed; use a new output directory")
    args.zkpassport_revision = subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=args.zkpassport_checkout, text=True).strip()
    build_info = {}
    if args.zkpassport_binary:
        args.zkpassport_worker = args.zkpassport_binary.resolve(strict=True)
        cached_build = ROOT / "benchmarks/zkpassport/.cache/build.json"
        if cached_build.exists():
            candidate = json.loads(cached_build.read_text())
            if candidate.get("binary_sha256") == file_hash(args.zkpassport_worker):
                build_info = candidate
    else:
        print("Building pinned native ZKPassport worker...", flush=True)
        command = [sys.executable, str(ROOT / "benchmarks/zkpassport/build.py")]
        if args.offline:
            command.append("--offline")
        with (directory / "zkpassport-build.log").open("w") as log:
            result = subprocess.run(command, stdout=subprocess.PIPE, stderr=log, text=True)
        if result.returncode:
            raise RuntimeError("ZKPassport build failed; see zkpassport-build.log")
        build_info = json.loads(result.stdout.splitlines()[-1])
        args.zkpassport_worker = Path(build_info["binary"])
    args.nargo = (args.nargo or Path(build_info.get("nargo", ROOT / "benchmarks/zkpassport/.cache/tools/nargo/nargo"))).resolve(strict=True)
    input_hash = hashlib.sha256((source_hash + file_hash(args.nargo)).encode()).hexdigest()
    args.zkpassport_artifacts, args.zkpassport_errors = {}, {}
    artifacts = {}
    for exponent in sorted({r+c for r, c in splits}):
        old_artifact = previous.get("artifacts", {}).get(str(exponent))
        if old_artifact == "preparation_failed" and not args.retry_failed:
            args.zkpassport_errors[exponent] = "Previously failed preparation; use --retry-failed to retry"
            artifacts[str(exponent)] = old_artifact
            continue
        cache = args.zkpassport_checkout / "benchmarks/sha256-ecdsa/build" / f"i{exponent}"
        artifact, meta = cache / "circuit.json", cache / "manifest.json"
        try:
            cached = json.loads(meta.read_text()) if meta.exists() else {}
            if (cached.get("input_hash") != input_hash or not artifact.exists()
                    or cached.get("artifact_sha256") != file_hash(artifact)):
                print(f"Compiling Noir: {1 << exponent} compressions...", flush=True)
                compile_noir(args, directory, exponent)
                cached = json.loads(meta.read_text())
                cached["input_hash"] = input_hash
                meta.write_text(json.dumps(cached, indent=2) + "\n")
            saved = directory / "artifacts" / f"i{exponent}"
            saved.mkdir(parents=True, exist_ok=True)
            if old_artifact not in (None, "preparation_failed", file_hash(artifact)):
                raise ValueError("compiled artifact changed; use a new output directory")
            shutil.copyfile(artifact, saved / "circuit.json")
            shutil.copyfile(meta, saved / "manifest.json")
            if (cache / "compile.log").exists():
                shutil.copyfile(cache / "compile.log", saved / "compile.log")
            args.zkpassport_artifacts[exponent] = saved / "circuit.json"
            artifacts[str(exponent)] = file_hash(artifact)
        except (OSError, RuntimeError, ValueError) as error:
            args.zkpassport_errors[exponent] = str(error)
            artifacts[str(exponent)] = "preparation_failed"
            print(f"Noir i={exponent}: {error}", flush=True)
    fixtures = directory / "fixtures"
    fixtures.mkdir(exist_ok=True)
    hashes = {}
    for exponent, seed in itertools.product(sorted({r+c for r, c in splits}), sorted(set(args.seeds))):
        path = fixtures / f"i{exponent}-seed{seed}.json"
        if not path.exists():
            subprocess.run([str(binary), "--method", "f2z-split", "--r", str(exponent), "--c", "0",
                            "--seed", str(seed), "--export-fixture", str(path)], check=True)
        hashes[path.name] = file_hash(path)
    return dict(checkout=str(args.zkpassport_checkout), revision=args.zkpassport_revision,
                source_hash=source_hash, binary_sha256=file_hash(args.zkpassport_worker),
                nargo_sha256=file_hash(args.nargo), artifacts=artifacts, fixtures=hashes,
                native_build=build_info or None, fixture_profile="low-s/v1")


def compatible_zkpassport(previous, current):
    """A failed preparation may succeed on retry; existing artifacts stay fixed."""
    if previous is None or current is None:
        return previous == current
    old, new = dict(previous), dict(current)
    old_artifacts, new_artifacts = old.pop("artifacts"), new.pop("artifacts")
    return (old == new and old_artifacts.keys() == new_artifacts.keys()
            and all(value == "preparation_failed" or value == new_artifacts[key]
                    for key, value in old_artifacts.items()))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--zkpassport-checkout", type=Path, default=ROOT.parent / "zk-passport-circuits")
    parser.add_argument("--zkpassport-binary", type=Path)
    parser.add_argument("--nargo", type=Path)
    shapes = parser.add_mutually_exclusive_group()
    shapes.add_argument("--spartan-splits", nargs="+",
                        help="Spartan r:c pairs: 2^r compressions per instance, 2^c instances; F2Z uses only i=r+c")
    shapes.add_argument("--exponents", nargs="+", type=int,
                        help="Total compression exponents i; sweep every Spartan r+c=i split, F2Z once per i; default 3 5 7")
    parser.add_argument("--methods", nargs="+", choices=METHODS, default=DEFAULT_METHODS)
    parser.add_argument("--targets", nargs="+", type=int, choices=[100, 128], default=[100, 128])
    parser.add_argument("--threads", nargs="+", type=int, default=None)
    parser.add_argument("--seeds", nargs="+", type=int, default=[0])
    parser.add_argument("--reps", type=int, default=3)
    parser.add_argument("--memory-gib", type=int, default=48)
    parser.add_argument("--timeout", type=float, default=3600)
    parser.add_argument("--retry-failed", action="store_true")
    parser.add_argument("--offline", action="store_true")
    parser.add_argument("--summarize-only", action="store_true", help="Regenerate summary.csv and samples.csv from existing raw records")
    args = parser.parse_args()
    args.with_zkpassport = "zkpassport-honk" in args.methods
    if args.threads is None:
        args.threads = [1, 16] if args.with_zkpassport else sorted({1, os.cpu_count() or 1})
    if args.exponents is None and args.spartan_splits is None and args.with_zkpassport:
        args.exponents = list(range(3, 17))
    if args.summarize_only:
        if not args.output.is_dir() or not any(args.output.glob("*.result.json")):
            parser.error("--summarize-only requires an output directory with recorded cases")
        return 0 if summarize(args.output.resolve()) else 1
    if args.exponents and any(i not in range(3, 17) for i in args.exponents):
        parser.error("compression exponents must be in 3..16")
    try:
        spartan_splits = ([tuple(map(int, value.split(":"))) for value in args.spartan_splits] if args.spartan_splits else
                         [(r, i-r) for i in (args.exponents or [3, 5, 7]) for r in range(i+1)])
        if not spartan_splits or any(len(s) != 2 or min(s) < 0 or not 3 <= sum(s) <= 16 for s in spartan_splits):
            raise ValueError()
    except ValueError:
        parser.error("expected Spartan r:c pairs with r,c >= 0 and 3 <= r+c <= 16")
    if (args.reps < 1 or args.memory_gib < 1 or not math.isfinite(args.timeout) or args.timeout <= 0 or min(args.threads) < 1
            or any(not 0 <= seed < 2**64 for seed in args.seeds)):
        parser.error("positive reps/threads/limits and unsigned 64-bit seeds required")
    args.output.mkdir(parents=True, exist_ok=True)
    directory = args.output.resolve()
    binary = args.binary.resolve(strict=True) if args.binary else build(args, directory)
    manifest = metadata(binary)
    if args.with_zkpassport:
        manifest["zkpassport"] = prepare_zkpassport(args, directory, binary, spartan_splits)
    manifest["campaign"] = dict(methods=args.methods, targets=args.targets, threads=args.threads,
                                seeds=args.seeds, reps=args.reps, timeout=args.timeout, memory_gib=args.memory_gib)
    manifest_path = directory / "manifest.json"
    if manifest_path.exists():
        previous = json.loads(manifest_path.read_text())
        if (previous["binary_sha256"] != manifest["binary_sha256"]
                or not compatible_zkpassport(previous.get("zkpassport"), manifest.get("zkpassport"))):
            parser.error("output belongs to different binaries, circuits, or fixtures; choose a new output directory")
        # Preserve the initial machine/build metadata, recording newly prepared artifacts.
        if args.with_zkpassport:
            previous["zkpassport"] = manifest["zkpassport"]
            manifest_path.write_text(json.dumps(previous, indent=2)+"\n")
    else:
        manifest_path.write_text(json.dumps(manifest, indent=2)+"\n")
        patch = subprocess.run(["git", "diff", "--binary", "HEAD"], cwd=ROOT, capture_output=True, check=True).stdout
        (directory / "source.patch").write_bytes(patch)
        # Preserve this task's new source files, which git diff does not include.
        sources = ["benches/sha256_ecdsa_compare.rs", "scripts/run_sha256_ecdsa_compare.py",
                   "benches/support/sha256_ecdsa_fixture.rs"]
        if args.with_zkpassport:
            sources += ["benchmarks/zkpassport/" + path for path in
                        ["Cargo.toml", "Cargo.lock", "build.py", "src/main.rs"]]
            shutil.copytree(args.zkpassport_checkout / "benchmarks/sha256-ecdsa",
                            directory / "source/zkpassport-circuit",
                            ignore=shutil.ignore_patterns("build", "target", "__pycache__"))
        for relative in sources:
            target = directory / "source" / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes((ROOT / relative).read_bytes())
    jobs = list(cases(spartan_splits, args.methods, args.targets, sorted(set(args.threads)), sorted(set(args.seeds))))
    (directory / "requested_cases.json").write_text(json.dumps(jobs, indent=2)+"\n")
    complete = True
    for case in jobs:
        complete = run_case(binary, case, args, directory) and complete
    matched = summarize(directory)
    if not matched:
        print("ERROR: completed methods used different fixtures; see comparison.json", file=sys.stderr)
    return 0 if complete and matched else 1


if __name__ == "__main__":
    raise SystemExit(main())
