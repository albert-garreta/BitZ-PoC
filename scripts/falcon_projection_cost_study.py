#!/usr/bin/env python3
"""Check shared-source projections and count ideal-packing costs.

Run: python3 scripts/falcon_projection_cost_study.py
This is an algebra/geometry study, not a prover or a timing benchmark.
"""

import json
from random import Random

from falcon_ring_count_study import FP, M, N, ROOT, decrease, fixture_trace, pad


def evaluate(coefficients, point, modulus=FP):
    result = 0
    for coefficient in reversed(coefficients):
        result = (result * point + coefficient) % modulus
    return result


def sample_address(sample, bit):
    # Mirrors hybrid_keccak.rs: numeric sample bits are LSB-first, while
    # Falcon reads each pair of SHAKE stream bytes as a big-endian word.
    byte = 2 * sample + int(bit < 8)
    permutation, offset = divmod(byte, 136)
    slab = int(permutation >= 16)
    local_permutation = permutation - (16 if slab else 0)
    return slab, local_permutation, 8 * offset + bit % 8


def check_shared_sample_view(trace):
    addresses = [sample_address(i, j) for i in range(M) for j in range(16)]
    assert len(set(addresses)) == 16 * M
    # A map of actual selected Keccak output bit positions, independently
    # populated from the SHAKE stream rather than from the numeric words.
    outputs = {}
    for byte_index, byte in enumerate(trace["shake"]):
        permutation, offset = divmod(byte_index, 136)
        slab = int(permutation >= 16)
        local_permutation = permutation - (16 if slab else 0)
        for bit in range(8):
            outputs[(slab, local_permutation, 8 * offset + bit)] = (byte >> bit) & 1
    copied = [(word >> bit) & 1 for word in trace["w"] for bit in range(16)]
    assert copied == [outputs[address] for address in addresses]
    rng = Random(855)
    prime_weights = [rng.randrange(1, FP) for _ in copied]
    binary_weights = [rng.getrandbits(128) for _ in copied]
    # Transpose routing of any terminal linear functional to its owning source.
    routed_prime = {}
    routed_binary = {}
    for address, pw, bw in zip(addresses, prime_weights, binary_weights):
        routed_prime[address] = (routed_prime.get(address, 0) + pw) % FP
        routed_binary[address] = routed_binary.get(address, 0) ^ bw
    prime_copy = sum(b*w for b, w in zip(copied, prime_weights)) % FP
    prime_alias = sum(outputs[a]*w for a, w in routed_prime.items()) % FP
    binary_copy = 0
    binary_alias = 0
    for bit, weight in zip(copied, binary_weights):
        if bit:
            binary_copy ^= weight
    for address, weight in routed_binary.items():
        if outputs[address]:
            binary_alias ^= weight
    assert prime_copy == prime_alias and binary_copy == binary_alias
    bad_outputs = dict(outputs)
    bad_outputs[addresses[17]] ^= 1
    assert sum(bad_outputs[a]*w for a, w in routed_prime.items()) % FP != prime_copy
    return {"checked_aliases": len(addresses),
            "slab_16_aliases": sum(a[0] == 0 for a in addresses),
            "slab_4_aliases": sum(a[0] == 1 for a in addresses),
            "prime_and_binary_linear_routing": "pass", "modified_sample_rejected": True}


def check_word_polynomial_division(trace):
    rng = Random(256)
    weights = [rng.randrange(1, FP) for _ in range(M)]
    residual = [0] * 16
    projected = 0
    alpha = 1234567
    for w, q, r, weight in zip(trace["w"], trace["q"], trace["r"], weights):
        wb = [(w >> j) & 1 for j in range(16)]
        qb = [(q >> j) & 1 for j in range(3)]
        rb = [(r >> j) & 1 for j in range(14)]
        assert evaluate(wb, 2) == 12289 * evaluate(qb, 2) + evaluate(rb, 2)
        for j in range(16):
            value = wb[j] - 12289 * (qb[j] if j < 3 else 0) - (rb[j] if j < 14 else 0)
            residual[j] = (residual[j] + weight * value) % FP
        projected += weight * (evaluate(wb, alpha) - 12289*evaluate(qb, alpha) - evaluate(rb, alpha))
    assert evaluate(residual, 2) == 0
    assert evaluate(residual, alpha) == projected % FP
    assert evaluate(residual, alpha) != 0, "projected residual is a known-sum target, not zero"
    bad = residual.copy()
    bad[0] = (bad[0] + weights[0]) % FP
    assert evaluate(bad, 2) != 0
    return {"rows_checked": M, "batched_residual_slots": len(residual),
            "ideal_membership_and_projection": "pass", "modified_word_rejected": True}


