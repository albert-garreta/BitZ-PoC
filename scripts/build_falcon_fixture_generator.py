#!/usr/bin/env python3
"""Build a standalone public-fixture generator from canonical benchcommon code.

First build the native release Falcon benchmark with Cargo JSON artifact output.
Then run this tool with --source-root, --cargo-artifacts and a fresh --output
folder. It compiles a small wrapper against those existing rlibs; it does not
edit the source checkout, rebuild protocol code, or run any proof.
"""
from __future__ import annotations
import argparse
import datetime
import hashlib
import json
from pathlib import Path
import subprocess

WRAPPER = '\n/// Fresh generation supplies the reference before any cache file is read.\npub fn reference_and_publish(directory: &Path, degree: usize, count: usize, seed: u64)\n    -> Result<(String, String, usize), Box<dyn Error>>\n{\n    let fresh = generate_cases_uncached(degree, count, seed)?;\n    let reference_digest = blake3::Hash::from(input_digest(&fresh)).to_hex().to_string();\n    let encoded = encode_cache(degree, count, seed, fresh.clone())?;\n    if decode_cache(&encoded, degree, count, seed)? != fresh {\n        return Err("fresh cache round trip changed public inputs".into());\n    }\n    fs::create_dir_all(directory)?;\n    let path = directory.join(format!("fn-dsa-0.3.0-n{degree}-b{count}-seed{seed}.bin"));\n    if !path.exists() {\n        let temporary = path.with_extension(format!("{}.tmp", std::process::id()));\n        let mut file = fs::OpenOptions::new().write(true).create_new(true).open(&temporary)?;\n        file.write_all(&encoded)?;\n        file.sync_all()?;\n        drop(file);\n        let publication = fs::hard_link(&temporary, &path);\n        fs::remove_file(&temporary)?;\n        match publication {\n            Ok(()) => {},\n            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {},\n            Err(error) => return Err(error.into()),\n        }\n    }\n    let stored = fs::read(&path)?;\n    if stored != encoded || decode_cache(&stored, degree, count, seed)? != fresh {\n        return Err("published cache differs from independently generated public inputs".into());\n    }\n    let checksum = blake3::hash(&stored[..stored.len() - 32]).to_hex().to_string();\n    Ok((reference_digest, checksum, stored.len()))\n}\n'
MAIN = '#[path = "generator_inputs.rs"]\nmod inputs;\nuse std::{error::Error, path::Path};\nfn main() -> Result<(), Box<dyn Error>> {\n    let args: Vec<_> = std::env::args().skip(1).collect();\n    if args.len() != 4 { return Err("usage: fixture-generator DEGREE BATCH SEED DIRECTORY".into()); }\n    let degree: usize = args[0].parse()?;\n    let batch: usize = args[1].parse()?;\n    let seed: u64 = args[2].parse()?;\n    if !matches!(degree, 512 | 1024) || !(1..=1024).contains(&batch) {\n        return Err("unsupported degree or batch".into());\n    }\n    inputs::reject_fixture_overrides()?;\n    let (digest, checksum, bytes) = inputs::reference_and_publish(Path::new(&args[3]), degree, batch, seed)?;\n    println!("{}", serde_json::json!({\n        "schema":"bitz/falcon-degree-fixture-reference/v1",\n        "degree":degree,"batch":batch,"seed":seed,\n        "input_digest":digest,"cache_payload_blake3":checksum,"cache_bytes":bytes,\n        "fresh_uncached_generation":true,"cache_roundtrip_validated":true,\n        "upstream_verified":true,"ct_conversion_verified":true,"native_preflight_verified":true,\n        "cache_filename":format!("fn-dsa-0.3.0-n{degree}-b{batch}-seed{seed}.bin")\n    }));\n    Ok(())\n}\n'
DEPENDENCIES = ("bitz", "serde", "serde_json", "fn_dsa", "fn_dsa_comm", "rand_chacha", "blake3", "bincode")


def sha256(path):
    digest = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for chunk in iter(lambda: stream.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def resolve_rlibs(events):
    result = {}
    for event in events:
        if event.get("reason") != "compiler-artifact":
            continue
        name = event.get("target", {}).get("name")
        if name not in DEPENDENCIES:
            continue
        for filename in event.get("filenames", []):
            if filename.endswith(".rlib"):
                path = Path(filename).resolve()
                if name in result and result[name] != path:
                    raise ValueError("ambiguous library artifact: " + name)
                result[name] = path
    missing = set(DEPENDENCIES) - result.keys()
    if missing:
        raise ValueError("missing library artifacts: " + ", ".join(sorted(missing)))
    if len({path.parent for path in result.values()}) != 1:
        raise ValueError("dependency artifacts must share one Cargo deps directory")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-root", type=Path, required=True)
    parser.add_argument("--cargo-artifacts", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--helper", default="benches/common/falcon_degree_inputs.rs")
    args = parser.parse_args()
    root = args.source_root.resolve()
    helper = (root / args.helper).resolve()
    if not helper.is_relative_to(root):
        raise ValueError("helper must be within source root")
    events = [json.loads(line) for line in args.cargo_artifacts.read_text().splitlines() if line.startswith("{")]
    if not events or events[-1] != {"reason": "build-finished", "success": True}:
        raise ValueError("Cargo artifact log does not finish successfully")
    externs = resolve_rlibs(events)
    stage = args.output.resolve()
    stage.mkdir(parents=True, exist_ok=False)
    (stage / "generator_inputs.rs").write_bytes(helper.read_bytes() + WRAPPER.encode())
    (stage / "main.rs").write_text(MAIN)
    binary = stage / "falcon_fixture_generator"
    deps = next(iter(externs.values())).parent
    command = ["rustc", "--edition", "2024", "--crate-name", "falcon_fixture_generator",
               "-C", "opt-level=3", "-C", "target-cpu=native", "-L", f"dependency={deps}",
               str(stage / "main.rs"), "-o", str(binary)]
    for name, path in externs.items():
        command += ["--extern", f"{name}={path}"]
    method = {"status": "building", "argv": command,
              "started_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "helper_source": str(helper), "helper_sha256": sha256(helper),
              "cargo_artifacts": str(args.cargo_artifacts.resolve()),
              "cargo_artifacts_sha256": sha256(args.cargo_artifacts),
              "builder_sha256": sha256(__file__),
              "input_sources": {p.name: sha256(p) for p in stage.glob("*.rs")},
              "rlibs": {name: {"path": str(path), "sha256": sha256(path)} for name, path in externs.items()},
              "rustc": subprocess.check_output(["rustc", "-Vv"], cwd=root, text=True)}
    record = stage / "build-method.json"
    record.write_text(json.dumps(method, indent=2) + "\n")
    with (stage / "build.log").open("w") as log:
        result = subprocess.run(command, cwd=root, stdout=log, stderr=subprocess.STDOUT)
    method.update(status="built" if result.returncode == 0 else "failed", exit_code=result.returncode,
                  finished_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),
                  log_sha256=sha256(stage / "build.log"))
    if result.returncode == 0:
        binary.chmod(0o555)
        method.update(binary=str(binary), sha256=sha256(binary))
    record.write_text(json.dumps(method, indent=2) + "\n")
    print(method["status"])
    return result.returncode


if __name__ == "__main__":
    raise SystemExit(main())
