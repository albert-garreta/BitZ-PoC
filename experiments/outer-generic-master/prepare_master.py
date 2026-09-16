#!/usr/bin/env python3
"""Install only a benchmark driver into an exact archive of master."""
import argparse, io, subprocess, tarfile
from pathlib import Path
ROOT = Path(__file__).resolve().parents[2]
REVISION = 'ac0aa44c5785db30f889fce8c5cdc98264a0e686'
p = argparse.ArgumentParser()
p.add_argument('destination', type=Path)
a = p.parse_args()
a.destination.mkdir(parents=True, exist_ok=False)
data = subprocess.check_output(['git', 'archive', REVISION], cwd=ROOT)
with tarfile.open(fileobj=io.BytesIO(data)) as archive:
    archive.extractall(a.destination, filter='data')
driver = (Path(__file__).parent / 'master_driver.rs').read_text()
(a.destination / 'src/piop/spartan/outer_master_benchmark.rs').write_text(driver)
with (a.destination / 'src/piop/spartan/mod.rs').open('a') as f:
    f.write('\n#[cfg(feature="bench-internals")]\n#[doc(hidden)]\npub mod outer_master_benchmark;\n')
(a.destination / 'benches/outer_master.rs').write_text('fn main() { f2z::piop::spartan::outer_master_benchmark::run(); }\n')
with (a.destination / 'Cargo.toml').open('a') as f:
    f.write('\n[[bench]]\nname="outer_master"\nharness=false\nrequired-features=["parallel", "bench-internals"]\n')
print(a.destination.resolve())
