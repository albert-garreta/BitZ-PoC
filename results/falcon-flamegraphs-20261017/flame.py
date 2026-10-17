"""Turn macOS `sample` reports into prover-only folded stacks and flamegraphs.

usage: python3 -I flame.py <samples dir> <out dir>

For each profile it keeps the stacks under the bench's marker frames
(`profile_commit_region` / `profile_prove_region`), drops the frames above
them, cleans Rust symbol names, and writes:
  <name>.folded        raw-sample counts, one stack per line
  <name>.svg           inferno flamegraph, widths in ms per prove
  <name>.json          d3-flame-graph tree, values in ms per prove
  summary.json         per-profile totals and top inclusive/self frames
"""

import collections
import csv
import io
import json
import re
import subprocess
import sys
from pathlib import Path

SAMPLES = Path(sys.argv[1])
OUT = Path(sys.argv[2])
OUT.mkdir(parents=True, exist_ok=True)

PROFILES = {
    "aerie": {
        "title": "Aerie v0.2.0 prover, Falcon-512, 32768 signatures, 1 thread",
        "markers": {"profile_prove_region": "Aerie prover"},
        "root": "Aerie prover",
    },
    "bitz-shipped": {
        "title": "BitZ prover (falcon-poc e140a3f3 as shipped), Falcon-512, 32768 signatures, 1 thread",
        "markers": {
            "profile_commit_region": "witness + commit (commit_from_s2)",
            "profile_prove_region": "prove",
        },
        "root": "BitZ prover",
    },
    "bitz-onelimb": {
        "title": "BitZ prover (per-site grinding + one-limb bridge), Falcon-512, 32768 signatures, 1 thread",
        "markers": {
            "profile_commit_region": "witness + commit (commit_from_s2)",
            "profile_prove_region": "prove",
        },
        "root": "BitZ prover",
    },
    "bitz-t16": {
        "title": "BitZ prover (keyed ring, one-limb bridge with 2^16 rows, per-site grinding), Falcon-512, 32768 signatures, 1 thread",
        "markers": {
            "profile_commit_region": "witness + commit (commit_from_s2)",
            "profile_prove_region": "prove",
        },
        "root": "BitZ prover",
    },
}

HASH = re.compile(r"::h[0-9a-f]{16}$")
# Non-leaf frames with no information beyond the call into their child:
# Rayon's scheduling and closure-call shims (at one thread, Rayon work runs
# inline), and the benches' own closures around the marker.
GLUE = re.compile(
    r"^<?(rayon|rayon_core|core::ops::function|core::iter|std::panicking|std::panic|"
    r"core::hint|std::hint|std::sys::backtrace)::|"
    r" as core::ops::function::Fn(Once|Mut)?\b|"
    r"^(falcon_algebraic::main|falcon::run_once|falcon_algebraic::in_profile_pool)\b"
)
IDLE_LEAF = re.compile(r"__psynch_cvwait|__semwait_signal|__ulock_wait|mach_msg|kevent|__workq_kernreturn")


def strip_generics(name: str) -> str:
    """Drop generic argument lists (`::<..>`, `Type<..>`); keep a leading `<..>` qualifier."""
    out, depth, index = [], 0, 0
    while index < len(name):
        char = name[index]
        if char == "<" and (depth > 0 or (index > 0 and (name[index - 1].isalnum() or name[index - 1] in "_:"))):
            depth += 1
            if depth == 1 and out and out[-1] == ":" and len(out) > 1 and out[-2] == ":":
                del out[-2:]  # the `::` of a turbofish
        elif char == ">" and depth > 0 and name[index - 1] != "-":
            depth -= 1
        elif depth == 0:
            out.append(char)
        index += 1
    return "".join(out)


def last(path: str) -> str:
    return path.strip().split("::")[-1]


def closing(text: str, open_index: int) -> int:
    """Index of the `>` matching the `<` at `open_index` (`->` is not a bracket)."""
    depth = 0
    for index in range(open_index, len(text)):
        if text[index] == "<":
            depth += 1
        elif text[index] == ">" and text[index - 1] != "-":
            depth -= 1
            if depth == 0:
                return index
    return len(text) - 1


LEGACY_IMPL = re.compile(r"(?:^|::)_?<impl ")