def certificate_cost(degree, ideal_degree, families=1):
    # Fixed-width 16-byte field representation; counts exclude framing,
    # sumcheck messages, oracle authentication and opening proofs.
    residual = degree + 1
    quotient = residual - ideal_degree
    return {"residual_field_slots": residual * families,
            "residual_bytes_per_proof": 16 * residual * families,
            "residual_bytes_per_signature_at_1024": residual * families / 64,
            "quotient_field_slots": quotient * families,
            "quotient_bytes_per_proof": 16 * quotient * families}


def poly_add(a, b):
    out = [0] * max(len(a), len(b))
    for i, value in enumerate(a):
        out[i] += value
    for i, value in enumerate(b):
        out[i] += value
    return out


def poly_mul(a, b):
    out = [0] * (len(a) + len(b) - 1)
    for i, x in enumerate(a):
        for j, y in enumerate(b):
            out[i+j] += x*y
    return out


def gf256_mul(a, b):
    result = 0
    while b:
        if b & 1:
            result ^= a
        b >>= 1
        a <<= 1
        if a & 256:
            a ^= 0x11B
    return result


def gf256_evaluate(poly, alpha=2):
    result = 0
    for coefficient in reversed(poly):
        result = gf256_mul(result, alpha) ^ (coefficient & 1)
    return result


def check_zinc_projected_tensor():
    """Toy check of Eq. (41)/Algorithm 4; no code commitment or IOPP.

    Both a prime query and a characteristic-two query use the same W. The
    binary extension has degree eight, not production security parameters.
    """
    rng = Random(2026855)
    width = 4
    witness = [[[rng.randrange(2) for _ in range(width)] for _ in range(4)] for _ in range(4)]
    field_claims = {}
    for binary in (False, True):
        if binary:
            # Equality weights are computed INSIDE the extension field first.
            def eq2(z0, z1):
                return [gf256_mul(z0 if i & 1 else z0 ^ 1,
                                  z1 if i & 2 else z1 ^ 1) for i in range(4)]
            u, v = eq2(17, 93), eq2(47, 123)
            lift = lambda x: [(x >> k) & 1 for k in range(8)]
            project = gf256_evaluate
            mul = gf256_mul
            add = lambda a, b: a ^ b
        else:
            u = [(17 if i & 1 else 1-17) * (93 if i & 2 else 1-93) % FP for i in range(4)]
            v = [(47 if i & 1 else 1-47) * (123 if i & 2 else 1-123) % FP for i in range(4)]
            lift = lambda x: [x]
            project = lambda p: evaluate(p, 53)
            mul = lambda a, b: a*b % FP
            add = lambda a, b: (a+b) % FP
        tensor = []
        claimed = 0
        for i in range(4):
            for j in range(4):
                tensor = poly_add(tensor, poly_mul(poly_mul(lift(u[i]), witness[i][j]), lift(v[j])))
                claimed = add(claimed, mul(mul(u[i], project(witness[i][j])), v[j]))
        assert project(tensor) == claimed
        # Now project the lifted weights to a fresh prime and evaluation point.
        # The witness polynomial axis remains until the final scalar check.
        xi = 7654321
        secondary_prime = (1 << 61) - 1
        reduced_tensor = [0] * width
        for i in range(4):
            for j in range(4):
                scale = (evaluate(lift(u[i]), xi, secondary_prime)
                         * evaluate(lift(v[j]), xi, secondary_prime)) % secondary_prime
                for k in range(width):
                    reduced_tensor[k] = (reduced_tensor[k] + scale*witness[i][j][k]) % secondary_prime
        assert evaluate(tensor, xi, secondary_prime) == evaluate(reduced_tensor, xi, secondary_prime)
        if binary:
            # Binary consistency alone misses addition of 2. Authentication
            # of the rational identity against the original W is essential.
            malicious_tensor = tensor.copy()
            malicious_tensor[0] += 2
            assert project(malicious_tensor) == claimed
            assert evaluate(malicious_tensor, xi, secondary_prime) != evaluate(reduced_tensor, xi, secondary_prime)
        field_claims["binary_extension" if binary else "prime"] = {
            "lifted_tensor_coefficients": len(tensor), "projection_check": "pass",
            "reprojection_consistency_check": "pass"}
    return {"shared_witness_shape": [4, 4, width], "claims": field_claims,
            "binary_invisible_integer_mutation_detected": True,
            "scope": "Algebra only; no commitment, proximity proof or security benchmark"}


