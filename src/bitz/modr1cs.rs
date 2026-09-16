//! An integer Mod-R1CS instance (Limber's MultiSwap in the paper) in their
//! gadget language, from an instance file both sides read: `A·z ∘ B·z =
//! C·z + m ∘ q` over `ℤ`, every value committed as bits — `WIDE` bits per
//! value, one bit for the values a `v·v = v` row constrains — and, per
//! modular row, the quotient `q` as a `WIDE`-bit hint and the product
//! `t = m·q` as a `2·WIDE`-bit hint with its own rank-1 row `m · q = t`
//! (so every coefficient on a committed bit is a power of two; the moduli
//! and the instance's other constants sit on the constant column). The
//! instance's rows, moduli, coefficients and widths are public through
//! their digest; the values are the witness.
//!
//! The file format is this module's own (`to_bytes`/`from_bytes`); the
//! exporter `bitz_modr1cs_export` writes the paper's 6,209-row MultiSwap
//! instance from the crate's Limber port.

use std::sync::Arc;

use circuit::{Circuit, HintResult, PackedBits};
use num_bigint::BigUint;
use num_traits::{One, Zero};
use sha2::{Digest, Sha256};

use super::e2e::{CircuitStatement, Error};

/// The width of every value that is not a bit, and of every quotient.
pub const WIDE: usize = 2048;
const WIDE_WORDS: usize = WIDE / 64;
const PRODUCT: usize = 2 * WIDE;
const PRODUCT_WORDS: usize = PRODUCT / 64;
/// Signed intermediates: products of two `WIDE`-bit values, with room.
const LIMBS: usize = PRODUCT / 64 + 1;

/// One coefficient: on a value column, or the constant (`column == None`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub column: Option<u32>,
    pub coefficient: BigUint,
}

/// One row `A·z ∘ B·z = C·z (+ m·q)`; `modulus` zero means exact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub a: Vec<Entry>,
    pub b: Vec<Entry>,
    pub c: Vec<Entry>,
    pub modulus: BigUint,
}

/// The instance: the rows, the columns' widths (`1` or [`WIDE`]), the
/// witness values and the quotients.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instance {
    pub rows: Vec<Row>,
    pub widths: Vec<u16>,
    pub values: Vec<BigUint>,
    pub quotients: Vec<BigUint>,
}

fn put_u64(out: &mut Vec<u8>, value: usize) {
    out.extend_from_slice(&(value as u64).to_le_bytes());
}

fn put_big(out: &mut Vec<u8>, value: &BigUint) {
    let bytes = value.to_bytes_le();
    put_u64(out, bytes.len());
    out.extend_from_slice(&bytes);
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn u64(&mut self) -> Option<usize> {
        let word = self.bytes.get(self.at..self.at + 8)?;
        self.at += 8;
        Some(u64::from_le_bytes(word.try_into().ok()?) as usize)
    }

    fn big(&mut self) -> Option<BigUint> {
        let len = self.u64()?;
        let bytes = self.bytes.get(self.at..self.at + len)?;
        self.at += len;
        Some(BigUint::from_bytes_le(bytes))
    }

    fn entries(&mut self, columns: usize) -> Option<Vec<Entry>> {
        let count = self.u64()?;
        (0..count)
            .map(|_| {
                let column = self.u64()?;
                let coefficient = self.big()?;
                Some(Entry {
                    column: (column < columns).then_some(column as u32),
                    coefficient,
                })
            })
            .collect()
    }
}

