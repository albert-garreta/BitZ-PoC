//! FEASIBILITY PROBE (throwaway): what our vendored `crates/circuit` derives
//! for the oracle's SHA-256 statements, in the oracle probe's own units —
//! the constraint-matrix counts and the upstream `bitz/spartan/constraint-
//! matrices/v1` SHA-256 digest (coefficients lowered to canonical Q100
//! residues, 16 LE bytes each), and the materialised `M^T`'s counts and the
//! upstream `bitz/virtual-map/csc/v1` blake3 digest — so the two can be
//! diffed line by line against `e2e_probe` on the oracle branch.
//!
//! `bitz_circuit_probe <sha256-compression|sha256-chain> <blocks> <seed>`
//! The blocks derive from the seed exactly as the oracle probe's do
//! (SplitMix64, 8 bytes per output, little-endian).
use std::time::Instant;

use circuit::Circuit;
use circuit::constraints::ConstraintGenerator;
use circuit::matrix_transpose::MTransposeGenerator;
use circuit::sha256::{INITIAL_STATE, Word, compress};
use circuit::witgen::Witgen;
use num_bigint::BigInt;
use num_traits::{Signed, ToPrimitive};
use sha2::{Digest, Sha256};

const Q100: u128 = (1u128 << 100) - 15;

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// FIPS 180-4 compression, one block (no padding), for the expected chaining value.
fn sha256_compress(state: &mut [u32; 8], block: &[u8; 64]) {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut w = [0u32; 64];
    for i in 0..16 {
        w[i] = u32::from_be_bytes([block[4 * i], block[4 * i + 1], block[4 * i + 2], block[4 * i + 3]]);
    }
    for i in 16..64 {
        let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
        let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
    }
    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;
    for i in 0..64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ (!e & g);
        let t1 = h.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let t2 = s0.wrapping_add(maj);
        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(t1);
        d = c;
        c = b;
        b = a;
        a = t1.wrapping_add(t2);
    }
    for (s, v) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
        *s = s.wrapping_add(v);
    }
}

struct Statement {
    blocks: Vec<[u32; 16]>,
    digest: [u32; 8],
}

impl Statement {
    fn seeded(blocks: usize, seed: u64) -> Self {
        let mut state = seed;
        let mut digest = INITIAL_STATE;
        let blocks = (0..blocks)
            .map(|_| {
                let mut bytes = [0u8; 64];
                for chunk in bytes.chunks_exact_mut(8) {
                    chunk.copy_from_slice(&splitmix64(&mut state).to_le_bytes());
                }
                sha256_compress(&mut digest, &bytes);
                std::array::from_fn(|i| u32::from_be_bytes(std::array::from_fn(|j| bytes[4 * i + j])))
            })
            .collect();
        Self { blocks, digest }
    }

    fn input(&self) -> Vec<bool> {
        self.blocks
            .iter()
            .flatten()
            .flat_map(|word| (0..32).map(move |bit| word >> bit & 1 != 0))
            .collect()
    }

    /// Their `Sha256Statement::synthesize` (d6b637e) against our vendored trait
    /// (`f2z` where theirs says `bitz`).
    fn synthesize<CS: Circuit>(&self, cs: &mut CS, inputs: &[CS::Bool]) {
        assert_eq!(inputs.len(), self.blocks.len() * 512);
        for (bit, expected) in inputs.iter().zip(self.input()) {
            constrain_bit(cs, bit.clone(), expected);
        }
        let mut state = INITIAL_STATE.map(|word| Word::constant(u64::from(word)));
        for bits in inputs.chunks_exact(512) {
            let block = std::array::from_fn(|word| {
                Word::new(std::array::from_fn(|bit| bits[word * 32 + bit].clone()))
            });
            state = compress(cs, block, state).map(|value| value.word);
        }
        for (word, expected) in state.iter().zip(self.digest) {
            for bit in 0..32 {
                constrain_bit(cs, word.bit(bit), expected >> bit & 1 != 0);
            }
        }
    }
}

fn constrain_bit<CS: Circuit>(cs: &mut CS, bit: CS::Bool, expected: bool) {
    let value = cs.f2z::<1>(bit);
    let expected = CS::Z::<1>::from(CS::Coefficient::<1>::from(u64::from(expected)));
    cs.assert_r1c::<1>(
        CS::Z::<1>::from(CS::Coefficient::<1>::from(1u64)),
        value,
        expected,
    );
}

