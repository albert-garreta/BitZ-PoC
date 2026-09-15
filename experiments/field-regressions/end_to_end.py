#!/usr/bin/env python3
"""Freeze and measure verified F2Z consumers, with optional paired candidate checkout.

Without --candidate-root these are explicitly baseline characterization. Candidate
activation must be established by its integration tests; changing a microbenchmark
selection file does not wire that kernel into a prover.
"""
import argparse
import json
import os
import random
import shutil
import subprocess
import sys
from pathlib import Path
import host
from ryzen import jobs
from run import ROOT, clean_environment, save, sha


def cases():
    result=[]
    for workload in ['u32-mod32','u128']:
        for n in [15,20]:
            result.append(dict(id=f'{workload}-n{n}',bench='mul_e2e_compare',args=[],
                env={'F2Z_BENCH_SHAPES':str(n),'F2Z_MUL_COMPARE_WORKLOADS':workload,
                     'F2Z_MUL_COMPARE_BACKENDS':'f2z','F2Z_MUL_COMPARE_MEMORY':'0'}))
    for n in [7,12]:
        result.append(dict(id=f'sha-ecdsa-n{n}',bench='sha256_ecdsa',args=[str(n),'split','100','1'],env={}))
    for shape in ['13:7:1','16:10:1','7:8:32']:
        result.append(dict(id='pcs-'+shape.replace(':','-'),bench='pcs',args=[],env={'F2Z_BENCH_SHAPES':shape}))
    return result


def freeze(root,dest):
    dest.mkdir()
    for name in ['src','crates','benches','tests','examples','vendor','benchmarks/binius64','benchmarks/zkpassport']:
        if (root/name).exists():
            shutil.copytree(root/name,dest/name,ignore=shutil.ignore_patterns('target','.git','results','__pycache__'))
    for name in ['Cargo.toml','Cargo.lock','rust-toolchain.toml']:
        shutil.copy2(root/name,dest/name)
    return {str(p.relative_to(dest)):sha(p) for p in dest.rglob('*') if p.is_file()}


def records(output):
    parsed=[]
    for line in output.splitlines():
        line=line.strip()
        if line.startswith('{'):
            try:value=json.loads(line)
            except json.JSONDecodeError:continue
            if isinstance(value,dict):parsed.append(value)
        elif line.startswith('RESULT '):
            parsed.append(dict(token.split('=',1) for token in line.split()[1:] if '=' in token))
    return parsed


