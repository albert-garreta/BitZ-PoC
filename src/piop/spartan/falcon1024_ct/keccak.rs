const RATE: usize = 136;
const ROUNDS: usize = 24;

pub(crate) const ROUND_CONSTANTS: [u64; ROUNDS] = [
    0x0000_0000_0000_0001,
    0x0000_0000_0000_8082,
    0x8000_0000_0000_808a,
    0x8000_0000_8000_8000,
    0x0000_0000_0000_808b,
    0x0000_0000_8000_0001,
    0x8000_0000_8000_8081,
    0x8000_0000_0000_8009,
    0x0000_0000_0000_008a,
    0x0000_0000_0000_0088,
    0x0000_0000_8000_8009,
    0x0000_0000_8000_000a,
    0x0000_0000_8000_808b,
    0x8000_0000_0000_008b,
    0x8000_0000_0000_8089,
    0x8000_0000_0000_8003,
    0x8000_0000_0000_8002,
    0x8000_0000_0000_0080,
    0x0000_0000_0000_800a,
    0x8000_0000_8000_000a,
    0x8000_0000_8000_8081,
    0x8000_0000_0000_8080,
    0x0000_0000_8000_0001,
    0x8000_0000_8000_8008,
];

const ROTATION: [[u32; 5]; 5] = [
    [0, 36, 3, 41, 18],
    [1, 44, 10, 45, 2],
    [62, 6, 43, 15, 61],
    [28, 55, 25, 21, 56],
    [27, 20, 39, 8, 14],
];

/// The nonlinear witness of SHAKE256.  `chi_ands` is ordered by
/// permutation, round, `y`, then `x`; each word is
/// `(!B[x+1,y]) & B[x+2,y]`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct KeccakTrace {
    /// Native inputs to each permutation, used by the independent trace checker.
    /// These checkpoints are reconstructed from public inputs and prior states
    /// by the proof and are not separate committed source bits.
    pub permutation_inputs: Vec<[u64; 25]>,
    /// The `B = \rho(\pi(\theta(A)))` lanes entering every chi layer, in
    /// permutation/round/`y`/`x` order.
    pub chi_inputs: Vec<u64>,
    pub chi_ands: Vec<u64>,
    /// State lanes after chi and iota, in the same order as `chi_inputs`.
    /// Committing these checkpoints keeps the theta/rho/pi wiring sparse.
    pub round_states: Vec<u64>,
    /// Theta column parity words, in permutation/round/`x` order.
    pub column_parities: Vec<u64>,
    /// Two-bit quotients in `sum_y A[x,y,bit] = C[x,bit] + 2 * quotient`,
    /// in permutation/round/`x`/bit order.
    pub column_parity_quotients: Vec<u8>,
    /// For every bit of `chi_inputs`, the exact quotient in
    /// `A_bit + C_left + C_rotated_right = B_bit + 2 * quotient`.
    /// Every quotient fits one bit.
    pub parity_quotients: Vec<u8>,
    pub permutations: usize,
}

impl KeccakTrace {
    pub(super) fn validate_falcon_shape(&self) -> Result<(), super::FalconError> {
        if self.permutations != 20
            || self.permutation_inputs.len() != 20
            || self.chi_inputs.len() != 20 * ROUNDS * 25
            || self.chi_ands.len() != self.chi_inputs.len()
            || self.round_states.len() != self.chi_inputs.len()
            || self.column_parities.len() != 20 * ROUNDS * 5
            || self.column_parity_quotients.len() != self.column_parities.len() * 64
            || self.parity_quotients.len() != self.chi_inputs.len() * 64
        {
            return Err(super::FalconError::ConstraintViolation {
                family: "keccak-shape",
                index: 0,
            });
        }
        for (index, &quotient) in self.column_parity_quotients.iter().enumerate() {
            if quotient > 2 {
                return Err(super::FalconError::ConstraintViolation {
                    family: "keccak-column-quotient-range",
                    index,
                });
            }
        }
        for (index, &quotient) in self.parity_quotients.iter().enumerate() {
            if quotient > 1 {
                return Err(super::FalconError::ConstraintViolation {
                    family: "keccak-theta-quotient-range",
                    index,
                });
            }
        }
        Ok(())
    }
}

#[inline]
const fn lane(x: usize, y: usize) -> usize {
    x + 5 * y
}

fn permutation(state: &mut [u64; 25], trace: &mut KeccakTrace) {
    trace.permutation_inputs.push(*state);
    for &round_constant in &ROUND_CONSTANTS {
        let round_input = *state;
        let c: [u64; 5] = core::array::from_fn(|x| {
            state[lane(x, 0)]
                ^ state[lane(x, 1)]
                ^ state[lane(x, 2)]
                ^ state[lane(x, 3)]
                ^ state[lane(x, 4)]
        });
        trace.column_parities.extend_from_slice(&c);
        for x in 0..5 {
            for bit_index in 0..64 {
                let sum: u8 = (0..5)
                    .map(|y| bit(round_input[lane(x, y)], bit_index))
                    .sum();
                trace
                    .column_parity_quotients
                    .push((sum - bit(c[x], bit_index)) / 2);
            }
        }
        let d: [u64; 5] = core::array::from_fn(|x| c[(x + 4) % 5] ^ c[(x + 1) % 5].rotate_left(1));
        for y in 0..5 {
            for x in 0..5 {
                state[lane(x, y)] ^= d[x];
            }
        }

        let mut b = [0u64; 25];
        for y in 0..5 {
            for x in 0..5 {
                b[lane(y, (2 * x + 3 * y) % 5)] = state[lane(x, y)].rotate_left(ROTATION[x][y]);
            }
        }

        for y in 0..5 {
            for x in 0..5 {
                let word = b[lane(x, y)];
                trace.chi_inputs.push(word);
                let (source_x, source_y) = rho_pi_source(x, y);
                let rotation = ROTATION[source_x][source_y] as usize;
                for destination_bit in 0..64 {
                    let source_bit = (destination_bit + 64 - rotation) & 63;
                    let rotated_bit = (source_bit + 63) & 63;
                    let sum = bit(round_input[lane(source_x, source_y)], source_bit)
                        + bit(c[(source_x + 4) % 5], source_bit)
                        + bit(c[(source_x + 1) % 5], rotated_bit);
                    let output = bit(word, destination_bit);
                    debug_assert!(sum >= output && (sum - output) & 1 == 0);
                    trace.parity_quotients.push((sum - output) / 2);
                }
            }
        }

        for y in 0..5 {
            for x in 0..5 {
                let z = (!b[lane((x + 1) % 5, y)]) & b[lane((x + 2) % 5, y)];
                trace.chi_ands.push(z);
                state[lane(x, y)] = b[lane(x, y)] ^ z;
            }
        }
        state[0] ^= round_constant;
        for y in 0..5 {
            for x in 0..5 {
                trace.round_states.push(state[lane(x, y)]);
            }
        }
    }
    trace.permutations += 1;
}

