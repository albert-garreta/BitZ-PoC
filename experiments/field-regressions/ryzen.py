#!/usr/bin/env python3
"""Sequential campaign driver for the current 9950X3D; no concurrent timing."""
import argparse
import json
import subprocess
import sys
from pathlib import Path
import host
from run import HERE, ROOT, save


def jobs(include_scaling):
    result=[('l3-96m','0',1,'all'),('l3-32m','8',1,'all')]
    if include_scaling:
        result += [('l3-96m-8t','0-7',8,'consumers'),('l3-32m-8t','8-15',8,'consumers'),
                   ('physical-16t','0-15',16,'consumers'),('smt-32t','0-31',32,'consumers')]
    return result


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--out',type=Path,required=True)
    p.add_argument('--phase',choices=['explore','confirm'],default='explore')
    p.add_argument('--exploration',type=Path,help='Previous campaign directory supplying per-placement selections')
    p.add_argument('--scaling',action='store_true')
    p.add_argument('--streaming',action='store_true')
    p.add_argument('--scope',choices=['all','regressions','comprehensive','candidate-repairs'],default='all')
    p.add_argument('--runs',type=int)
    p.add_argument('--samples',type=int)
    p.add_argument('--seed-offset',type=int)
    p.add_argument('--flags',default='-C target-cpu=native')
    p.add_argument('--target-dir',type=Path,default=Path('/tmp/bitz-ryzen-target'))
    p.add_argument('--dry-run',action='store_true')
    args=p.parse_args()
    if args.seed_offset is None: args.seed_offset=3141593 if args.phase=='explore' else 27182817
    machine=host.snapshot({0,8})
    if '9950X3D' not in (machine['model'] or ''):p.error('this placement matrix is specific to the Ryzen 9950X3D')
    if args.phase=='confirm' and not args.exploration:p.error('confirmation requires --exploration')
    commands=[]
    for label,cpus,threads,scope in jobs(args.scaling):
        if args.scope=='regressions':scope='regressions' if threads==1 else 'regressions-consumers'
        if args.scope=='comprehensive':scope='comprehensive' if threads==1 else 'comprehensive-consumers'
        if args.scope=='candidate-repairs':scope='candidate-repairs' if threads==1 else 'candidate-repairs-consumers'
        command=[sys.executable,str(HERE/'run.py'),'--suite','x86','--arch','x86_64','--cpu-set',cpus,
                 '--threads',str(threads),'--scope',scope,'--phase',args.phase,'--flags',args.flags,
                 '--out',str(args.out/label),'--target-dir',str(args.target_dir),
                 '--seed-offset',str(args.seed_offset)]
        if args.streaming and threads==1:command+=['--streaming']
        if args.phase=='explore':
            command+=['--runs',str(args.runs or 3),'--samples',str(args.samples or 8),'--calibration-ms','3','--no-retry']
        else:
            command+=['--selection',str(args.exploration/label/'selection.json')]
            if args.runs:command+=['--runs',str(args.runs)]
            if args.samples:command+=['--samples',str(args.samples)]
        commands.append(dict(label=label,command=command))
    if args.dry_run:
        print(json.dumps(commands,indent=2));return
    args.out.mkdir(parents=True,exist_ok=False)
    save(args.out/'campaign.json',dict(host=machine,phase=args.phase,jobs=commands,status='running'))
    status=[]
    for job in commands:
        result=subprocess.run(job['command'],cwd=ROOT)
        item=dict(label=job['label'],returncode=result.returncode)
        status.append(item);save(args.out/'status.json',status)
        if result.returncode:raise SystemExit(result.returncode)
        if args.phase=='confirm':
            gate=subprocess.run([sys.executable,str(HERE/'gate.py'),str(args.out/job['label'])],cwd=ROOT)
            item['gate_returncode']=gate.returncode;save(args.out/'status.json',status)
    save(args.out/'campaign.json',dict(host=machine,phase=args.phase,jobs=commands,status='complete'))
    raise SystemExit(any(row.get('gate_returncode',0) for row in status))

if __name__=='__main__':main()
