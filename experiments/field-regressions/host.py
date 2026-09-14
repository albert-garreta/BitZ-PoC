"""Read and validate Linux CPU placement without changing machine policy."""
import os
from pathlib import Path


def cpu_set(value):
    cpus=set()
    for part in value.split(','):
        ends=part.split('-')
        if len(ends)==1: cpus.add(int(ends[0]))
        elif len(ends)==2 and int(ends[0])<=int(ends[1]):cpus.update(range(int(ends[0]),int(ends[1])+1))
        else:raise ValueError('expected CPUs such as 0 or 0-7,16-23')
    if not cpus or min(cpus)<0:raise ValueError('empty or negative CPU set')
    return cpus


def read(path):
    try:return Path(path).read_text().strip()
    except OSError:return None


def snapshot(cpus=None):
    allowed=os.sched_getaffinity(0) if hasattr(os,'sched_getaffinity') else set()
    selected=set(cpus or allowed)
    if not selected<=allowed:raise ValueError(f'CPUs unavailable to this process: {sorted(selected-allowed)}')
    info=read('/proc/cpuinfo') or ''
    first=info.split('\n\n')[0]
    values=dict(line.split(':',1) for line in first.splitlines() if ':' in line)
    values={k.strip():v.strip() for k,v in values.items()}
    topology=[]
    for cpu in sorted(selected):
        base=Path(f'/sys/devices/system/cpu/cpu{cpu}')
        cache=base/'cache/index3'
        topology.append(dict(cpu=cpu, core=read(base/'topology/core_id'),
            siblings=read(base/'topology/thread_siblings_list'),
            l3_cpus=read(cache/'shared_cpu_list'),l3_size=read(cache/'size'),
            governor=read(base/'cpufreq/scaling_governor'),frequency_khz=read(base/'cpufreq/scaling_cur_freq')))
    return dict(model=values.get('model name'),stepping=values.get('stepping'),microcode=values.get('microcode'),
        runtime_features=values.get('flags','').split(),allowed_cpus=sorted(allowed),cpus=sorted(selected),
        topology=topology,loadavg=read('/proc/loadavg'),boost=read('/sys/devices/system/cpu/cpufreq/boost'))


def streaming_terms(host):
    sizes=[]
    for cpu in host['topology']:
        size=cpu['l3_size']
        if not size:raise ValueError('cannot establish streaming coverage without L3 size')
        sizes.append(int(size.rstrip('KM'))*(1024 if size.endswith('K') else 1024**2))
    # GF dot has the smallest active footprint, 32 bytes per element.
    terms=2*max(sizes)//32+1
    return 1<<(terms-1).bit_length()


def exclusive(path=Path('/tmp/bitz-arithmetic-benchmark.lock')):
    """Hold across build and timing; separate drivers must not contend silently."""
    import fcntl
    stream=path.open('a+')
    try:
        fcntl.flock(stream,fcntl.LOCK_EX|fcntl.LOCK_NB)
    except BlockingIOError:
        stream.close()
        raise RuntimeError('another BitZ benchmark driver owns the machine lease') from None
    return stream
