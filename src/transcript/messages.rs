//! Encodings are protocol-specific. These adapters preserve the existing bytes;
//! display values are diagnostic and are never absorbed.
use super::{
    logging::Hex,
    traits::{GenTranscribable, Transcribable},
};
use crypto_primitives::{Field, PrimeField};
use serde_json::{Value, json};

/// An encoder can emit bytes, but cannot access the transcript or draw challenges.
pub trait Absorbable {
    fn kind(&self) -> &'static str;
    fn visit_chunks(&self, emit: &mut dyn FnMut(&[u8]));
    fn log_value(&self) -> Value;
}

pub fn numeric_hex(little_endian: &[u8]) -> String {
    format!(
        "0x{}",
        Hex(&little_endian.iter().rev().copied().collect::<Vec<_>>())
    )
}
pub fn transcribed_value<T: Transcribable>(value: &T) -> Value {
    let mut bytes = vec![0; value.get_num_bytes()];
    value.write_transcription_bytes_exact(&mut bytes);
    json!({"type": std::any::type_name::<T>(), "encoding_hex": Hex(&bytes).to_string(), "value_hex": numeric_hex(&bytes)})
}

/// Explicit canonical display conversion; never interpret a Montgomery residue
/// as the mathematical field value. Implementations do not affect wire encoding.
pub trait TranscriptField: PrimeField {
    fn log_value(&self) -> Value;
}
impl<const N: usize> TranscriptField for crypto_primitives::crypto_bigint_monty::MontyField<N> {
    fn log_value(&self) -> Value {
        json!({"field": "prime", "value_hex": transcribed_value(&self.retrieve())["value_hex"],
            "modulus_hex": transcribed_value(&self.modulus())["value_hex"]})
    }
}
impl<Mod: crypto_bigint::modular::ConstMontyParams<N>, const N: usize> TranscriptField
    for crypto_primitives::crypto_bigint_const_monty::ConstMontyField<Mod, N>
{
    fn log_value(&self) -> Value {
        json!({"field": "prime", "value_hex": transcribed_value(&self.retrieve())["value_hex"],
            "modulus_hex": transcribed_value(&self.modulus())["value_hex"]})
    }
}
macro_rules! binary_field {
    ($ty:ty, $name:literal) => {
        impl TranscriptField for $ty {
            fn log_value(&self) -> Value {
                json!({"field": $name, "basis": "polynomial", "value_hex": transcribed_value(self.inner())["value_hex"]})
            }
        }
    }
}
binary_field!(
    crate::poly::univariate::binary_gf128::BinaryFieldGF128,
    "GF(2^128)"
);
binary_field!(
    crate::poly::univariate::binary_b127::BinaryFieldB127,
    "GF(2^127)"
);

pub struct FramedBytes<'a>(pub &'a [u8]);
impl Absorbable for FramedBytes<'_> {
    fn kind(&self) -> &'static str {
        "bytes"
    }
    fn visit_chunks(&self, emit: &mut dyn FnMut(&[u8])) {
        emit(&[0x06]);
        emit(self.0);
        emit(&[0x07]);
    }
    fn log_value(&self) -> Value {
        json!({"bytes_hex": Hex(self.0).to_string()})
    }
}

pub struct LegacyFieldValues<'a, F>(pub &'a [F]);
impl<F: TranscriptField> Absorbable for LegacyFieldValues<'_, F>
where
    F::Inner: Transcribable,
    F::Modulus: Transcribable,
{
    fn kind(&self) -> &'static str {
        "field_values"
    }
    fn visit_chunks(&self, emit: &mut dyn FnMut(&[u8])) {
        let mut buf = Vec::new();
        for v in self.0 {
            buf.resize(v.inner().get_num_bytes(), 0);
            debug_assert_eq!(buf.len(), v.modulus().get_num_bytes());
            emit(&[0x03]);
            v.modulus().write_transcription_bytes_exact(&mut buf);
            emit(&buf);
            emit(&[0x05]);
            emit(&[0x01]);
            v.inner().write_transcription_bytes_exact(&mut buf);
            emit(&buf);
            emit(&[0x03]);
        }
    }
    fn log_value(&self) -> Value {
        json!({"values": self.0.iter().map(TranscriptField::log_value).collect::<Vec<_>>()})
    }
}

/// Attach decoded values to an existing byte frame without changing its bytes.
/// The closure is evaluated only when diagnostics are enabled.
pub struct DescribedFrame<'a, V> {
    pub bytes: &'a [u8],
    pub kind: &'static str,
    pub value: V,
}
impl<V: Fn() -> Value> Absorbable for DescribedFrame<'_, V> {
    fn kind(&self) -> &'static str {
        self.kind
    }
    fn visit_chunks(&self, emit: &mut dyn FnMut(&[u8])) {
        FramedBytes(self.bytes).visit_chunks(emit);
    }
    fn log_value(&self) -> Value {
        (self.value)()
    }
}