impl Instance {
    /// The public part first (the digest covers it), then the values.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = self.public_bytes();
        put_u64(&mut out, self.values.len());
        for value in &self.values {
            put_big(&mut out, value);
        }
        put_u64(&mut out, self.quotients.len());
        for quotient in &self.quotients {
            put_big(&mut out, quotient);
        }
        out
    }

    /// The rows, moduli, coefficients and widths.
    fn public_bytes(&self) -> Vec<u8> {
        let mut out = b"bitz/mod-r1cs/instance/v1".to_vec();
        let columns = self.widths.len();
        put_u64(&mut out, self.rows.len());
        put_u64(&mut out, columns);
        for width in &self.widths {
            out.extend_from_slice(&width.to_le_bytes());
        }
        for row in &self.rows {
            put_big(&mut out, &row.modulus);
            for entries in [&row.a, &row.b, &row.c] {
                put_u64(&mut out, entries.len());
                for entry in entries {
                    put_u64(&mut out, entry.column.map_or(columns, |c| c as usize));
                    put_big(&mut out, &entry.coefficient);
                }
            }
        }
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let magic = b"bitz/mod-r1cs/instance/v1";
        if !bytes.starts_with(magic) {
            return None;
        }
        let mut reader = Reader {
            bytes,
            at: magic.len(),
        };
        let rows = reader.u64()?;
        let columns = reader.u64()?;
        let widths = (0..columns)
            .map(|_| {
                let word = bytes.get(reader.at..reader.at + 2)?;
                reader.at += 2;
                Some(u16::from_le_bytes(word.try_into().ok()?))
            })
            .collect::<Option<Vec<_>>>()?;
        let rows = (0..rows)
            .map(|_| {
                let modulus = reader.big()?;
                let a = reader.entries(columns)?;
                let b = reader.entries(columns)?;
                let c = reader.entries(columns)?;
                Some(Row { a, b, c, modulus })
            })
            .collect::<Option<Vec<_>>>()?;
        let values = (0..reader.u64()?).map(|_| reader.big()).collect::<Option<Vec<_>>>()?;
        let quotients = (0..reader.u64()?).map(|_| reader.big()).collect::<Option<Vec<_>>>()?;
        if values.len() != columns || quotients.len() != rows.len() || reader.at != bytes.len() {
            return None;
        }
        Some(Self {
            rows,
            widths,
            values,
            quotients,
        })
    }

    /// SHA-256 of the public part.
    pub fn digest(&self) -> [u8; 32] {
        Sha256::digest(self.public_bytes()).into()
    }

    /// Whether every row holds over the integers with these values.
    pub fn satisfied(&self) -> bool {
        let evaluate = |entries: &[Entry]| -> BigUint {
            entries
                .iter()
                .map(|entry| match entry.column {
                    Some(column) => &entry.coefficient * &self.values[column as usize],
                    None => entry.coefficient.clone(),
                })
                .sum()
        };
        self.rows.iter().zip(&self.quotients).all(|(row, quotient)| {
            evaluate(&row.a) * evaluate(&row.b) == evaluate(&row.c) + &row.modulus * quotient
        })
    }

    // MIRROR-DROP-START
    /// The paper's MultiSwap instance (`k = 0`, 6,209 live rows) from the
    /// crate's Limber port: the live rows and columns only, values a
    /// `v·v = v` row constrains one bit wide, every other value and every
    /// quotient [`WIDE`].
    pub fn multiswap() -> Result<Self, Error> {
        use crate::piop::spartan::multiswap::MultiswapDims;
        Self::from_limber(MultiswapDims::multiswap(0))
    }

    /// The port's reduced-size shape (every row species, a `2^11`-gate
    /// instance; not a Limber configuration): what the oracle's generator
    /// can dump, for the parity pins.
    pub fn multiswap_mini() -> Result<Self, Error> {
        use crate::piop::spartan::multiswap::MultiswapDims;
        Self::from_limber(MultiswapDims::mini())
    }

    fn from_limber(dims: crate::piop::spartan::multiswap::MultiswapDims) -> Result<Self, Error> {
        use crate::piop::spartan::multiswap::MultiswapCircuit;
        let circuit = MultiswapCircuit::build(dims)
            .map_err(|_| Error::Configuration("the MultiSwap circuit did not build"))?;
        let const_col = circuit.const_col();
        let mut rows: Vec<Row> = (0..circuit.num_cons())
            .map(|row| Row {
                a: Vec::new(),
                b: Vec::new(),
                c: Vec::new(),
                modulus: circuit.mods()[row].clone(),
            })
            .collect();
        let convert = |column: usize, coefficient: &BigUint| Entry {
            column: (column != const_col).then_some(column as u32),
            coefficient: coefficient.clone(),
        };
        for (row, column, coefficient) in circuit.a_entries() {
            rows[*row].a.push(convert(*column, coefficient));
        }
        for (row, column, coefficient) in circuit.b_entries() {
            rows[*row].b.push(convert(*column, coefficient));
        }
        for (row, column, coefficient) in circuit.c_entries() {
            rows[*row].c.push(convert(*column, coefficient));
        }
        let live: Vec<usize> = (0..circuit.num_cons())
            .filter(|&row| !(rows[row].a.is_empty() && rows[row].b.is_empty() && rows[row].c.is_empty()))
            .collect();
        // A column is live if any live row touches it; it is a bit if an
        // exact row reads `v·v = v` on it alone.
        let mut touched = vec![false; circuit.num_vars()];
        let mut bit = vec![false; circuit.num_vars()];
        for &row in &live {
            for entry in rows[row].a.iter().chain(&rows[row].b).chain(&rows[row].c) {
                if let Some(column) = entry.column {
                    touched[column as usize] = true;
                }
            }
            let single = |entries: &[Entry]| -> Option<u32> {
                match entries {
                    [Entry {
                        column: Some(column),
                        coefficient,
                    }] if coefficient.is_one() => Some(*column),
                    _ => None,
                }
            };
            if rows[row].modulus.is_zero() {
                if let (Some(a), Some(b), Some(c)) = (single(&rows[row].a), single(&rows[row].b), single(&rows[row].c)) {
                    if a == b && b == c {
                        bit[a as usize] = true;
                    }
                }
            }
        }
        let mut renumber = vec![u32::MAX; circuit.num_vars()];
        let mut widths = Vec::new();
        let mut values = Vec::new();
        for column in 0..circuit.num_vars() {
            if touched[column] {
                renumber[column] = widths.len() as u32;
                widths.push(if bit[column] { 1 } else { WIDE as u16 });
                values.push(circuit.witness()[column].clone());
            }
        }
        let mut kept = Vec::with_capacity(live.len());
        let mut quotients = Vec::with_capacity(live.len());
        for &row in &live {
            let mut kept_row = rows[row].clone();
            for entry in kept_row.a.iter_mut().chain(kept_row.b.iter_mut()).chain(kept_row.c.iter_mut()) {
                if let Some(column) = entry.column {
                    entry.column = Some(renumber[column as usize]);
                }
            }
            kept.push(kept_row);
            quotients.push(circuit.quotients()[row].clone());
        }
        let instance = Self {
            rows: kept,
            widths,
            values,
            quotients,
        };
        if !instance.satisfied() {
            return Err(Error::Configuration("the MultiSwap instance is not satisfied"));
        }
        if instance.values.iter().zip(&instance.widths).any(|(value, width)| value.bits() > u64::from(*width))
            || instance.quotients.iter().any(|quotient| quotient.bits() > WIDE as u64)
        {
            return Err(Error::Configuration("a MultiSwap value is wider than its width"));
        }
        Ok(instance)
    }
    // MIRROR-DROP-END
}

