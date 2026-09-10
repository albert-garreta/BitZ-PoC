"""Regression tests for verified independent multiplication campaigns."""
import copy
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import run_native_mul_compare as runner
import native_mul_table as table
from native_mul_results import fingerprint, require_compatible

FIXTURE = json.loads('{"backend": "limber", "config": {"brakedown": {"direct_open_max": 65536, "row_len_cap": 32768, "spec": 4, "split": false, "target_bits": 114}, "challenge_target_bits": 117, "commitment_backend": "brakedown", "constraints": 32768, "engine": "T256DynPrimeBdEngine", "inteval_target_bits": 128, "k": 9, "log_p": 23, "log_q": 256, "log_t": 32, "log_t_f": 32, "modulus": "4294967296", "numlimb": 1, "operand_bits": 32, "padded_constraints": 32768, "padded_variables": 131072, "pcs": "Limber IntEval/Brakedown", "piop": "Integer-Mod Spartan", "proof_size_encoding": "canonical input commitments + canonical eval argument + shape-derived fixed-width sumcheck payload", "quotients": 32768, "s": 12, "security_policy": "native Limber ~114-bit fingerprint floor; IntEval component targets reported separately", "target_bits": 114, "variables": 98304}, "corpus_digest": "a006d0e2143cfce1ec9dc60dd92be801f48a126b8076d4704cb698e7fd4ac9da", "log_multiplications": 15, "measurement_policy": "warm-process/v1", "metrics": {"online_prover_ms": 652.6666, "proof_bytes": 3417232, "verify_ms": 36.979629, "witness_ms": 3.215627, "witness_to_proof_ms": 655.882409}, "multiplications": 32768, "proof_size": {"commitment_bytes": 72, "eval_arg_bytes": 3415800, "sumcheck_bytes": 1360}, "proof_verified": true, "schema": "native-mul-sample/v2", "seed": 6139306037344403556, "setup_ms": 18.515112000000002, "threads": 8, "trial": {"index": 0, "kind": "warmup"}, "workload": "u32-mod32"}')
SOURCE = dict(source_sha256="a"*64, build=runner.BUILD, machine={"cpu":"test"}, git_revision="test", git_dirty=True)


def samples(reps=2):
    result = []
    for index in range(reps + 1):
        row = copy.deepcopy(FIXTURE)
        row["trial"] = {"kind":"warmup" if index == 0 else "sample", "index":max(0,index-1)}
        row["metrics"]["online_prover_ms"] = 999 if index == 0 else 10*index
        result.append(row)
    return result


def config(**overrides):
    return dict(reps=2, threads=8, seed=runner.DEFAULT_SEED, seed_explicit=False, memory=False,
                workloads=["u32-mod32"], backends=["limber"], exponents=[15], **overrides)