def main():
    trace = fixture_trace()
    old = 198935
    samples = 16 * M
    profile_path = ROOT / "bench_results/falcon-prime-inner-20260926/goal-profile-b1024-summary.json"
    profile_summary = None
    if profile_path.is_file():
        profile = json.loads(profile_path.read_text())
        stages, grinding = profile["stages"], profile["grinding"]
        h2p = stages["falcon_arithmetic:hash_to_point_products"]
        profile_summary = {
            "h2p_products_including_grinding": h2p,
            "h2p_products_excluding_nested_grinding": h2p - grinding["falcon_arithmetic:hash_to_point_products"],
            "arithmetic_binding_including_ring_adjoint": stages["falcon_arithmetic:binding_inner"],
            "prime_to_binary_bridge": stages["falcon_hybrid:binary_bridge"],
            "joint_sumcheck": stages["falcon_hybrid:joint_sumcheck"],
            "note": "Existing diagnostic profile only; no proposed protocol timed",
        }
    per_signatures = []
    for bits in (old, 121058):
        per_signatures.append({"before_live_bits": bits, "after_live_bits": bits - samples,
                               "before_padded_bits": pad(bits), "after_padded_bits": pad(bits - samples)})
    print(json.dumps({
        "scope": "Proposed shared sample views and ideal packing; no production prover changes",
        "checks": {"sample_view": check_shared_sample_view(trace),
                   "word_polynomial_division": check_word_polynomial_division(trace),
                   "zinc_projected_tensor": check_zinc_projected_tensor()},
        "sample_copy": {"removed_bits": samples,
                        "arithmetic_live_decrease_pct": decrease(old, old - samples),
                        "hybrid_live_decrease_pct": decrease(1030955, 1030955 - samples),
                        "source_scenarios": per_signatures,
                        "wiring_equations_before": 32000 + samples, "wiring_equations_after": 32000,
                        "wiring_index_padded_before": pad(32000 + samples),
                        "wiring_index_padded_after": pad(32000),
                        "removed_equality_gather_terms": 2 * samples,
                        "added_routed_binary_sample_coefficients": samples,
                        "additional_simple_extracted_sample_bridge_slots": pad(samples)},
        "certificate_costs": {
            "word_division_common_batch": certificate_cost(15, 1),
            "ring_common_batch": certificate_cost(2*N-2, N),
            "crt_1311_common_batch": certificate_cost(2*M-2, M),
            "crt_1311_four_separate_families": certificate_cost(2*M-2, M, 4),
            "crt_2048_common_batch": certificate_cost(4094, 2048),
            "crt_2048_four_separate_families": certificate_cost(4094, 2048, 4)},
        "crt_work_model_per_signature": {
            "current_boolean_index_products": 4*M,
            "distinct_operand_views": 9,
            "scalar_entries_in_nine_unpadded_projections": 9*M,
            "schoolbook_coefficient_products_1311": 4*M*M,
            "schoolbook_coefficient_products_2048": 4*2048*2048,
            "degree_2047_operand_coefficient_bytes_nine_views": 9*2048*16,
            "note": "Schoolbook dense direct-backend model, not a lower bound or a timing prediction"},
        "profile_ms_per_signature": profile_summary,
    }, indent=2))


if __name__ == "__main__":
    main()