def trace_processor():
    requested=os.environ.get('PERFETTO_TRACE_PROCESSOR','trace_processor_shell')
    path=shutil.which(requested)
    if not path:
        raise ValueError('full-prover benches require native Perfetto; set PERFETTO_TRACE_PROCESSOR to its executable before building')
    version=subprocess.check_output([path,'--version'],text=True,stderr=subprocess.STDOUT).strip()
    return Path(path).resolve(),version


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--out',type=Path,required=True)
    p.add_argument('--baseline-root',type=Path,default=ROOT)
    p.add_argument('--candidate-root',type=Path)
    p.add_argument('--activation-evidence',type=Path,help='Required candidate integration-test record, copied into the report')
    p.add_argument('--cases',help='Comma-separated case IDs; omitted cases are explicitly unmeasured')
    p.add_argument('--scaling',action='store_true')
    p.add_argument('--runs',type=int,default=5)
    p.add_argument('--flags',default='-C target-cpu=native')
    p.add_argument('--target-dir',type=Path,default=Path('/tmp/bitz-prover-target'))
    p.add_argument('--dry-run',action='store_true')
    args=p.parse_args()
    if args.runs<1:p.error('--runs must be positive')
    if args.candidate_root and not args.activation_evidence:p.error('a paired candidate needs --activation-evidence from its integration tests')
    selected=cases()
    if args.cases:
        requested=set(args.cases.split(','))
        if requested-{c['id'] for c in selected}:p.error('unknown consumer case')
        selected=[c for c in selected if c['id'] in requested]
    placements=jobs(args.scaling)
    plan=dict(cases=selected,placements=placements,runs=args.runs,
              mode='paired_candidate' if args.candidate_root else 'baseline_characterization',
              flags=args.flags,security=100,ligerito='custom:1:4:100',
              omitted_cases=[c['id'] for c in cases() if c not in selected])
    if args.dry_run:print(json.dumps(plan,indent=2));return
    try:processor,version=trace_processor()
    except (ValueError,subprocess.CalledProcessError,OSError) as error:p.error(str(error))
    lease=host.exclusive()
    args.out=args.out.resolve();args.out.mkdir(parents=True,exist_ok=False)
    frozen_processor=args.out/'trace_processor_shell';shutil.copy2(processor,frozen_processor)
    plan['trace_processor']=dict(source=str(processor),version=version,sha256=sha(frozen_processor))
    plan['host']=host.snapshot({0,8});save(args.out/'plan.json',plan)
    if args.activation_evidence:shutil.copy2(args.activation_evidence,args.out/'activation-evidence.json')
    roots={'baseline':args.baseline_root.resolve()}
    if args.candidate_root:roots['candidate']=args.candidate_root.resolve()
    binaries={};source_hashes={}
    # All builds finish before any timed worker is launched.
    for variant,root in roots.items():
        dest=args.out/f'{variant}-source';source_hashes[variant]=freeze(root,dest)
        env=clean_environment(args.flags,args.target_dir.resolve(),1)
        features={'span-metrics'}
        if any(c['bench']=='mul_e2e_compare' for c in selected):features.update(['bench-internals','native-mul-compare'])
        if any(c['bench']=='sha256_ecdsa' for c in selected):features.add('ecdsa')
        command=['cargo','bench','--offline','--no-run','--message-format=json','--manifest-path',str(dest/'Cargo.toml'),
                 '--features',','.join(sorted(features))]
        for bench in sorted({c['bench'] for c in selected}):command+=['--bench',bench]
        with (args.out/f'{variant}-build.jsonl').open('w') as log, (args.out/f'{variant}-build.log').open('w') as errors:
            subprocess.run(command,cwd=dest,env=env,stdout=log,stderr=errors,check=True)
        for row in records((args.out/f'{variant}-build.jsonl').read_text()):
            if row.get('reason')=='compiler-artifact' and row.get('executable'):
                name=row['target']['name'];binary=args.out/f'{variant}-{name}.bin';shutil.copy2(row['executable'],binary)
                binaries[variant,name]=binary
    save(args.out/'sources.json',source_hashes)
    hashes={str(path.name):sha(path) for path in binaries.values()};save(args.out/'binaries.json',hashes)
    results=[];order=random.Random(2718281)
    for label,cpus,threads,_ in placements:
        for case in selected:
            for run in range(args.runs):
                variants=list(roots);order.shuffle(variants)
                for variant in variants:
                    binary=binaries[variant,case['bench']]
                    if sha(binary)!=hashes[binary.name]:raise ValueError('frozen binary changed')
                    env=clean_environment(args.flags,args.target_dir.resolve(),threads)
                    for key in list(env):
                        if key.startswith(('F2Z_','NTT_')):env.pop(key)
                    env.update(RAYON_NUM_THREADS=str(threads),F2Z_BENCH_REPS='1',
                               F2Z_BENCH_SEED=str(424242+run),F2Z_LIG_PROFILE='custom:1:4:100',
                               PERFETTO_TRACE_PROCESSOR=str(frozen_processor),**case['env'])
                    if case['bench']!='pcs':env['F2Z_BENCH_LAMBDA']='100'
                    stem=f'{label}-{case["id"]}-{variant}-{run}'
                    env['F2Z_MUL_COMPARE_OUTPUT_DIR']=str(args.out/(stem+'-detail'))
                    stdout=args.out/(stem+'.stdout');stderr=args.out/(stem+'.stderr')
                    before=host.snapshot(host.cpu_set(cpus))
                    command=['taskset','--cpu-list',cpus,str(binary),*case['args']]
                    with stdout.open('w') as out,stderr.open('w') as err:
                        subprocess.run(command,cwd=args.out,env=env,stdout=out,stderr=err,check=True)
                    parsed=records(stdout.read_text())
                    measured=[r for r in parsed if r.get('schema') and r.get('trial')!='warmup']
                    if not measured:raise ValueError(f'no benchmark records: {stem}')
                    if any(int(r['threads']) != threads for r in measured if 'threads' in r):
                        raise ValueError(f'worker thread count differs from requested placement: {stem}')
                    results.append(dict(placement=label,cpus=cpus,threads=threads,case=case['id'],variant=variant,
                        run=run,records=measured,host_before=before,host_after=host.snapshot(host.cpu_set(cpus)),
                        stdout_sha256=sha(stdout),stderr_sha256=sha(stderr)))
                    save(args.out/'results.json',results)
                    print(f'Completed {stem}',flush=True)
    save(args.out/'completion.json',dict(status='complete',mode=plan['mode'],workers=len(results),
                                       results_sha256=sha(args.out/'results.json')))

if __name__=='__main__':main()
