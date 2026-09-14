"""Preserve exact measured executables, sources, manifests, and raw confirmation."""
import hashlib
import json
import sys
import tarfile
from pathlib import Path

primary, exploration, selections, out = map(Path, sys.argv[1:5])
refinement = Path(sys.argv[5]) if len(sys.argv) > 5 else None
assert json.loads((primary / 'campaign.json').read_text())['status'] == 'complete'
if refinement:
    assert json.loads((refinement / 'plan.json').read_text())['status'] == 'complete'
archive = out / 'frozen-runs.tar.xz'
with tarfile.open(archive, 'w:xz', preset=9) as dest:
    sources = [('confirmation', primary), ('exploration', exploration), ('frozen-selections', selections)]
    if refinement:
        sources.append(('refinement', refinement))
    for prefix, directory in sources:
        for path in sorted(directory.rglob('*')):
            if not path.is_file():
                continue
            # Confirmation carries every exact binary. Development retains
            # raw measurements and provenance without duplicating build artifacts.
            if prefix == 'exploration' and path.name in {'benchmark.bin', 'sources.tar.gz', 'assembly.txt.gz'}:
                continue
            dest.add(path, arcname=str(Path(prefix) / path.relative_to(directory)), recursive=False)
record = dict(archive=archive.name, bytes=archive.stat().st_size,
              sha256=hashlib.sha256(archive.read_bytes()).hexdigest(),
              contents='Exact confirmation/refinement binaries and sources, frozen manifests, raw measurements, and development evidence.')
(out / 'archive.json').write_text(json.dumps(record, indent=2) + '\n')
for name in ['finalize', 'refine', 'freeze-x86-shapes', 'archive']:
    filename = {'finalize':'bitz-finalize-x86-repairs.py','refine':'bitz-refine-x86-repairs.py',
                'freeze-x86-shapes':'bitz-freeze-x86-shapes.py','archive':'bitz-archive-x86-repairs.py'}[name]
    script = Path('/tmp') / filename
    if not script.exists():
        script = Path(__file__).with_name(name + '.py')
    content = script.read_text()
    content = content.replace("ROOT = Path(__file__).resolve().parents[2]", "ROOT = Path(__file__).resolve().parents[2]")
    content = content.replace("HERE=Path(__file__).resolve().parents[2]", "HERE=Path(__file__).resolve().parents[2]")
    (out / (name + '.py')).write_text(content)
test = Path('/tmp/test-x86-shape-policy.py')
if test.exists():
    content = test.read_text().replace("Path('/tmp/bitz-finalize-x86-repairs.py')", "Path(__file__).with_name('finalize.py')")
    (out / 'test_shape_policy.py').write_text(content)
print(json.dumps(record, indent=2))
