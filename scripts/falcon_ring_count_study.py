#!/usr/bin/env python3
"""Reproduce the Falcon arithmetic count study and check candidate identities.

Standard library only. This is an algebra/count prototype, not a SNARK prover.
The NTT uses Goldilocks solely as a convenient test field, not a security choice.
Run: python3 scripts/falcon_ring_count_study.py > /tmp/falcon-ring-study.json
"""

import hashlib
import json
from math import isqrt
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
N, M, Q, BETA2 = 1024, 1311, 12289, 70265242
FP = 2**64 - 2**32 + 1

# Each pair is (disjoint semantic values, committed bits). Public word grouping
# is conventional: one + message bytes + header/nonce bytes + s2 coefficients.
SOURCE = {
    "public_copies": (1098, 12873),
    "s2_canonical_slack": (N, 4 * N),
    "shake_words": (M, 16 * M),
    "division_quotients": (M, 3 * M),
    "division_remainders": (M, 14 * M),
    "remainder_slack": (M, 14 * M),
    "quotient_slack": (M, 3 * M),
    "reject": (M, M),
    "prefix": (M + 1, 11 * (M + 1)),
    "selector": (M, M),
    "selected_prefix": (M, 11 * M),
    "selected_remainder": (M, 14 * M),
    "hash_point": (N, 14 * N),
    "biased_s1": (N, 14 * N),
    "s1_range_slack": (N, 14 * N),
    "ring_quotient": (N, 23 * N),
    "norm_slack": (1, 27),
}
LINEAR = {
    "public": 1834, "s2_canonical": N, "division": M,
    "remainder_range": M, "quotient_range": M,
    "prefix_and_boundaries": M + 2, "s1_range": N, "ring": N,
}


def pad(n):
    return 1 << (n - 1).bit_length()


def decrease(old, new):
    return round(100 * (old - new) / old, 6)


def counts(removed=(), linear_removed=0, outer=4 * M, leaf=0,
           outer_degree=3, leaf_degree=0, quotient_width=23):
    values = sum(v for name, (v, _) in SOURCE.items() if name not in removed)
    bits = sum(b for name, (_, b) in SOURCE.items() if name not in removed)
    bits -= N * (23 - quotient_width)
    linear = sum(LINEAR.values()) - linear_removed
    # Outer padding is DENSE for proposed mixed-degree families. Current code
    # uses family blocks, which must change to achieve this proposed padding.
    total = linear + outer + leaf + 4094 + 1 + 1
    return {
        "semantic_source_values": values, "auxiliary_source_values": values - 1098,
        "committed_bits_live": bits, "committed_bits_padded": pad(bits),
        "committed_live_decrease_pct": decrease(198935, bits),
        "committed_padded_decrease_pct": decrease(262144, pad(bits)),
        "linear_live": linear, "linear_padded": pad(linear),
        "outer_live": outer, "outer_padded_dense": pad(outer),
        "outer_live_decrease_pct": decrease(5244, outer),
        "outer_padded_decrease_pct": decrease(8192, pad(outer)),
        "outer_sumcheck_round_degree": outer_degree,
        "additional_leaf_terms_live": leaf,
        "additional_leaf_terms_padded": pad(leaf) if leaf else 0,
        "additional_leaf_sumcheck_round_degree": leaf_degree,
        "mixed_relation_inventory_including_leaf_norm_forest": total,
        "mixed_inventory_decrease_pct": decrease(19492, total),
    }


def decode_words(payload, width, signed=False):
    bits = "".join(f"{b:08b}" for b in payload)
    words = [int(bits[i:i + width], 2) for i in range(0, len(bits), width)]
    return [w - (1 << width) if signed and w >> (width - 1) else w for w in words]