/// Their `bigint_to_fq`, then the 16-byte LE wire form.
fn residue_bytes(value: &BigInt) -> [u8; 16] {
    let modulus = BigInt::from(Q100);
    let mut reduced = value % &modulus;
    if reduced.is_negative() {
        reduced += modulus;
    }
    reduced.to_u128().expect("canonical residue fits").to_le_bytes()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let blocks: usize = args[1].parse().expect("blocks");
    let seed: u64 = args[2].parse().expect("seed");
    if args[0] == "sha256-compression" {
        assert_eq!(blocks, 1);
    }
    let statement = Statement::seeded(blocks, seed);
    let inputs = statement.input();
    println!(
        "circuit={} blocks={blocks} seed={seed} input_bits={} digest_word0={:08x}",
        args[0],
        inputs.len(),
        statement.digest[0]
    );

    let started = Instant::now();
    let mut constraints = ConstraintGenerator::new(inputs.len());
    let symbolic: Vec<_> = (0..inputs.len()).map(|i| constraints.input(i)).collect();
    statement.synthesize(&mut constraints, &symbolic);
    let integer = constraints.into_matrices();
    let t_gen = started.elapsed();
    let nnz = |m: &circuit::constraints::SparseMatrix<BigInt>| -> usize {
        m.rows().iter().map(|r| r.entries().len()).sum()
    };
    let m_nnz: usize = integer.m.rows().iter().map(|r| r.positions().len()).sum();
    println!(
        "M: rows={} cols={} nnz={}  A: rows={} cols={} nnz={}  B: nnz={}  C: nnz={}  (gen {:.1} ms)",
        integer.m.row_count(),
        integer.m.column_count(),
        m_nnz,
        integer.a.row_count(),
        integer.a.column_count(),
        nnz(&integer.a),
        nnz(&integer.b),
        nnz(&integer.c),
        t_gen.as_secs_f64() * 1e3
    );

    // Their `constraint_matrix_digest` (crates/spartan/src/matrix.rs:416).
    let started = Instant::now();
    let mut hash = Sha256::new();
    let put = |hash: &mut Sha256, v: usize| hash.update((v as u64).to_le_bytes());
    hash.update(b"bitz/spartan/constraint-matrices/v1");
    hash.update(b"M");
    put(&mut hash, integer.m.row_count());
    put(&mut hash, integer.m.column_count());
    for row in integer.m.rows() {
        put(&mut hash, row.positions().len());
        for &column in row.positions() {
            put(&mut hash, column);
        }
    }
    for (label, matrix) in [(b"A", &integer.a), (b"B", &integer.b), (b"C", &integer.c)] {
        hash.update(label);
        put(&mut hash, matrix.row_count());
        put(&mut hash, matrix.column_count());
        for row in matrix.rows() {
            put(&mut hash, row.entries().len());
            for (column, coefficient) in row.entries() {
                put(&mut hash, *column);
                let bytes = residue_bytes(coefficient);
                put(&mut hash, bytes.len());
                hash.update(bytes);
            }
        }
    }
    let digest: [u8; 32] = hash.finalize().into();
    let rows = integer.a.row_count();
    let cols = integer.a.column_count();
    println!(
        "constraint_digest={} num_row_vars={} num_column_vars={} (digest {:.1} ms)",
        hex(&digest),
        rows.max(1).next_power_of_two().ilog2(),
        cols.max(1).next_power_of_two().ilog2(),
        started.elapsed().as_secs_f64() * 1e3
    );

    // Their `MaterializedMTranspose::digest` (crates/circuit/src/matrix_transpose.rs:387).
    let started = Instant::now();
    let mut generator = MTransposeGenerator::new(inputs.len());
    let map_inputs = generator.take_inputs();
    statement.synthesize(&mut generator, &map_inputs);
    let map = generator.finish();
    let (column_offsets, row_indices) = map.csc();
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"bitz/virtual-map/csc/v1");
    for count in [map.row_count(), map.column_count(), map.nonzero_count()] {
        hasher.update(&(count as u64).to_le_bytes());
    }
    for indices in [column_offsets, row_indices] {
        let bytes: Vec<u8> = indices.iter().flat_map(|index| index.to_le_bytes()).collect();
        hasher.update(&bytes);
    }
    println!(
        "map: h_len={} f_len={} nnz={} payload_bytes={} digest={} (gen {:.1} ms)",
        map.row_count(),
        map.column_count(),
        map.nonzero_count(),
        map.payload_bytes(),
        hex(hasher.finalize().as_bytes()),
        started.elapsed().as_secs_f64() * 1e3
    );
    assert_eq!(map.row_count(), integer.a.column_count());

    let started = Instant::now();
    let mut witgen = Witgen::with_inputs(&inputs);
    statement.synthesize(&mut witgen, &inputs);
    let (f, h) = witgen.into_witnesses();
    println!(
        "witness: f_bits={} h_bits={} satisfied={} (witgen {:.1} ms)",
        f.bit_len(),
        h.bit_len(),
        integer.is_satisfied(&f),
        started.elapsed().as_secs_f64() * 1e3
    );
}
