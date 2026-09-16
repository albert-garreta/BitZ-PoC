//! Borrowed transcript messages; proof containers remain protocol orchestrators.
use super::{FIELD_ELEMENTS_TAG, SPARTAN_TRANSCRIPT_FRAME_DOMAIN, SpartanField, extend_frame_len};
use crate::transcript::{
    logging::Hex,
    messages::{Absorbable, FramedBytes, numeric_hex},
};
use serde_json::{Value, json};

/// The statement immediately preceding Spartan's equality challenges.
pub struct SpartanStatement<'a> {
    pub protocol: &'a [u8],
    pub modulus: &'a [u8],
    pub matrix_digest: &'a [u8; 32],
    pub assignment_binding: &'a [u8; 32],
    pub skip_variables: Option<u8>,
}

impl Absorbable for SpartanStatement<'_> {
    fn kind(&self) -> &'static str {
        "spartan.statement"
    }

    fn visit_chunks(&self, emit: &mut dyn FnMut(&[u8])) {
        for (tag, payload) in [
            (&b"protocol"[..], self.protocol),
            (&b"field-modulus"[..], self.modulus),
            (&b"matrix-statement"[..], &self.matrix_digest[..]),
            (
                super::piop::SPARTAN_ASSIGNMENT_ORACLE_DOMAIN,
                &self.assignment_binding[..],
            ),
        ] {
            SpartanMessage { tag, payload }.visit_chunks(emit);
        }
        if let Some(skip) = self.skip_variables {
            SpartanMessage {
                tag: b"univariate-skip-vars",
                payload: &[skip],
            }
            .visit_chunks(emit);
        }
    }

    fn log_value(&self) -> Value {
        let mut value = json!({
            "protocol": String::from_utf8_lossy(self.protocol),
            "modulus_hex": numeric_hex(self.modulus),
            "matrix_digest_hex": Hex(self.matrix_digest).to_string(),
            "assignment_binding_hex": Hex(self.assignment_binding).to_string(),
        });
        if let Some(skip) = self.skip_variables {
            value["skip_variables"] = skip.into();
        }
        value
    }
}

/// Parameter absorption only; sampling and accepted-prime feedback follow it.
pub struct PrimeSamplingParameters<'a> {
    pub domain: &'a [u8],
    pub min: u128,
    pub max: u128,
}

impl Absorbable for PrimeSamplingParameters<'_> {
    fn kind(&self) -> &'static str {
        "projection_prime.parameters"
    }
    fn visit_chunks(&self, emit: &mut dyn FnMut(&[u8])) {
        SpartanMessage {
            tag: b"prime-domain",
            payload: self.domain,
        }
        .visit_chunks(emit);
        SpartanMessage {
            tag: b"prime-min",
            payload: &self.min.to_le_bytes(),
        }
        .visit_chunks(emit);
        SpartanMessage {
            tag: b"prime-max",
            payload: &self.max.to_le_bytes(),
        }
        .visit_chunks(emit);
    }
    fn log_value(&self) -> Value {
        json!({"domain": String::from_utf8_lossy(self.domain),
            "min_hex": format!("0x{:032x}", self.min), "max_hex": format!("0x{:032x}", self.max)})
    }
}

pub struct SpartanMessage<'a> {
    pub tag: &'a [u8],
    pub payload: &'a [u8],
}
impl Absorbable for SpartanMessage<'_> {
    fn kind(&self) -> &'static str {
        "spartan.message"
    }
    fn visit_chunks(&self, emit: &mut dyn FnMut(&[u8])) {
        let _context = crate::transcript_context!(
            "spartan.message",
            message_tag = tracing::field::display(String::from_utf8_lossy(self.tag))
        );
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

pub struct OuterTerminalEvaluations<'a, F>(pub &'a [F]);
impl<F: SpartanField> Absorbable for OuterTerminalEvaluations<'_, F> {
    fn kind(&self) -> &'static str {
        "spartan.outer_terminal_evaluations"
    }
    fn visit_chunks(&self, emit: &mut dyn FnMut(&[u8])) {
        SpartanFieldElements(self.0).visit_chunks(emit);
    }
    fn log_value(&self) -> Value {
        let [az, bz, cz] = self.0 else {
            unreachable!("three terminal evaluations")
        };
        json!({"field": "prime", "modulus_hex": numeric_hex(&F::canonical_modulus_encoding(az.cfg())),
            "az_hex": numeric_hex(&az.canonical_element_encoding()),
            "bz_hex": numeric_hex(&bz.canonical_element_encoding()),
            "cz_hex": numeric_hex(&cz.canonical_element_encoding())})
    }
}

/// Finite evaluations followed by the leading coefficient, in the original order.
pub struct SkipPolynomialMessage<'a, F> {
    pub skip_variables: u8,
    pub points: &'a [i32],
    pub values: &'a [F],
}
impl<F: SpartanField> Absorbable for SkipPolynomialMessage<'_, F> {
    fn kind(&self) -> &'static str {
        "spartan.univariate_skip_polynomial"
    }
    fn visit_chunks(&self, emit: &mut dyn FnMut(&[u8])) {
        SpartanFieldElements(self.values).visit_chunks(emit);
    }
    fn log_value(&self) -> Value {
        let (leading, finite) = self.values.split_last().expect("skip leading coefficient");
        debug_assert_eq!(finite.len(), self.points.len());
        json!({"field": "prime", "modulus_hex": numeric_hex(&F::canonical_modulus_encoding(leading.cfg())),
            "skip_variables": self.skip_variables,
            "evaluations": self.points.iter().zip(finite).map(|(point, value)| json!({
                "point": point, "value_hex": numeric_hex(&value.canonical_element_encoding())
            })).collect::<Vec<_>>(),
            "leading_coefficient": {"degree": 2 * ((1usize << self.skip_variables) - 1),
                "value_hex": numeric_hex(&leading.canonical_element_encoding())}})
    }
}