/// SHAKE256 with the exact Keccak-⁠f nonlinear trace required by the
/// circuit.  Falcon hashes `nonce || message`; for the benchmark's 72-byte
/// input and 2622-byte output this records exactly 20 permutations and
/// 12,000 64-bit chi witnesses.
pub fn shake256_with_trace(input: &[u8], output_len: usize) -> (Vec<u8>, KeccakTrace) {
    let mut state = [0u64; 25];
    let mut trace = KeccakTrace {
        permutation_inputs: Vec::new(),
        chi_inputs: Vec::new(),
        chi_ands: Vec::new(),
        round_states: Vec::new(),
        column_parities: Vec::new(),
        column_parity_quotients: Vec::new(),
        parity_quotients: Vec::new(),
        permutations: 0,
    };

    let mut chunks = input.chunks_exact(RATE);
    for block in &mut chunks {
        xor_block(&mut state, block);
        permutation(&mut state, &mut trace);
    }
    let tail = chunks.remainder();
    xor_block(&mut state, tail);
    xor_byte(&mut state, tail.len(), 0x1f);
    xor_byte(&mut state, RATE - 1, 0x80);
    permutation(&mut state, &mut trace);

    let mut output = Vec::with_capacity(output_len);
    while output.len() < output_len {
        let take = (output_len - output.len()).min(RATE);
        for offset in 0..take {
            output.push((state[offset / 8] >> (8 * (offset % 8))) as u8);
        }
        if output.len() < output_len {
            permutation(&mut state, &mut trace);
        }
    }
    (output, trace)
}

#[inline]
const fn bit(word: u64, index: usize) -> u8 {
    ((word >> index) & 1) as u8
}

/// Inverse of `(x,y) -> (y, 2x+3y mod 5)` used by rho/pi.
const fn rho_pi_source(destination_x: usize, destination_y: usize) -> (usize, usize) {
    let source_y = destination_x;
    let source_x = (3 * (destination_y + 5 - (3 * source_y) % 5)) % 5;
    (source_x, source_y)
}

fn xor_block(state: &mut [u64; 25], block: &[u8]) {
    for (offset, &byte) in block.iter().enumerate() {
        xor_byte(state, offset, byte);
    }
}

fn xor_byte(state: &mut [u64; 25], offset: usize, byte: u8) {
    state[offset / 8] ^= u64::from(byte) << (8 * (offset % 8));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shake256_known_answer() {
        let (digest, trace) = shake256_with_trace(b"", 64);
        assert_eq!(
            hex(&digest),
            "46b9dd2b0ba88d13233b3feb743eeb243fcd52ea62b81b82b50c27646ed5762f\
             d75dc4ddd8c0f200cb05019d67b592f6fc821c49479ab48640292eacb3b7c4be"
                .replace(char::is_whitespace, "")
        );
        assert_eq!(trace.permutations, 1);
        assert_eq!(trace.chi_ands.len(), 25 * 24);
        assert_eq!(trace.chi_inputs.len(), 25 * 24);
        assert_eq!(trace.round_states.len(), 25 * 24);
        assert_eq!(trace.permutation_inputs.len(), 1);
        assert_eq!(trace.column_parities.len(), 5 * 24);
        assert_eq!(trace.column_parity_quotients.len(), 5 * 24 * 64);
        assert!(trace.column_parity_quotients.iter().all(|&q| q <= 2));
        assert_eq!(trace.parity_quotients.len(), 25 * 24 * 64);
        assert!(trace.parity_quotients.iter().all(|&q| q <= 1));
    }

    #[test]
    fn falcon_shape_has_twenty_permutations() {
        let (_, trace) = shake256_with_trace(&[0u8; 72], 2 * 1_311);
        assert_eq!(trace.permutations, 20);
        assert_eq!(trace.chi_ands.len(), 12_000);
        assert_eq!(trace.chi_inputs.len(), 12_000);
        assert_eq!(trace.round_states.len(), 12_000);
        assert_eq!(trace.permutation_inputs.len(), 20);
        assert_eq!(trace.column_parities.len(), 2_400);
        assert_eq!(trace.column_parity_quotients.len(), 153_600);
        assert_eq!(trace.parity_quotients.len(), 768_000);
    }

    #[test]
    fn rho_pi_inverse_is_exact() {
        for y in 0..5 {
            for x in 0..5 {
                let destination = (y, (2 * x + 3 * y) % 5);
                assert_eq!(rho_pi_source(destination.0, destination.1), (x, y));
            }
        }
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
