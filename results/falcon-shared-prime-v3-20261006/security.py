#!/usr/bin/env python3
"""Independent Falcon V3 ledger certificate; no dependencies or repository I/O.

Run: python3 /private/tmp/falcon_v3_security.py
All security inequalities use exact rational arithmetic. Reported security bits
use a floating-point logarithm only after the exact checks pass. This reproduces
the existing work-normalized grinding model, not unconditional statistical
security. Nonce-work expectations count ideal serial prefixes, excluding SIMD
overscan; they are not wall-clock predictions.
"""

from fractions import Fraction
from functools import cache
import json
from math import log2


def component_bits(numerator):
    return 0 if numerator == 0 else 8 + (numerator - 1).bit_length()


@cache
def compaction_bits(d):
    rounds = 2 * (11 + d) + 55
    cubic_degree, claim_degree = 3 * rounds, 10 * (d + 1) + 11
    uniform = component_bits(cubic_degree + claim_degree)
    limit = uniform + 2
    budget = Fraction(1, 256)
    best = (uniform, uniform)
    work = (rounds + 21) * 2**uniform
    for r in range(limit + 1):
        for c in range(limit + 1):
            error = Fraction(cubic_degree, 2**r) + Fraction(claim_degree, 2**c)
            candidate = rounds * 2**r + 21 * 2**c
            if error <= budget and candidate < work:
                best, work = (r, c), candidate
    return best


def ledger(batch, target):
    d = (batch - 1).bit_length()
    floor = {100: 114, 128: 125}[target]
    projection = {100: 2, 128: 14}[target]
    bits = component_bits if target == 128 else lambda _: 0
    cubic, claims = compaction_bits(d) if target == 128 else (0, 0)
    prime_terms = [
        (2 * d, bits(2 * d)),
        (4 * (10 + d), bits(4 * (10 + d))),
        (11 + d, bits(11 + d)),
        (3 * (2 * (11 + d) + 55), cubic),
        (10 * (d + 1) + 11, claims),
        (2048, bits(2048)),
        (13 + d + batch + 12, bits(13 + d + batch + 12)),
        (2 * (17 + d), bits(2 * (17 + d))),
        (20, projection),
    ]
    # Accepted-composite error; PCS quarter-budget; six binary components.
    bound = Fraction(1, 2**144) + Fraction(1, 2 ** (target + 2))
    bound += Fraction(6, 2 ** (target + 8))
    bound += sum((Fraction(n, 2 ** (floor + g)) for n, g in prime_terms), Fraction())
    return bound + Fraction(4 * d + 2049, 12289**11)


def main():
    report = {"model": "work-normalized grinding", "exact_cases_checked": 2048}
    report["minimum_security"] = []
    extension = 12289**11
    for target in (100, 128):
        worst, worst_batch = Fraction(), None
        for batch in range(1, 1025):
            bound = ledger(batch, target)
            assert bound <= Fraction(1, 2**target), (batch, target)
            if bound > worst:
                worst, worst_batch = bound, batch
            if target == 128:
                d = (batch - 1).bit_length()
                old = Fraction(d + 2046, extension - 12289) + Fraction(10, 2**137)
                ring = Fraction(4 * d + 2049, extension)
                assert ring + Fraction(20, 2**139) <= old, batch
                assert ring + Fraction(20, 2**138) > old, batch
        report["minimum_security"].append({
            "target": target,
            "first_worst_batch": worst_batch,
            "bits_approx": -log2(float(worst)),
            "exact_error_numerator": str(worst.numerator),
            "exact_error_denominator": str(worst.denominator),
        })
    report["projection_14_preserves_native_ring_bound_and_13_fails_cases"] = 1024

    # Batch1024 has d=10. Counts are challenge boundaries, not error degrees.
    rows = [
        ("instance and outer", 3, 24, 13),
        ("quadratic", 20, 26, 15),
        ("cubic and forest claims", 118, 28, 17),
        ("fingerprint and linear batching", 2, 30, 19),
        ("source binder and ring projection", 28, 25, 14),
    ]
    report["expected_128_batch1024_prime_prefixes"] = [
        dict(category=name, boundaries=count, v2_bits=old, v3_bits=new,
             v2_prefixes=count * 2**old, v3_prefixes=count * 2**new)
        for name, count, old, new in rows
    ]
    old = sum(count * 2**g for _, count, g, _ in rows)
    new = sum(count * 2**g for _, count, _, g in rows)
    assert sum(count for _, count, _, _ in rows) == 171
    assert old == 36_154_900_480 and new == 17_653_760 and old == 2048 * new
    # Retained PCS schedule:19folds,6query boundaries,5auxiliary boundaries.
    pcs = 4 * 2**26 + 3 * sum(2**g for g in (25, 23, 21, 19, 17))
    pcs += 6 * 2**11 + 5 * 2**7
    other_binary = 74 * 2**17 + 31 * 2**14 + 2**15 + 62 * 2**16
    bridges = {"v2": 274 * 2**18, "v3": 287 * 2**18}
    assert pcs == 402_535_040 and other_binary == 14_303_232
    report["expected_128_batch1024_totals"] = dict(
        v2_prime=old, v3_prime=new, prime_work_factor=old // new,
        native_prime=new - (2**14 - 2**12),
        unchanged_pcs=pcs, unchanged_other_binary=other_binary,
        bridge_prefixes=bridges,
        v2_all=old + pcs + other_binary + bridges["v2"],
        v3_all=new + pcs + other_binary + bridges["v3"],
    )
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
