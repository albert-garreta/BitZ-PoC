#!/usr/bin/env python3
"""Archive completed campaign logs, verify every byte hash, then remove originals.

Run only after all measurements and report validation finish. Existing archives,
an existing archives.json, incomplete campaigns, unexpected raw files, or changed
inputs are errors. All campaigns are archived and verified before any loose log
is removed. A failure leaves existing evidence in place for inspection.
"""
from __future__ import annotations

import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import tarfile


def require(condition, message):
    if not condition:
        raise ValueError(message)


def stream_hash(stream):
    value = hashlib.sha256()
    for chunk in iter(lambda: stream.read(1024 * 1024), b''):
        value.update(chunk)
    return value.hexdigest()


def file_hash(path):
    with path.open('rb') as stream:
        return stream_hash(stream)


def raw_names(directory):
    return {path.name for pattern in ('*.stdout', '*.stderr', 'runs.jsonl')
            for path in directory.glob(pattern)}


def prepare_campaign(path):
    manifest_hash = file_hash(path)
    manifest = json.loads(path.read_text())
    config = manifest.get('configuration', {})
    if 'mode' not in config or 'diagnostic' not in config:
        return None
    require(manifest.get('complete') is True, f'{path}: campaign is incomplete')
    directory = path.parent
    archive = directory / 'raw-runs.tar.gz'
    require(not archive.exists() and not archive.with_suffix('.gz.partial').exists(),
            f'{directory}: archive or partial archive already exists')
    records_path = directory / 'runs.jsonl'
    require(records_path.is_file() and not records_path.is_symlink(), f'{records_path}: missing regular log')
    records = [json.loads(line) for line in records_path.read_text().splitlines() if line.strip()]
    expected = {(tuple(cell), variant, block) for cell in manifest['cells']
                for variant in ('baseline', 'candidate') for block in range(config['blocks'])}
    observed = [(tuple(row['cell']), row['variant'], row['block']) for row in records]
    require(expected and len(observed) == len(expected) and set(observed) == expected,
            f'{directory}: missing or duplicate process records')
    require(manifest['completed_processes'] == len(records), f'{directory}: process count differs')
    trials = config['warmup'] + config['samples']
    require(all(row['verified_proofs'] == trials for row in records)
            and manifest['verified_proofs'] == len(records) * trials,
            f'{directory}: verified proof count differs')
    names = {'runs.jsonl'}
    for cell, variant, block in observed:
        degree, batch, threads, security = cell
        stem = f'n{degree}-b{batch}-t{threads}-s{security}-r{block:02}-{variant}'
        names.update((stem + '.stdout', stem + '.stderr'))
    require(raw_names(directory) == names, f'{directory}: missing or unexpected loose raw files')
    hashes, sizes = {}, {}
    for name in sorted(names):
        raw = directory / name
        require(raw.is_file() and not raw.is_symlink(), f'{raw}: expected a regular file')
        hashes[name], sizes[name] = file_hash(raw), raw.stat().st_size
    require(file_hash(path) == manifest_hash, f'{path}: manifest changed during preflight')
    return dict(directory=directory, archive=archive, manifest_path=path,
                manifest_sha256=manifest_hash, files=hashes, sizes=sizes)


def verify_archive(path, hashes, sizes):
    observed = {}
    with tarfile.open(path, 'r:gz') as archive:
        for member in archive:
            require(member.isfile() and member.name in hashes and member.name not in observed,
                    f'{path}: unexpected, duplicate or nonregular archive member {member.name}')
            require(member.size == sizes[member.name], f'{path}: member size differs: {member.name}')
            with archive.extractfile(member) as stream:
                observed[member.name] = stream_hash(stream)
    require(observed == hashes, f'{path}: archived contents do not match original byte hashes')


def publish_without_overwrite(temporary, destination):
    # A hard link publishes the completed file atomically and refuses collisions.
    os.link(temporary, destination)
    temporary.unlink()


def archive_campaign(campaign):
    archive = campaign['archive']
    temporary = archive.with_suffix('.gz.partial')
    with temporary.open('xb') as output:
        # Fixed metadata makes archives reproducible for the same files and zlib.
        with gzip.GzipFile(filename='', mode='wb', fileobj=output, compresslevel=6, mtime=0) as zipped:
            with tarfile.open(fileobj=zipped, mode='w', format=tarfile.USTAR_FORMAT) as stream:
                for name in campaign['files']:
                    member = tarfile.TarInfo(name)
                    member.size = campaign['sizes'][name]
                    member.mode = 0o644
                    member.mtime = member.uid = member.gid = 0
                    member.uname = member.gname = ''
                    with (campaign['directory'] / name).open('rb') as raw:
                        stream.addfile(member, raw)
        output.flush()
        os.fsync(output.fileno())
    verify_archive(temporary, campaign['files'], campaign['sizes'])
    publish_without_overwrite(temporary, archive)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parent)
    root = parser.parse_args().root.resolve()
    index = root / 'archives.json'
    temporary_index = root / 'archives.json.partial'
    require(not index.exists() and not temporary_index.exists(), 'archives index or partial index already exists')
    campaigns = []
    for path in sorted(root.rglob('manifest.json')):
        campaign = prepare_campaign(path)
        if campaign is not None:
            campaigns.append(campaign)
    require(campaigns, 'no completed benchmark campaigns found')

    entries = []
    for campaign in campaigns:
        archive_campaign(campaign)
        entries.append(dict(path=str(campaign['archive'].relative_to(root)),
                            sha256=file_hash(campaign['archive']), files=campaign['files'],
                            archive_bytes=campaign['archive'].stat().st_size,
                            original_bytes=sum(campaign['sizes'].values()),
                            manifest_sha256=campaign['manifest_sha256'],
                            byte_hashes_verified=True))

    # Recheck every original before publishing the index or deleting any files.
    for campaign in campaigns:
        require(file_hash(campaign['manifest_path']) == campaign['manifest_sha256'],
                f'{campaign["directory"]}: manifest changed while archiving')
        require(raw_names(campaign['directory']) == set(campaign['files']),
                f'{campaign["directory"]}: raw file inventory changed while archiving')
        for name, expected_hash in campaign['files'].items():
            require(file_hash(campaign['directory'] / name) == expected_hash,
                    f'{campaign["directory"]}/{name}: raw bytes changed while archiving')
    with temporary_index.open('x') as stream:
        stream.write(json.dumps(entries, indent=2) + '\n')
        stream.flush()
        os.fsync(stream.fileno())
    publish_without_overwrite(temporary_index, index)

    for campaign in campaigns:
        for name in campaign['files']:
            (campaign['directory'] / name).unlink()
    print(json.dumps(dict(index=str(index), archives=len(entries),
                          archived_files=sum(len(row['files']) for row in entries),
                          archive_bytes=sum(row['archive_bytes'] for row in entries),
                          original_bytes=sum(row['original_bytes'] for row in entries))))


if __name__ == '__main__':
    main()
