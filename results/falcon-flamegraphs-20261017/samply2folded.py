"""Convert a samply profile (+ its --unstable-presymbolicate sidecar) to folded stacks.

usage: python3 -I samply2folded.py <profile.json> <out.folded> [cpu|wall]

samply samples every thread on a wall-clock timer and records each thread's
CPU time since its previous sample. `cpu` (the default) weights a sample by
that CPU delta: time a thread spends suspended by the sampler costs no CPU, so
the attribution does not depend on the sampler's overhead. `wall` weights it by
the wall time until the next sample instead. Counts are microseconds. Frames
are written as `library`symbol`, the format inferno-collapse-sample emits.
"""

import bisect
import collections
import json
import sys
from pathlib import Path

profile_path = Path(sys.argv[1])
out_path = Path(sys.argv[2])
weighting = sys.argv[3] if len(sys.argv) > 3 else "cpu"
assert weighting in ("cpu", "wall")
profile = json.loads(profile_path.read_text())
sidecar = json.loads(profile_path.with_name(profile_path.name.replace(".json", ".syms.json")).read_text())

strings = sidecar["string_table"]
tables = {}
for lib in sidecar["data"]:
    entries = sorted((e["rva"], e["size"], strings[e["symbol"]]) for e in lib["symbol_table"])
    tables[lib["debug_name"]] = ([e[0] for e in entries], entries)


def symbolize(lib_name: str, address: int) -> str:
    starts, entries = tables.get(lib_name, ([], []))
    for probe in (address, address - 1):
        index = bisect.bisect_right(starts, probe) - 1
        if index >= 0:
            rva, size, name = entries[index]
            if rva <= probe < rva + max(size, 1):
                return name
    return f"[{lib_name}]"


libs = profile["libs"]
folded = collections.Counter()
for thread in profile["threads"]:
    names = thread["stringArray"]
    frames, stacks, resources = thread["frameTable"], thread["stackTable"], thread["resourceTable"]
    frame_names = []
    for i in range(frames["length"]):
        func = frames["func"][i]
        resource = thread["funcTable"]["resource"][func]
        lib_index = resources["lib"][resource] if resource is not None and resource >= 0 else None
        lib = libs[lib_index]["debugName"] if lib_index is not None else "?"
        # `;` separates frames in the folded format; demangled array types
        # (`[u64; 2]`) contain it.
        frame_names.append(f"{lib}`{symbolize(lib, frames['address'][i])}".replace(";", ","))
    cache = {}

    def path(stack):
        if stack in cache:
            return cache[stack]
        parts = []
        node = stack
        while node is not None:
            parts.append(frame_names[stacks["frame"][node]])
            node = stacks["prefix"][node]
        result = cache[stack] = parts[::-1]
        return result

    samples = thread["samples"]
    times, weights = samples["time"], samples.get("weight") or [1] * samples["length"]
    cpu = samples.get("threadCPUDelta") or [0] * samples["length"]
    interval = profile["meta"]["interval"]
    root = thread.get("name") or f"thread {thread.get('tid')}"
    for i in range(samples["length"]):
        stack = samples["stack"][i]
        if stack is None:
            continue
        if weighting == "cpu":
            # CPU microseconds since this thread's previous sample, spent on
            # the way to this stack.
            micros = int(cpu[i] or 0)
            folded_key = ";".join([root] + path(stack))
            if micros > 0:
                folded[folded_key] += micros
            continue
        if i + 1 < samples["length"]:
            span_ms = times[i + 1] - times[i]
        else:
            span_ms = interval * weights[i]
        micros = int(round(span_ms * 1000))
        if micros > 0:
            folded[";".join([root] + path(stack))] += micros

out_path.write_text("".join(f"{stack} {count}\n" for stack, count in sorted(folded.items())))
print(f"{out_path}: {len(folded)} stacks, {sum(folded.values()) / 1e6:.2f} s of thread time")
