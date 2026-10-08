import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys

root = Path.cwd()
sys.path.insert(0, str(root / 'scripts'))
from qualify_falcon_witness import source_identity

label = sys.argv[1]
target = Path('/home/john-wu/code/BitZ-pcs/target/falcon-witness-candidate')
out = target / 'frozen-commit-experiments' / label
out.mkdir(parents=True, exist_ok=False)
binaries = {}
for kind, source in [('full', target / 'release/deps/falcon_hybrid-0d2856552ca243a9'),
                     ('arithmetic', target / 'release/examples/falcon_algebraic')]:
    dest = out / source.name
    shutil.copy2(source, dest)
    binaries[kind] = dict(frozen_path=str(dest), sha256=hashlib.sha256(dest.read_bytes()).hexdigest())
manifest = dict(source=source_identity(root), binaries=binaries,
                rustc=subprocess.check_output(['rustc', '-Vv'], text=True),
                command='cargo build --offline --locked --release --features falcon-hybrid --example falcon_algebraic --bench falcon_hybrid',
                environment={'RUSTFLAGS': '-C target-cpu=native'},
                profile={'lto': True, 'codegen-units': 1})
folder = root / 'results/falcon-commit-experiments-20261008'
(folder / (label + '-build.json')).write_text(json.dumps(manifest, indent=2) + '\n')
print(json.dumps(dict(label=label, binaries=binaries)))