/// The statement: a Mod-R1CS instance, identified by its digest.
#[derive(Clone, Debug)]
pub struct ModR1csStatement {
    instance: Arc<Instance>,
    digest: [u8; 32],
}

impl PartialEq for ModR1csStatement {
    fn eq(&self, other: &Self) -> bool {
        self.digest == other.digest
    }
}

impl Eq for ModR1csStatement {}

impl ModR1csStatement {
    pub fn new(instance: Instance) -> Self {
        let digest = instance.digest();
        Self {
            instance: Arc::new(instance),
            digest,
        }
    }

    pub fn instance(&self) -> &Instance {
        &self.instance
    }

    pub fn digest(&self) -> &[u8; 32] {
        &self.digest
    }

    /// No inputs: every value is a hint.
    pub fn input(&self) -> Vec<bool> {
        Vec::new()
    }

    /// The public bytes must be this instance's.
    pub fn matches_public_bytes(&self, public: &[u8]) -> bool {
        public == self.public_bytes()
    }
}

impl CircuitStatement for ModR1csStatement {
    fn domain(&self) -> &'static [u8] {
        b"mod-r1cs/v1"
    }

    /// The instance digest, `u64` rows, `u64` columns.
    fn public_bytes(&self) -> Vec<u8> {
        let mut bytes = self.digest.to_vec();
        put_u64(&mut bytes, self.instance.rows.len());
        put_u64(&mut bytes, self.instance.widths.len());
        bytes
    }

    fn input_bits(&self) -> usize {
        0
    }

    fn synthesize<CS: Circuit>(&self, cs: &mut CS, inputs: &[CS::Bool]) -> Result<(), Error> {
        if !inputs.is_empty() {
            return Err(Error::Input("a Mod-R1CS statement takes no inputs"));
        }
        let instance = &self.instance;
        // The values, hinted at their widths and lifted once.
        let lifted: Vec<CS::Z<LIMBS>> = instance
            .widths
            .iter()
            .zip(&instance.values)
            .map(|(&width, value)| {
                if width == 1 {
                    let bit = value.is_one();
                    let bits = cs.hint::<LIMBS, 1, 1, _>(move |_| HintResult::Ok(PackedBits::<1, 1>::from_u64(u64::from(bit))));
                    cs.f2z_unsigned::<LIMBS, 1, 1, 1>(&bits).0
                } else {
                    let words = words::<WIDE_WORDS>(value);
                    let bits = cs.hint::<LIMBS, WIDE, WIDE_WORDS, _>(move |_| HintResult::Ok(PackedBits::from_words(words)));
                    cs.f2z_unsigned::<LIMBS, WIDE, WIDE_WORDS, WIDE>(&bits).0
                }
            })
            .collect();
        for (row, quotient) in instance.rows.iter().zip(&instance.quotients) {
            let combine = |cs: &mut CS, entries: &[Entry]| -> CS::Z<LIMBS> {
                let mut sum = CS::Z::<LIMBS>::zero();
                for entry in entries {
                    let coefficient = coefficient::<CS>(&entry.coefficient);
                    sum += match entry.column {
                        Some(column) => lifted[column as usize].clone() * coefficient,
                        None => CS::Z::<LIMBS>::from(coefficient),
                    };
                }
                sum
            };
            let a = combine(cs, &row.a);
            let b = combine(cs, &row.b);
            let mut c = combine(cs, &row.c);
            if !row.modulus.is_zero() {
                // `t = m·q` on its own row, then `C + t`.
                let quotient_words = words::<WIDE_WORDS>(quotient);
                let quotient_bits = cs.hint::<LIMBS, WIDE, WIDE_WORDS, _>(move |_| {
                    HintResult::Ok(PackedBits::from_words(quotient_words))
                });
                let product_words = words::<PRODUCT_WORDS>(&(&row.modulus * quotient));
                let product_bits = cs.hint::<LIMBS, PRODUCT, PRODUCT_WORDS, _>(move |_| {
                    HintResult::Ok(PackedBits::from_words(product_words))
                });
                let (q, _) = cs.f2z_unsigned::<LIMBS, WIDE, WIDE_WORDS, WIDE>(&quotient_bits);
                let (t, _) = cs.f2z_unsigned::<LIMBS, PRODUCT, PRODUCT_WORDS, PRODUCT>(&product_bits);
                let m = CS::Z::<LIMBS>::from(coefficient::<CS>(&row.modulus));
                cs.assert_r1c::<LIMBS>(m, q, t.clone());
                c += t;
            }
            cs.assert_r1c::<LIMBS>(a, b, c);
        }
        Ok(())
    }
}