def fixture_trace():
    fixtures = ROOT / "src/piop/spartan/falcon1024_ct/fixtures"
    pk = (fixtures / "public_key.bin").read_bytes()
    sig = (fixtures / "signature_ct.bin").read_bytes()
    msg = (fixtures / "message.bin").read_bytes()
    assert pk[0] == 0x0A and sig[0] == 0x5A and len(msg) == 32
    h, s2 = decode_words(pk[1:], 14), decode_words(sig[41:], 12, signed=True)
    assert len(h) == len(s2) == N and max(h) < Q and -2048 not in s2
    shake = hashlib.shake_256(sig[1:41] + msg).digest(2 * M)
    w = [int.from_bytes(shake[2*i:2*i+2], "big") for i in range(M)]
    q, r = [v // Q for v in w], [v % Q for v in w]
    d = [int(v == 5) for v in q]
    prefix = [0]
    for rejected in d:
        prefix.append(prefix[-1] + 1 - rejected)
    e = [(1 - d[i]) * (1 - (prefix[i] >> 10)) for i in range(M)]
    u, v = [e[i] * prefix[i] for i in range(M)], [e[i] * r[i] for i in range(M)]
    c = [r[i] for i in range(M) if e[i]]
    assert len(c) == N and 1024 <= prefix[-1] < 2048
    full = [0] * (2 * N)
    for i, a in enumerate(h):
        for j, b in enumerate(s2):
            full[i + j] += a * b
    conv = [full[i] - full[i + N] for i in range(N)]
    s1 = [(c[i] - conv[i] + Q // 2) % Q - Q // 2 for i in range(N)]
    k = [(c[i] - conv[i] - s1[i]) // Q for i in range(N)]
    norm = sum(a*a + b*b for a, b in zip(s1, s2))
    assert norm <= BETA2
    return locals()


def ntt(values, inverse=False):
    a, n = [v % FP for v in values], len(values)
    assert n == pad(n) and (FP - 1) % n == 0
    j = 0
    for i in range(1, n):
        bit = n >> 1
        while j & bit:
            j ^= bit
            bit >>= 1
        j ^= bit
        if i < j:
            a[i], a[j] = a[j], a[i]
    length = 2
    while length <= n:
        root = pow(7, (FP - 1) // length, FP)
        if inverse:
            root = pow(root, -1, FP)
        for start in range(0, n, length):
            w = 1
            for i in range(start, start + length // 2):
                x, y = a[i], a[i + length // 2] * w % FP
                a[i], a[i + length // 2] = (x + y) % FP, (x - y) % FP
                w = w * root % FP
        length *= 2
    if inverse:
        inv_n = pow(n, -1, FP)
        a = [v * inv_n % FP for v in a]
    return a


def ideal_residual(a, b, c):
    size = 2048
    coefficients = [ntt(t + [0] * (size - len(t)), True) for t in (a, b, c)]
    aa, bb = [ntt(t + [0] * size) for t in coefficients[:2]]
    product = ntt([x * y % FP for x, y in zip(aa, bb)], True)
    for i, value in enumerate(coefficients[2]):
        product[i] = (product[i] - value) % FP
    remainder = [(product[i] + product[i + size]) % FP for i in range(size)]
    return product, remainder


def validate(t):
    # Exhaust the entire 14-bit input domain for the slack-free range predicate.
    for x in range(1 << 14):
        product = ((x >> 13) & 1) * ((x >> 12) & 1) * (x & 4095)
        assert (product == 0) == (x <= 12288)
    # Even without enforcing the tighter remainder bound, q>=6 is impossible.
    assert all(Q*q + r > 65535 for q in (6, 7) for r in range(1 << 14))
    coefficient_bound = (N * (Q - 1) * 2047 + 18432) // Q
    norm_bound = (isqrt(N * (Q - 1)**2 * BETA2) + 18432) // Q
    assert coefficient_bound == 2095958 < (1 << 21) - 1
    assert norm_bound == 268217 < (1 << 19) - 1
    assert max(abs(v) for v in t["k"]) < 1 << 19
    # Exact Z[X] ring residual lies in (X^1024+1), keeping Q*K.
    residual = [-v for v in t["full"]]
    for i in range(N):
        residual[i] += t["c"][i] - t["s1"][i] - Q * t["k"][i]
    assert all(residual[i] == residual[i + N] for i in range(N))
    # The packed prefix recurrence, including its boundary corrections.
    prefix_poly = [0] * (M + 2)
    for i, value in enumerate(t["prefix"]):
        prefix_poly[i] += value
        prefix_poly[i + 1] -= value
    prefix_poly[0] -= t["prefix"][0]
    prefix_poly[-1] += t["prefix"][-1]
    for i, rejected in enumerate(t["d"]):
        prefix_poly[i + 1] += rejected - 1
    assert not any(prefix_poly)
    families = [
        ([(q >> 2) & 1 for q in t["q"]], [q & 1 for q in t["q"]], t["d"]),
        ([1-d for d in t["d"]], [1-(p >> 10) for p in t["prefix"][:-1]], t["e"]),
        (t["e"], t["prefix"][:-1], t["u"]),
        (t["e"], t["r"], t["v"]),
    ]
    for a, b, c in families:
        _, rem = ideal_residual(a, b, c)
        assert not any(rem)
        bad = c.copy()
        bad[37] += 1
        _, rem = ideal_residual(a, b, bad)
        assert any(rem), "altered row must fail ideal membership"
    # Verify virtual leaves pointwise and under non-Boolean MLE evaluation.
    gamma, rho = 123456789, 987654321
    old = [(1 + e*(gamma-1) + rho*u + v) % FP
           for e, u, v in zip(t["e"], t["u"], t["v"])]
    virtual = [(1 + (1-d)*(1-(p >> 10))*(gamma-1+rho*p+r)) % FP
               for d, p, r in zip(t["d"], t["prefix"], t["r"])]
    assert old == virtual
    weights = [1]
    for challenge in range(100, 111):
        weights = [v*(1-challenge) % FP for v in weights] + [v*challenge % FP for v in weights]
    weighted_affine = [weights[i]*(gamma-1+rho*t["prefix"][i]+t["r"][i]) % FP for i in range(M)]
    direct = sum(weights[i]*(old[i]-1) for i in range(M)) % FP
    assert direct == sum(e*v for e, v in zip(t["e"], weighted_affine)) % FP
    assert direct == sum((1-t["d"][i])*(1-(t["prefix"][i] >> 10))*weighted_affine[i] for i in range(M)) % FP
    # Required terminal linear binding of the weighted operand, at a fresh point.
    fresh = [1]
    for challenge in range(200, 211):
        fresh = [v*(1-challenge) % FP for v in fresh] + [v*challenge % FP for v in fresh]
    endpoint = sum(fresh[i]*weighted_affine[i] for i in range(M)) % FP
    bound = sum(fresh[i]*weights[i]*(gamma-1+rho*t["prefix"][i]+t["r"][i]) for i in range(M)) % FP
    assert endpoint == bound
    return {"fixture_norm": t["norm"], "max_abs_ring_quotient": max(map(abs, t["k"])),
            "universal_coefficient_quotient_bound": coefficient_bound,
            "universal_norm_quotient_bound": norm_bound,
            "range_inputs_exhausted": 16384, "impossible_division_pairs_checked": 32768,
            "crt_families_checked": 4, "mutated_crt_families_rejected": 4,
            "virtual_leaf_and_terminal_linear_identities": "pass",
            "ring_and_prefix_polynomial_identities": "pass"}


def main():
    assert sum(b for _, b in SOURCE.values()) == 198935
    assert sum(v for v, _ in SOURCE.values()) == 19330
    assert sum(LINEAR.values()) == 10152
    uv = ("selected_prefix", "selected_remainder")
    uv_e = uv + ("selector",)
    ranges = ("quotient_slack", "remainder_slack", "s1_range_slack")
    removed_linear = 2 * M + N
    scenarios = {
        "baseline": counts(),
        "virtual_uv": counts(uv, outer=2*M, leaf=M, leaf_degree=2),
        "virtual_uv_selector": counts(uv_e, outer=M, leaf=M, leaf_degree=3),
        "virtual_uv_and_ranges": counts(uv+ranges, removed_linear, 3*M+N, M, 4, 2),
        "virtual_uv_selector_and_ranges": counts(uv_e+ranges, removed_linear, 2*M+N, M, 4, 3),
        "plus_public_canonical_slack": counts(uv_e+ranges+("s2_canonical_slack",), removed_linear+N, 2*M+N, M, 4, 3),
        "plus_20bit_ring_quotient": counts(uv_e+ranges+("s2_canonical_slack",), removed_linear+N, 2*M+N, M, 4, 3, 20),
    }
    code = ROOT / "src/piop/spartan/falcon1024_ct/layout.rs"
    print(json.dumps({"scope": "Falcon1024 hybrid; one live signature; proposals only",
                      "layout_sha256": hashlib.sha256(code.read_bytes()).hexdigest(),
                      "source_components_values_bits": SOURCE, "linear_components": LINEAR,
                      "scenarios": scenarios, "validation": validate(fixture_trace())}, indent=2))


if __name__ == "__main__":
    main()
