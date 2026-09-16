//! Borrowed transcript messages; proof containers remain protocol orchestrators.
use super::{FIELD_ELEMENTS_TAG, SPARTAN_TRANSCRIPT_FRAME_DOMAIN, SpartanField, extend_frame_len};
use crate::transcript::{
    logging::Hex,
    messages::{Absorbable, FramedBytes, numeric_hex},
};
use serde_json::{Value, json};

pub struct SpartanMessage<'a> {
    pub tag: &'a [u8],
    pub payload: &'a [u8],
}
impl Absorbable for SpartanMessage<'_> {
    fn kind(&self) -> &'static str {
        "spartan.message"
    }
    fn visit_chunks(&self, emit: &mut dyn FnMut(&[u8])) {
        // Preserve the original single payload update as well as its encoding.
        let mut frame = Vec::with_capacity(
            SPARTAN_TRANSCRIPT_FRAME_DOMAIN.len() + self.tag.len() + self.payload.len() + 16,
        );
        frame.extend_from_slice(SPARTAN_TRANSCRIPT_FRAME_DOMAIN);
        extend_frame_len(&mut frame, self.tag.len());
        frame.extend_from_slice(self.tag);
        extend_frame_len(&mut frame, self.payload.len());
        frame.extend_from_slice(self.payload);
        FramedBytes(&frame).visit_chunks(emit);
    }
    fn log_value(&self) -> Value {
        json!({"tag": String::from_utf8_lossy(self.tag), "bytes_hex": Hex(self.payload).to_string()})
    }
}

pub struct SpartanFieldElements<'a, F>(pub &'a [F]);
impl<F: SpartanField> Absorbable for SpartanFieldElements<'_, F> {
    fn kind(&self) -> &'static str {
        "spartan.field_elements"
    }
    fn visit_chunks(&self, emit: &mut dyn FnMut(&[u8])) {
        let mut payload = Vec::new();
        extend_frame_len(&mut payload, self.0.len());
        for value in self.0 {
            let encoding = value.canonical_element_encoding();
            extend_frame_len(&mut payload, encoding.len());
            payload.extend_from_slice(&encoding);
        }
        SpartanMessage {
            tag: FIELD_ELEMENTS_TAG,
            payload: &payload,
        }
        .visit_chunks(emit);
    }
    fn log_value(&self) -> Value {
        json!({"field": "prime", "modulus_hex": self.0.first().map(|f| numeric_hex(&F::canonical_modulus_encoding(f.cfg()))),
            "values_hex": self.0.iter().map(|f| numeric_hex(&f.canonical_element_encoding())).collect::<Vec<_>>()})
    }
}

pub struct SpartanRoundPolynomial<'a, F>(pub &'a [F]);
impl<F: SpartanField> Absorbable for SpartanRoundPolynomial<'_, F> {
    fn kind(&self) -> &'static str {
        "sumcheck.round_polynomial"
    }
    fn visit_chunks(&self, emit: &mut dyn FnMut(&[u8])) {
        SpartanFieldElements(self.0).visit_chunks(emit);
    }
    fn log_value(&self) -> Value {
        let mut value = SpartanFieldElements(self.0).log_value();
        let coefficients = value.as_object_mut().unwrap().remove("values_hex").unwrap();
        value["coefficients_hex"] = coefficients;
        value["degree_bound"] = self.0.len().saturating_sub(1).into();
        value["basis"] = "monomial".into();
        value["coefficient_order"] = "constant_first".into();
        value
    }
}