/// A coefficient from an integer, by Horner over its 64-bit words (ring
/// operations only, so every backend on either side builds the same).
fn coefficient<CS: Circuit>(value: &BigUint) -> CS::Coefficient<LIMBS> {
    let mut radix = CS::Coefficient::<LIMBS>::one();
    for _ in 0..64 {
        radix += radix.clone();
    }
    value
        .to_u64_digits()
        .iter()
        .rev()
        .fold(CS::Coefficient::<LIMBS>::zero(), |acc, word| {
            acc * radix.clone() + CS::Coefficient::<LIMBS>::from(*word)
        })
}

/// The little-endian words of a value, `N` of them.
fn words<const N: usize>(value: &BigUint) -> [u64; N] {
    let digits = value.to_u64_digits();
    assert!(digits.len() <= N, "a value wider than its width");
    let mut out = [0u64; N];
    out[..digits.len()].copy_from_slice(&digits);
    out
}

#[cfg(test)]
mod tests {
    use super::super::e2e::{Prepared, PreparedSampled};
    use super::super::fq::{Q, set_modulus};
    use super::*;

    /// A three-row instance: `x·y ≡ z (mod 1000)`, a bit row, an exact
    /// row with a constant.
    fn tiny() -> Instance {
        let value = |v: u64| BigUint::from(v);
        let on = |column: u32, c: u64| Entry {
            column: Some(column),
            coefficient: value(c),
        };
        let constant = |c: u64| Entry {
            column: None,
            coefficient: value(c),
        };
        // x = 1234, y = 5678, z = x·y mod 1000 = 652, q = 7006; b = 1;
        // 3·x + 5 = 3707.
        Instance {
            rows: vec![
                Row {
                    a: vec![on(0, 1)],
                    b: vec![on(1, 1)],
                    c: vec![on(2, 1)],
                    modulus: value(1000),
                },
                Row {
                    a: vec![on(3, 1)],
                    b: vec![on(3, 1)],
                    c: vec![on(3, 1)],
                    modulus: value(0),
                },
                Row {
                    a: vec![on(0, 3), constant(5)],
                    b: vec![constant(1)],
                    c: vec![on(4, 1)],
                    modulus: value(0),
                },
            ],
            widths: vec![WIDE as u16, WIDE as u16, WIDE as u16, 1, WIDE as u16],
            values: vec![value(1234), value(5678), value(652), value(1), value(3707)],
            quotients: vec![value(7006), value(0), value(0)],
        }
    }