def bracket_depth(text: str) -> int:
    return sum(1 for c in text if c == "<") - sum(
        1 for i, c in enumerate(text) if c == ">" and (i == 0 or text[i - 1] != "-")
    )


def legacy_impl(name: str) -> str:
    """Legacy demangling `a::b::<impl T for U>::f` (`sample` writes `_<impl`) -> `<U as T>::f`."""
    for match in LEGACY_IMPL.finditer(name):
        open_index = name.index("<impl ", match.start())
        if bracket_depth(name[:open_index]) != 0:
            continue
        end = closing(name, open_index)
        inner = name[open_index + len("<impl ") : end]
        level, split = 0, None
        for j, c in enumerate(inner):
            level += c == "<"
            level -= c == ">" and inner[j - 1] != "-"
            if level == 0 and inner.startswith(" for ", j):
                split = j
                break
        head = f"<{inner}>" if split is None else f"<{inner[split + 5:]} as {inner[:split]}>"
        return head + name[end + 1 :]
    return name


def short(name: str) -> str:
    """`<a::b::Type as c::Trait>::f::{closure#0}` -> `<Type as Trait>::f::{closure}`."""
    if name == "<deduplicated_symbol>":
        return "(linker-folded function)"
    name = legacy_impl(name)
    name = re.sub(r"_?\{\{closure\}\}|\{closure#\d+\}", "{closure}", name)
    name = strip_generics(name)
    if name.startswith("<"):
        depth = 0
        for end, char in enumerate(name):
            depth += char == "<"
            depth -= char == ">"
            if depth == 0:
                break
        qualifier, rest = name[1:end], name[end + 1 :]
        if " as " in qualifier:
            ty, trait = qualifier.split(" as ", 1)
            head = f"<{last(ty)} as {last(trait)}>"
        else:
            head = last(qualifier)
        return head + rest
    parts = name.split("::")
    named = [i for i, part in enumerate(parts) if not part.startswith("{")]
    keep = named[-2] if len(named) >= 2 else 0
    return "::".join(parts[keep:])


def clean(frame: str) -> str:
    # inferno-collapse-sample writes image`symbol; keep the symbol.
    name = frame.split("`", 1)[1] if "`" in frame else frame
    return HASH.sub("", name.strip())


def fold(path: Path) -> list[tuple[list[str], int]]:
    if path.suffix == ".folded":  # samply2folded.py output (wall-time weighted)
        collapsed = path.read_text()
    else:
        collapsed = subprocess.run(
            ["inferno-collapse-sample", str(path)], capture_output=True, text=True, check=True
        ).stdout
    stacks = []
    for line in collapsed.splitlines():
        stack, _, count = line.rpartition(" ")
        stacks.append((stack.split(";"), int(count)))
    # Rust v0 symbols are not demangled by `sample`. Demangle frame by frame
    # (after splitting: demangled names may contain `;`, as in `[u64; 2]`).
    unique = sorted({frame for frames, _ in stacks for frame in frames})
    demangled = subprocess.run(
        ["rustfilt"], input="\n".join(unique) + "\n", capture_output=True, text=True, check=True
    ).stdout.splitlines()
    assert len(demangled) == len(unique)
    names = dict(zip(unique, demangled))
    return [([names[frame] for frame in frames], count) for frames, count in stacks]


def select(stacks, markers, root):
    kept = collections.Counter()
    idle = 0
    for frames, count in stacks:
        hit = None
        for index, frame in enumerate(frames):
            for marker, label in markers.items():
                if marker in frame:
                    hit = (index, label)
            if hit:
                break
        if hit is None:
            continue
        index, label = hit
        tail = [clean(frame) for frame in frames[index + 1 :]]
        if tail and IDLE_LEAF.search(tail[-1]):
            idle += count
        # Glue frames go everywhere, leaves included: with LTO a parallel
        # closure is inlined into Rayon's helper, so its self time belongs to
        # the caller of the parallel iterator.
        # A linker-folded body (identical code shared by several symbols) has
        # no name; between two named frames it is only a call, so drop it there.
        tail = [
            short(frame).replace(";", ",")
            for i, frame in enumerate(tail)
            if not GLUE.search(frame) and not (frame == "<deduplicated_symbol>" and i < len(tail) - 1)
        ]
        # Collapse direct self-recursion (e.g. a recursive fold) into one frame.
        tail = [frame for i, frame in enumerate(tail) if i == 0 or frame != tail[i - 1]]
        path = [root] + ([label] if len(markers) > 1 else []) + tail
        kept[";".join(path)] += count
    return kept, idle


