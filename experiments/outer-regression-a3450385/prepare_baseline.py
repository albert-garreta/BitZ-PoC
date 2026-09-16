#!/usr/bin/env python3
"""Prepare a historical build without touching the working tree or git metadata."""
import io, subprocess, tarfile, tempfile, shutil
from pathlib import Path
root=Path(__file__).resolve().parents[2]
base=Path(tempfile.mkdtemp(prefix='f2z-a3450385-'))
paths=['src','Cargo.toml','Cargo.lock','vendor/field','vendor/flock-mod','crates']
data=subprocess.check_output(['git','archive','a3450385e2a90115add370a3c05e5127db0b3a87',*paths],cwd=root)
with tarfile.open(fileobj=io.BytesIO(data)) as archive:archive.extractall(base,filter='data')
for name in ('experiments','tests','examples'):(base/name).symlink_to(root/name,target_is_directory=True)
(base/'benches').mkdir()
for path in (root/'benches').iterdir():
 if path.name not in ('outer_regression','outer_regression.rs'):(base/'benches'/path.name).symlink_to(path,target_is_directory=path.is_dir())
shutil.copytree(root/'benches/outer_regression',base/'benches/outer_regression')
shutil.copyfile(root/'benches/outer_regression.rs',base/'benches/outer_regression.rs')
shutil.copyfile(Path(__file__).with_name('legacy_adapter.rs'),base/'benches/outer_regression/adapter.rs')
with (base/'src/piop/spartan/mod.rs').open('a') as f:f.write('\n#[cfg(feature = "bench-internals")]\n#[doc(hidden)]\n#[path = "../../../benches/outer_regression/driver.rs"]\npub mod outer_regression;\n')
with (base/'Cargo.toml').open('a') as f:f.write('\n[[bench]]\nname = "outer_regression"\nharness = false\nrequired-features = ["parallel", "bench-internals"]\n')
# Archived mtimes can otherwise hide source changes when reusing a target dir.
(base/'src/lib.rs').touch()
print(base)
