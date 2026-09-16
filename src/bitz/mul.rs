//! The paper's native multiplication statements in their gadget language:
//! `N` independent products `x · y = z` over the integers, `x, y` of `W`
//! bits and `z` of `2W`, all three committed as bits — the operands as the
//! circuit's inputs, the product as a hint — and one rank-1 row per
//! product on their lifts (`Σ 2^i x_i · Σ 2^i y_i = Σ 2^i z_i`). Bits are
//! bits by construction on both sides of the map, so no range row exists:
//! `4W` committed bits and one row per multiplication, which is the
//! relation the paper's `u32`, `u64` and `u128` tables prove.
//!
//! Nothing about the operands is public (the count and the width are): the
//! operands derive from a seed (SplitMix64, as the SHA probes' blocks do),
//! which the dumps carry beside the public bytes.

use circuit::{BoolRepresentation, BoolWitness, Circuit, HintResult, PackedBits};
use super::e2e::{CircuitStatement, Error};
use super::statements::splitmix64;

/// The operand width.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MulWidth {
    U32,
    U64,
    U128,
}

impl MulWidth {
    pub fn bits(self) -> usize {
        match self {
            Self::U32 => 32,
            Self::U64 => 64,
            Self::U128 => 128,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::U32 => "mul-u32",
            Self::U64 => "mul-u64",
            Self::U128 => "mul-u128",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "mul-u32" => Some(Self::U32),
            "mul-u64" => Some(Self::U64),
            "mul-u128" => Some(Self::U128),
            _ => None,
        }
    }

    fn from_bits(bits: u64) -> Option<Self> {
        match bits {
            32 => Some(Self::U32),
            64 => Some(Self::U64),
            128 => Some(Self::U128),
            _ => None,
        }
    }
}

/// `gates` products of `width`-bit operands drawn from `seed`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MulStatement {
    pub width: MulWidth,
    pub gates: usize,
    pub seed: u64,
}

impl MulStatement {
    pub fn new(width: MulWidth, gates: usize, seed: u64) -> Self {
        Self { width, gates, seed }
    }

    /// The operands: `(x, y)` per gate, each `width` bits, little-endian
    /// words of the seed's stream (one per 64 bits, high words first).
    pub fn operands(&self) -> Vec<(u128, u128)> {
        let mut state = self.seed;
        let draw = |state: &mut u64| -> u128 {
            match self.width {
                MulWidth::U32 => u128::from(splitmix64(state) as u32),
                MulWidth::U64 => u128::from(splitmix64(state)),
                MulWidth::U128 => {
                    let high = splitmix64(state);
                    let low = splitmix64(state);
                    (u128::from(high) << 64) | u128::from(low)
                }
            }
        };
        (0..self.gates)
            .map(|_| {
                let x = draw(&mut state);
                let y = draw(&mut state);
                (x, y)
            })
            .collect()
    }

    /// The circuit's inputs: per gate `x` then `y`, bits little-endian.
    pub fn input(&self) -> Vec<bool> {
        let width = self.width.bits();
        let mut bits = Vec::with_capacity(self.input_bits());
        for (x, y) in self.operands() {
            for value in [x, y] {
                bits.extend((0..width).map(|bit| value >> bit & 1 != 0));
            }
        }
        bits
    }

    /// The inverse of [`CircuitStatement::public_bytes`] (the seed is not
    /// public; it comes from the dump).
    pub fn from_public_bytes(public: &[u8], seed: u64) -> Option<Self> {
        if public.len() != 16 {
            return None;
        }
        let width = MulWidth::from_bits(u64::from_le_bytes(public[..8].try_into().ok()?))?;
        let gates = u64::from_le_bytes(public[8..].try_into().ok()?) as usize;
        Some(Self { width, gates, seed })
    }
}

impl CircuitStatement for MulStatement {
    fn domain(&self) -> &'static [u8] {
        match self.width {
            MulWidth::U32 => b"mul-u32/v1",
            MulWidth::U64 => b"mul-u64/v1",
            MulWidth::U128 => b"mul-u128/v1",
        }
    }

    /// `u64` width in bits, `u64` gate count.
    fn public_bytes(&self) -> Vec<u8> {
        let mut bytes = (self.width.bits() as u64).to_le_bytes().to_vec();
        bytes.extend((self.gates as u64).to_le_bytes());
        bytes
    }

    fn input_bits(&self) -> usize {
        2 * self.width.bits() * self.gates
    }

    fn synthesize<CS: Circuit>(&self, cs: &mut CS, inputs: &[CS::Bool]) -> Result<(), Error> {
        if self.gates == 0 {
            return Err(Error::Input("at least one multiplication"));
        }
        if inputs.len() != self.input_bits() {
            return Err(Error::Input("wrong multiplication input length"));
        }
        let width = self.width.bits();
        for gate in inputs.chunks_exact(2 * width) {
            let (x, y) = gate.split_at(width);
            match self.width {
                MulWidth::U32 => mul_gate::<CS, 2, 32, 1, 64, 1>(cs, x, y),
                MulWidth::U64 => mul_gate::<CS, 3, 64, 1, 128, 2>(cs, x, y),
                MulWidth::U128 => mul_gate::<CS, 5, 128, 2, 256, 4>(cs, x, y),
            }
        }
        Ok(())
    }
}

