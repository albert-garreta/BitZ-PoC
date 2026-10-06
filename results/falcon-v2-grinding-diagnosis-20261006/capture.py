import hashlib
import json
import os
import platform
from pathlib import Path
import subprocess
import time

root = Path.cwd()
mac = platform.system() == 'Darwin'
out = root / '.tmp/falcon-v2-diagnosis-20261006'
out.mkdir(parents=True, exist_ok=False)
manifest = json.loads((root / 'results/falcon-shared-prime-v2-20261006' / ('mac-manifest.json' if mac else 'will/manifest.json')).read_text())
item = manifest['binaries']['v2']
binary = Path(item['binary'])
assert hashlib.sha256(binary.read_bytes()).hexdigest() == item['sha256']
threads = 8 if mac else 16
cmd = [str(binary), '--protocol', 'shared-prime', '--security', '128', '--batch', '1024', '--seed', '42', '--threads', str(threads), '--warmup', '0', '--iterations', '1']
env = {k:v for k,v in os.environ.items() if not k.startswith(('BITZ_', 'FLOCK_'))}
env.update(BITZ_FALCON_STAGE_TIMINGS='1', RAYON_NUM_THREADS=str(threads))
record = dict(command=cmd, binary=item, diagnostic_only=True, note='One instrumented proof to locate costs; no warmup, not a performance qualification sample.', started_at=time.time())
with (out/'stdout.jsonl').open('x') as stdout, (out/'stages.jsonl').open('x') as stderr:
    process = subprocess.Popen(cmd, env=env, stdout=stdout, stderr=stderr)
    record['pid'] = process.pid
    (out/'run.json').write_text(json.dumps(record, indent=2)+'\n')
    print(json.dumps(dict(started=process.pid, output=str(out))), flush=True)
    try:
        record['exit_code'] = process.wait(timeout=420)
    finally:
        if process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
        record['finished_at'] = time.time()
        (out/'run.json').write_text(json.dumps(record, indent=2)+'\n')
assert record['exit_code'] == 0
rows = [json.loads(x) for x in (out/'stdout.jsonl').read_text().splitlines()]
trial = next(x for x in rows if x.get('event') == 'trial')
ref = json.loads((root/'results/falcon-shared-prime-v2-20261006'/('mac-discovery128' if mac else 'will/will-discovery128')/'early-stop.json').read_text())['candidate']
for key in ('input_digest', 'proof_debug_digest', 'roots', 'proof_payload_bytes', 'grinding_diagnostics'):
    assert trial[key] == ref[key], key
assert trial['verified'] and rows[-1]['verified']
print(json.dumps(dict(verified=True, total_prover_ms=trial['total_prover_ms'], prove_ms=trial['proof_prove_ms'], proof_digest=trial['proof_debug_digest'])), flush=True)