class RunnerTests(unittest.TestCase):
    def test_warmup_excluded_and_repetitions_enforced(self):
        rows = samples()
        summary = runner.summarize_case(rows, None, config(), "limber", "u32-mod32", 15, SOURCE)
        self.assertEqual(summary["medians"]["online_prover_ms"], 15)
        for malformed in (rows[1:], rows + [rows[-1]], rows[::-1]):
            with self.assertRaises(ValueError):
                runner.summarize_case(malformed, None, config(), "limber", "u32-mod32", 15, SOURCE)

    def test_wrong_relation_backend_or_security_rejected(self):
        changes = [dict(proof_verified=False), dict(multiplications=32767), dict(workload="u32-mod32-chain"),
                   dict(threads=1), dict(corpus_digest="bad"), dict(measurement_policy="cold-process/v1")]
        for change in changes:
            with self.subTest(change=change), self.assertRaises(ValueError):
                runner.validate_sample(FIXTURE | change, config(), 15, "limber", "u32-mod32")
        for field, value in (("commitment_backend", "hyrax"), ("padded_variables", 65536), ("target_bits",100)):
            row = copy.deepcopy(FIXTURE)
            row["config"][field] = value
            with self.assertRaises(ValueError):
                runner.validate_sample(row, config(),15,"limber","u32-mod32")
        for field, value in (("online_prover_ms",float("nan")), ("proof_bytes",0)):
            row = copy.deepcopy(FIXTURE)
            row["metrics"][field] = value
            with self.assertRaises(ValueError):
                runner.validate_sample(row,config(),15,"limber","u32-mod32")
        row = copy.deepcopy(FIXTURE)
        row["proof_size"]["commitment_bytes"] = 0
        with self.assertRaises(ValueError):
            runner.validate_sample(row,config(),15,"limber","u32-mod32")
        with self.assertRaises(json.JSONDecodeError):
            runner.structured_lines("LIMBER_MUL_RESULT {bad", "LIMBER_MUL_RESULT ")
        self.assertEqual(runner.structured_lines("old chain timing: 20ms", "LIMBER_MUL_RESULT "), [])

    def test_memory_requires_verified_matching_corpus_and_boundary(self):
        cfg = config(); cfg["memory"] = True
        row = FIXTURE | dict(peak_rss_bytes=123456, boundary=runner.MEMORY_BOUNDARY)
        runner.summarize_case(samples(), row, cfg, "limber", "u32-mod32",15,SOURCE)
        for change in (dict(proof_verified=False),dict(corpus_digest="b"*64),dict(boundary="includes cargo"),dict(peak_rss_bytes=0)):
            with self.assertRaises(ValueError):
                runner.summarize_case(samples(),row|change,cfg,"limber","u32-mod32",15,SOURCE)

    def test_environment_and_wide_defaults_preserved(self):
        env = runner.campaign_environment({key:"unwanted" for key in runner.CLEAR_ENV}, config())
        self.assertTrue(all(key not in env for key in runner.CLEAR_ENV))
        self.assertEqual(env["RUSTFLAGS"], "-C target-cpu=native")
        self.assertEqual(env["RAYON_NUM_THREADS"], "8")
        self.assertNotIn("F2Z_BENCH_SEED",env)
        wide = config(); wide["workloads"] = ["u128"]
        self.assertEqual(runner.campaign_environment({"F2Z_BINIUS_LOG_INV_RATE":"3"},wide)["F2Z_BINIUS_LOG_INV_RATE"],"3")
        for workload in ("u64", "u128"):
            maximum = sys.maxsize.bit_length() + 1 - 11
            env = dict(F2Z_MUL_COMPARE_WORKLOADS=workload,F2Z_MUL_COMPARE_BACKENDS="f2z binius64",F2Z_BENCH_SHAPES=f"15 {maximum}")
            self.assertEqual(runner.configuration(env)["exponents"],[15,maximum])
            with self.assertRaises(ValueError): runner.configuration(env | {"F2Z_BENCH_SHAPES":str(maximum+1)})
        env = dict(F2Z_MUL_COMPARE_BACKENDS="f2z")
        self.assertEqual(runner.configuration(env | {"F2Z_MUL_COMPARE_WORKLOADS":"u32"})["workloads"],["u32-mod32"])
        for extra in (dict(F2Z_MUL_COMPARE_WORKLOADS="u32 u32-mod32"),dict(F2Z_MUL_COMPARE_WORKLOADS="babybear"),dict(F2Z_MUL_COMPARE_BACKENDS="unknown"),dict(RAYON_NUM_THREADS="0")):
            with self.assertRaises(ValueError): runner.configuration(env | extra)

    def test_whir_opt_in_security_and_replay_identity(self):
        cfg = runner.configuration(dict(F2Z_MUL_COMPARE_BACKENDS="plonky3-fri plonky3-whir", RAYON_NUM_THREADS="1"))
        self.assertEqual(cfg["threads"], 1)
        self.assertEqual(runner.campaign_environment({}, cfg)["RAYON_NUM_THREADS"], "1")
        row = copy.deepcopy(FIXTURE)
        row.update(backend="plonky3-whir", threads=1)
        row["config"] = dict(pcs="WHIR", base_field="Goldilocks", encoding="Reed-Solomon",
                             opening_claim="prescribed multilinear evaluation", params=dict(extension_degree=5),
                             security=dict(model="native-air-whir-johnson-union/v1", assumption="JohnsonBound",
                                           target_bits=100, achieved_bits=100.1,
                                           air=dict(log_height=15, width=137, constraints=139, constraint_degree=2)))
        runner.validate_sample(row, cfg, 15, "plonky3-whir", "u32-mod32")
        for change in (dict(achieved_bits=99.9), dict(achieved_bits=float("nan")), dict(assumption="UniqueDecoding"), dict(air={})):
            bad = copy.deepcopy(row); bad["config"]["security"].update(change)
            with self.assertRaises(ValueError):
                runner.validate_sample(bad, cfg, 15, "plonky3-whir", "u32-mod32")
        rows = [copy.deepcopy(row) for _ in range(cfg["reps"]+1)]
        for index, sample in enumerate(rows):
            sample["trial"] = dict(kind="warmup" if index == 0 else "sample", index=max(0,index-1))
        memory = row | dict(peak_rss_bytes=1234, boundary=runner.MEMORY_BOUNDARY)
        summary = runner.summarize_case(rows, memory, cfg, "plonky3-whir", "u32-mod32", 15, SOURCE)
        self.assertEqual(summary["threads"], 1)
        bad = copy.deepcopy(memory); bad["config"]["params"]["extension_degree"] = 2
        with self.assertRaises(ValueError):
            runner.summarize_case(rows, bad, cfg, "plonky3-whir", "u32-mod32", 15, SOURCE)
        rows[-1]["config"]["params"]["extension_degree"] = 2
        with self.assertRaises(ValueError):
            runner.summarize_case(rows, memory, cfg, "plonky3-whir", "u32-mod32", 15, SOURCE)

    def test_incompatible_results_and_size_import_rejected(self):
        rows = samples()
        summary = runner.summarize_case(rows,None,config(),"limber","u32-mod32",15,SOURCE)
        for field,value in (("corpus_digest","f"*64),("measurement_policy","old"),("protocol_fingerprint","b"*64)):
            with self.assertRaises(ValueError): require_compatible(summary,summary | {field:value})
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            (root/"summary.json").write_text(json.dumps([summary]))
            (root/"samples.jsonl").write_text("\n".join(map(json.dumps,rows)))
            self.assertEqual(table.proof_sizes(root,"u32-mod32")[("limber",15)],FIXTURE["metrics"]["proof_bytes"])
            rows[1]["config"]["commitment_backend"]="hyrax"
            (root/"samples.jsonl").write_text("\n".join(map(json.dumps,rows)))
            with self.assertRaises(ValueError): table.proof_sizes(root,"u32-mod32")
            old=summary|{"schema":"historical-chain/v1"}
            (root/"summary.json").write_text(json.dumps([old]))
            with self.assertRaises(ValueError): table.load_summaries(root,"u32-mod32")

    def test_shell_exact_author_command_one_warm_process_and_one_memory_process(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp); repo=root/"limber checkout"; (repo/"examples").mkdir(parents=True)
            for name in ("Cargo.toml","Cargo.lock","examples/int_mult.rs"): (repo/name).write_text("fixture")
            bindir=root/"bin"; bindir.mkdir(); commands=root/"commands.jsonl"
            cargo=bindir/"cargo"
            cargo.write_text("#!"+sys.executable+"\n" + "import json,os,sys\n" +
                "with open(os.environ['COMMANDS'],'a') as f: f.write(json.dumps(dict(args=sys.argv[1:],cwd=os.getcwd(),flags=os.environ.get('RUSTFLAGS'),threads=os.environ.get('RAYON_NUM_THREADS'),encoded=os.environ.get('CARGO_ENCODED_RUSTFLAGS'))) + '\\n')\n" +
                "if os.environ.get('FAIL_CARGO'): sys.exit(7)\n" +
                "rows=json.loads("+repr(json.dumps(samples()))+")\n" +
                "if os.environ.get('F2Z_MUL_MEMORY_ONLY') == '1':\n" +
                " row=rows[0]; row.update(peak_rss_bytes=123456,boundary="+repr(runner.MEMORY_BOUNDARY)+"); print('LIMBER_MUL_MEMORY '+json.dumps(row))\n" +
                "else:\n for row in rows: print('LIMBER_MUL_RESULT '+json.dumps(row))\n")
            cargo.chmod(0o755)
            git=bindir/"git";git.write_text('#!/bin/sh\nif [ "$1" = rev-parse ]; then printf "%s\\n" test-revision; fi\n');git.chmod(0o755)
            sysctl=bindir/"sysctl";sysctl.write_text('#!/bin/sh\nprintf "%s\\n" fixture-cpu\n');sysctl.chmod(0o755)
            output=root/"results"
            env={k:v for k,v in os.environ.items() if not k.startswith(("F2Z_","CARGO_"))}
            env.update(PATH=str(bindir)+os.pathsep+env["PATH"],COMMANDS=str(commands),LIMBER_REPO=str(repo),
                       RAYON_NUM_THREADS="8",F2Z_BENCH_REPS="2",F2Z_MUL_COMPARE_BACKENDS="limber",
                       F2Z_MUL_COMPARE_OUTPUT_DIR=str(output),CARGO_ENCODED_RUSTFLAGS="remove",PYTHONDONTWRITEBYTECODE="1")
            command=["bash",str(runner.ROOT/"scripts/run_native_mul_compare.sh")]
            run=subprocess.run(command,env=env,capture_output=True,text=True)
            self.assertEqual(run.returncode,0,run.stdout+run.stderr)
            calls=[json.loads(line) for line in commands.read_text().splitlines()]
            self.assertEqual(len(calls),2)
            for call in calls:
                self.assertEqual(call["args"],runner.limber_command(15)[1:]); self.assertEqual(Path(call["cwd"]).resolve(),repo.resolve())
                self.assertEqual(call["flags"],"-C target-cpu=native"); self.assertEqual(call["threads"],"8"); self.assertIsNone(call["encoded"])
            summary=json.loads((output/"summary.json").read_text())[0]
            self.assertEqual(summary["samples"],2);self.assertEqual(summary["multiplications"],32768)
            self.assertEqual(json.loads((output/"campaign.json").read_text())["status"],"complete")
            failed=root/"failed";env.update(FAIL_CARGO="1",F2Z_MUL_COMPARE_OUTPUT_DIR=str(failed))
            run=subprocess.run(command,env=env,capture_output=True,text=True)
            self.assertNotEqual(run.returncode,0)
            self.assertEqual(json.loads((failed/"campaign.json").read_text())["status"],"failed")


if __name__ == "__main__": unittest.main()
