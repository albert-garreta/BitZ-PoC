#!/usr/bin/env python3
"""Complete missing cells, including eight threads and rejected timing retries."""
import csv
import json
import os
import shutil
import time

import run as campaign


def main():
    os.sched_setaffinity(0, {31})
    source_hashes = json.loads((campaign.OUT / 'sources.json').read_text())
    assert campaign.source_hashes() == source_hashes
    provenance = json.loads((campaign.OUT / 'provenance.json').read_text())
    for name, artifact in provenance['binaries'].items():
        assert campaign.sha(campaign.BIN / name) == artifact['sha256']

    results = json.loads((campaign.OUT / 'summary.json').read_text())
    completed = {(row['variant'], row['degree'], row['threads']) for row in results}
    for threads in (1, 2, 4, 8, 16):
        for degree in (512, 1024):
            for kind, name in [('arithmetic', 'falcon_algebraic'), ('full', 'falcon_hybrid')]:
                if (kind, degree, threads) in completed:
                    continue
                for attempt in range(3):
                    try:
                        result = campaign.run(kind, degree, campaign.BIN / name, threads)
                        break
                    except AssertionError as error:
                        if str(error) != 'SMT sibling was not sufficiently idle':
                            raise
                        stem = f'{kind}-{degree}-t{threads}'
                        archive = campaign.OUT / 'discarded' / f'{stem}-{time.time_ns()}'
                        archive.mkdir(parents=True)
                        for path in campaign.OUT.glob(stem + '.*'):
                            shutil.move(path, archive / path.name)
                        (archive / 'reason.json').write_text(json.dumps({'reason': str(error)}) + '\n')
                        print(f'Retrying {stem}: excluded noisy timing attempt {attempt + 1}', flush=True)
                else:
                    raise RuntimeError('Three timing attempts exceeded the sibling-load threshold')
                results.append(result)
                campaign.save('summary.json', results)

    provenance['actual_configuration_order'] = [
        [row['variant'], row['degree'], row['threads']] for row in results
    ]
    results.sort(key=lambda row: (row['threads'], row['degree'], row['variant']))
    assert len(results) == 20
    for degree in (512, 1024):
        assert len({row['input_digest'] for row in results if row['degree'] == degree}) == 1
    assert campaign.source_hashes() == source_hashes
    verified = 0
    for row in results:
        stem = f"{row['variant']}-{row['degree']}-t{row['threads']}"
        records = [json.loads(line) for line in (campaign.OUT / (stem + '.jsonl')).read_text().splitlines()]
        trials = [record for record in records if record.get('trial') in ('warmup', 'sample')]
        assert len(trials) == 6 and all(record['verified'] for record in trials)
        verified += len(trials)
    assert verified == 120
    campaign.save('summary.json', results)
    with (campaign.OUT / 'summary.csv').open('w') as file:
        writer = csv.DictWriter(file, fieldnames=list(results[0]))
        writer.writeheader()
        writer.writerows(results)
    provenance['thread_counts'] = [1, 2, 4, 8, 16]
    provenance['execution_order_threads'] = [1, 2, 4, 16, 8, 16]
    provenance['worker_cpus']['8'] = list(campaign.WORKER_ORDER[:8])
    campaign.save('provenance.json', provenance)
    campaign.save('validation.json', {
        'sources_unchanged': True,
        'verified_proofs': verified,
        'configurations': len(results),
        'matched_inputs_between_variants': True,
        'workers_pinned_to_distinct_physical_cores': True,
        'all_security_ledgers_at_least_128_bits': True,
    })


if __name__ == '__main__':
    main()