    #[test]
    fn the_file_format_round_trips_and_digests_the_public_part() {
        let instance = tiny();
        assert!(instance.satisfied());
        let bytes = instance.to_bytes();
        let back = Instance::from_bytes(&bytes).unwrap();
        assert_eq!(back, instance);
        let mut other = instance.clone();
        other.values[0] = BigUint::from(1u32);
        assert_eq!(other.digest(), instance.digest());
        other.rows[0].modulus = BigUint::from(999u32);
        assert_ne!(other.digest(), instance.digest());
    }

    #[test]
    fn the_tiny_instance_proves_end_to_end() {
        let _guard = super::super::fq::test_modulus_guard();
        let statement = ModR1csStatement::new(tiny());
        let prepared = Prepared::new(statement.clone()).unwrap();
        // Three rows plus one auxiliary product row.
        assert_eq!(prepared.matrices().row_count(), 4);
        let witness = prepared.witness(&[]).unwrap();
        let (_, hint) = prepared.commit(&witness).unwrap();
        let proof = prepared.prove(&witness, &hint).unwrap();
        prepared.verify(&proof).unwrap();

        let sampled = PreparedSampled::new(statement, 100).unwrap();
        let witness = sampled.witness(&[]).unwrap();
        let (_, hint) = sampled.commit(&witness).unwrap();
        let proof = sampled.prove(&witness, &hint).unwrap();
        sampled.verify(&proof).unwrap();
        set_modulus(Q).unwrap();
    }

    #[test]
    fn a_wrong_value_is_unsatisfied() {
        let _guard = super::super::fq::test_modulus_guard();
        let mut instance = tiny();
        instance.values[2] = BigUint::from(653u32);
        assert!(!instance.satisfied());
        let prepared = Prepared::new(ModR1csStatement::new(instance)).unwrap();
        assert!(prepared.witness(&[]).is_err());
    }
}