/// One product: the operands' lifts, the product hinted as `P` bits and
/// lifted, one rank-1 row. `LIMBS` must hold the signed product
/// (`2W + 1` bits).
fn mul_gate<
    CS: Circuit,
    const LIMBS: usize,
    const W: usize,
    const WM: usize,
    const P: usize,
    const PM: usize,
>(
    cs: &mut CS,
    x: &[CS::Bool],
    y: &[CS::Bool],
) {
    let x_word = <CS::Bool as BoolWitness>::Repr::<W, WM>::from_array(std::array::from_fn(|i| x[i].clone()));
    let y_word = <CS::Bool as BoolWitness>::Repr::<W, WM>::from_array(std::array::from_fn(|i| y[i].clone()));
    // The fused lift: `Σ 2^i · f2z(bit_i)`, one `f2z` per bit in order on
    // the constraint backends, a packed word on the witness generator.
    let (x_lift, _) = cs.f2z_unsigned::<LIMBS, W, WM, W>(&x_word);
    let (y_lift, _) = cs.f2z_unsigned::<LIMBS, W, WM, W>(&y_word);
    let product = cs.hint::<LIMBS, P, PM, _>(move |context| {
        let x = x_word.evaluate(context);
        let y = y_word.evaluate(context);
        HintResult::Ok(PackedBits::<P, PM>::from_words(schoolbook::<PM>(x.words(), y.words())))
    });
    let (product_lift, _) = cs.f2z_unsigned::<LIMBS, P, PM, P>(&product);
    cs.assert_r1c::<LIMBS>(x_lift, y_lift, product_lift);
}

/// The product of two little-endian word vectors in `OUT` words (which
/// hold it: `OUT ≥ x.len() + y.len()`).
fn schoolbook<const OUT: usize>(x: &[u64], y: &[u64]) -> [u64; OUT] {
    let mut out = [0u64; OUT];
    for (i, &a) in x.iter().enumerate() {
        let mut carry = 0u128;
        for (j, &b) in y.iter().enumerate() {
            let total = u128::from(out[i + j]) + u128::from(a) * u128::from(b) + carry;
            out[i + j] = total as u64;
            carry = total >> 64;
        }
        let mut k = i + y.len();
        while carry != 0 {
            let total = u128::from(out[k]) + carry;
            out[k] = total as u64;
            carry = total >> 64;
            k += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::e2e::{Prepared, PreparedSampled};
    use super::super::fq::{Q, set_modulus};
    use super::*;

    #[test]
    fn products_are_schoolbook() {
        let x = 0xffff_ffff_ffff_ffff_ffff_ffff_ffff_fff1u128;
        let y = 0xdead_beef_0000_0000_1234_5678_9abc_def0u128;
        let words = schoolbook::<4>(&[x as u64, (x >> 64) as u64], &[y as u64, (y >> 64) as u64]);
        let expected = num_bigint::BigUint::from(x) * num_bigint::BigUint::from(y);
        let mut got = num_bigint::BigUint::default();
        for (i, word) in words.iter().enumerate() {
            got += num_bigint::BigUint::from(*word) << (64 * i);
        }
        assert_eq!(got, expected);
        assert_eq!(schoolbook::<1>(&[7], &[6]), [42]);
    }

    /// Every width proves and verifies end to end, fixed and sampled prime,
    /// and the statement round-trips its public bytes.
    #[test]
    fn every_width_proves_end_to_end() {
        let _guard = super::super::fq::test_modulus_guard();
        for width in [MulWidth::U32, MulWidth::U64, MulWidth::U128] {
            let statement = MulStatement::new(width, 3, 11);
            assert_eq!(
                MulStatement::from_public_bytes(&statement.public_bytes(), 11),
                Some(statement.clone())
            );
            let inputs = statement.input();
            let prepared = Prepared::new(statement.clone()).unwrap();
            assert_eq!(prepared.matrices().row_count(), 3);
            assert_eq!(prepared.matrices().column_count(), 1 + 3 * 4 * width.bits());
            let witness = prepared.witness(&inputs).unwrap();
            let (_, hint) = prepared.commit(&witness).unwrap();
            let proof = prepared.prove(&witness, &hint).unwrap();
            prepared.verify(&proof).unwrap();

            let sampled = PreparedSampled::new(statement, 100).unwrap();
            let witness = sampled.witness(&inputs).unwrap();
            let (_, hint) = sampled.commit(&witness).unwrap();
            let proof = sampled.prove(&witness, &hint).unwrap();
            sampled.verify(&proof).unwrap();
            set_modulus(Q).unwrap();
        }
    }

    /// The inputs are the operands' bits, `x` then `y` per gate.
    #[test]
    fn inputs_are_the_operands_bits() {
        let statement = MulStatement::new(MulWidth::U64, 2, 5);
        let inputs = statement.input();
        assert_eq!(inputs.len(), 256);
        let operands = statement.operands();
        let decode = |bits: &[bool]| bits.iter().rev().fold(0u128, |v, &b| (v << 1) | u128::from(b));
        assert_eq!(decode(&inputs[..64]), operands[0].0);
        assert_eq!(decode(&inputs[64..128]), operands[0].1);
        assert_eq!(decode(&inputs[128..192]), operands[1].0);
        assert!(operands[0].0 >> 64 == 0 && MulStatement::new(MulWidth::U32, 1, 5).operands()[0].0 >> 32 == 0);
    }
}