def measured_ms(name: str, suffix: str = ".out") -> list[float]:
    out = SAMPLES / f"{name}{suffix}"
    if not out.exists():
        return []
    if name == "aerie":
        rows, header = [], None
        for line in out.read_text().splitlines():
            if line.startswith("records\t"):
                header = line.split("\t")
            elif header and line[:1].isdigit():
                rows.append(dict(zip(header, line.split("\t"))))
        return [float(row["prove_ms"]) for row in rows]
    rows = [json.loads(line) for line in out.read_text().splitlines() if line.startswith("{")]
    return [row["total_prover_ms"] for row in rows if row.get("trial") in ("sample", "warmup")]


def tree(folded: dict[str, int], scale: float) -> dict:
    root = {"name": None, "value": 0, "children": {}}
    for stack, count in folded.items():
        node = root
        node["value"] += count
        for frame in stack.split(";"):
            child = node["children"].setdefault(frame, {"name": frame, "value": 0, "children": {}})
            child["value"] += count
            node = child

    def finish(node):
        return {
            "name": node["name"],
            "value": round(node["value"] * scale, 3),
            "children": [finish(child) for child in sorted(node["children"].values(), key=lambda c: -c["value"])],
        }

    (only,) = root["children"].values()
    return finish(only)


summary = {}
for name, spec in PROFILES.items():
    report = SAMPLES / f"{name}.samply.folded"
    if not report.exists():
        report = SAMPLES / f"{name}.sample.txt"
    if not report.exists():
        continue
    folded, idle = select(fold(report), spec["markers"], spec["root"])
    total = sum(folded.values())
    proves = measured_ms(name)  # wall times of the sampled proves
    unprofiled = measured_ms(name, ".clock.out")  # same setup, no sampler
    # Sampling stretches wall time; widths use the unprofiled per-prove time.
    per_prove_ms = sum(unprofiled or proves) / len(unprofiled or proves)
    # The samples cover len(proves) proves: a frame's count / total is its
    # share of one prove.
    scale = per_prove_ms / total
    (OUT / f"{name}.folded").write_text("".join(f"{k} {v}\n" for k, v in sorted(folded.items())))
    subprocess.run(
        [
            "inferno-flamegraph",
            "--title", spec["title"],
            "--subtitle",
            (
                f"{per_prove_ms:.0f} ms per prove unprofiled; samply at 250 Hz over {len(proves)} proves, "
                "weighted by thread CPU time; widths in ms per prove"
                if report.suffix == ".folded"
                else f"{per_prove_ms:.0f} ms per prove unprofiled; {total} samples over {len(proves)} proves "
                "(macOS sample, 1 ms); widths scaled to ms per prove"
            ),
            "--countname", "ms",
            "--factor", f"{scale:.6f}",
            "--hash",
            "--width", "1800",
            "--minwidth", "0.05",
            "--fonttype", "Helvetica",
            str(OUT / f"{name}.folded"),
        ],
        stdout=(OUT / f"{name}.svg").open("w"),
        check=True,
    )
    (OUT / f"{name}.json").write_text(json.dumps(tree(folded, scale)))
    inclusive = collections.Counter()
    exclusive = collections.Counter()
    for stack, count in folded.items():
        frames = stack.split(";")
        for frame in set(frames[1:]):
            inclusive[frame] += count
        exclusive[frames[-1]] += count
    summary[name] = {
        "title": spec["title"],
        "samples": total,
        "idle_samples": idle,
        "profiled_proves_ms": proves,
        "unprofiled_proves_ms": unprofiled,
        "ms_per_prove": per_prove_ms,
        "samples_per_prove": total / len(proves),
        "top_inclusive": [(f, round(c * scale, 1)) for f, c in inclusive.most_common(40)],
        "top_self": [(f, round(c * scale, 1)) for f, c in exclusive.most_common(25)],
    }
    print(
        f"{name}: {total} samples ({idle} idle) over {len(proves)} proves; "
        f"profiled {sum(proves) / len(proves):.0f} ms/prove, unprofiled {per_prove_ms:.0f} ms/prove"
    )

(OUT / "summary.json").write_text(json.dumps(summary, indent=1))
