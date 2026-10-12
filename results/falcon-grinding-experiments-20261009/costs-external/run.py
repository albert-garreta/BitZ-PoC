"""Run only when no other performance campaign is active."""
import csv,json,os,pathlib,subprocess,time,hashlib
OUT=pathlib.Path(__file__).resolve().parent
STATES=['user','nice','system','idle','iowait','irq','softirq','steal']
WORKERS=[6,7,4,5,2,3,0,1,14,15,12,13,10,11,8,9]
def stat():
    return {r[0]:dict(zip(STATES,map(int,r[1:9]))) for line in pathlib.Path('/proc/stat').read_text().splitlines() if (r:=line.split())[0].startswith('cpu')}
def delta(a,b):
    result={}
    for cpu in a:
        d={state:b[cpu][state]-a[cpu][state] for state in STATES}
        total=sum(d.values())
        result[cpu]={'ticks':d,'total_ticks':total,'busy_percent':100*(total-d['idle']-d['iowait'])/max(1,total),
                     'state_percent':{s:100*t/max(1,total) for s,t in d.items()}}
    return result
def save(path,value):path.write_text(json.dumps(value,indent=2)+'\n')
def main():
    os.sched_setaffinity(0,{31})
    original=hashlib.sha256((OUT.parent/'costs/curves.json').read_bytes()).hexdigest()
    started=time.monotonic(); runs=[]
    before_all=stat()
    for threads in [1,2,4,8,16]:
        initial=stat();time.sleep(1);before=stat()
        idle=delta(initial,before)
        selected=WORKERS[:threads]
        argv=[str(OUT/'probe'),str(threads)]
        environment={k:v for k,v in os.environ.items() if not k.startswith(('BITZ_','RAYON_'))}
        environment.update({'RAYON_NUM_THREADS':str(threads),'OMP_NUM_THREADS':str(threads)})
        save(OUT/f't{threads}.command.json',{'argv':argv,'environment':{k:environment[k] for k in ('RAYON_NUM_THREADS','OMP_NUM_THREADS')},'main_cpu':6,'worker_cpus':selected,'external_entry':True})
        snapshots=[{'elapsed_seconds':0,'stat':before}]
        launch=time.monotonic()
        with (OUT/f't{threads}.raw.csv').open('w') as stdout,(OUT/f't{threads}.log').open('w') as stderr:
            process=subprocess.Popen(argv,env=environment,stdout=stdout,stderr=stderr)
            while process.poll() is None:
                time.sleep(.25)
                snapshots.append({'elapsed_seconds':time.monotonic()-launch,'stat':stat()})
        after=stat()
        record={'threads':threads,'returncode':process.returncode,'elapsed_seconds':time.monotonic()-launch,
                'idle_cpu_deltas':idle,'run_cpu_deltas':delta(before,after),'cpu_before':before,'cpu_after':after,
                'samples':snapshots,'tick_frequency_hz':os.sysconf('SC_CLK_TCK')}
        save(OUT/f't{threads}.resources.json',record)
        assert process.returncode==0,(threads,(OUT/f't{threads}.log').read_text()[-3000:])
        rows=list(csv.DictReader((OUT/f't{threads}.raw.csv').open()))
        assert all(r['worker']=='-1' and r['cpu']=='6' and r['valid']=='true' for r in rows)
        runs.append({k:v for k,v in record.items() if k not in ('samples','cpu_before','cpu_after')})
        print('completed external',threads,'threads',round(record['elapsed_seconds'],3),'s',flush=True)
    with (OUT/'raw.csv').open('w') as merged:
        for index,threads in enumerate([1,2,4,8,16]):
            lines=(OUT/f't{threads}.raw.csv').read_text().splitlines()
            merged.write('\n'.join(lines if index==0 else lines[1:])+'\n')
    save(OUT/'run.json',{'entry_context':'external mainCPU6/global Rayon pool; main shares core withworker0',
                        'elapsed_seconds':time.monotonic()-started,'runs':runs,'all_cpu_deltas':delta(before_all,stat()),
                        'original_cost_curves_sha256':original})
    assert hashlib.sha256((OUT.parent/'costs/curves.json').read_bytes()).hexdigest()==original
    subprocess.run(['python3',str(OUT/'analyze.py')],check=True)
if __name__=='__main__': main()
