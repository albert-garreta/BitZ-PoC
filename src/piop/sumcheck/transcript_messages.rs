//! Logical metadata for the existing single- and multi-degree sumchecks.
use crate::transcript::messages::{Absorbable, LegacyFieldValues, TranscriptField};
use crate::transcript::traits::Transcribable;
use crypto_primitives::{FromPrimitiveWithConfig, PrimeField};
use serde_json::{Value, json};

enum Degrees<'a> {
    Single(usize),
    Grouped(&'a [usize]),
}

pub struct SumcheckHeader<'a, F: PrimeField> {
    variables: usize,
    degrees: Degrees<'a>,
    config: &'a F::Config,
}

impl<'a, F: PrimeField> SumcheckHeader<'a, F> {
    pub fn single(variables: usize, degree: usize, config: &'a F::Config) -> Self {
        Self {
            variables,
            degrees: Degrees::Single(degree),
            config,
        }
    }
    pub fn grouped(variables: usize, degrees: &'a [usize], config: &'a F::Config) -> Self {
        Self {
            variables,
            degrees: Degrees::Grouped(degrees),
            config,
        }
    }
}

impl<F> Absorbable for SumcheckHeader<'_, F>
where
    F: TranscriptField + FromPrimitiveWithConfig,
    F::Inner: Transcribable,
    F::Modulus: Transcribable,
{
    fn kind(&self) -> &'static str {
        "sumcheck.header"
    }
    fn visit_chunks(&self, emit: &mut dyn FnMut(&[u8])) {
        let mut field = |purpose: &'static str, count: usize| {
            let _context = crate::transcript_context!(purpose);
            let value = F::from_with_cfg(count as u64, self.config);
            LegacyFieldValues(std::slice::from_ref(&value)).visit_chunks(emit);
        };
        field("sumcheck.variable_count", self.variables);
        match self.degrees {
            Degrees::Single(degree) => field("sumcheck.degree_bound", degree),
            Degrees::Grouped(degrees) => {
                field("sumcheck.group_count", degrees.len());
                for &degree in degrees {
                    field("sumcheck.degree_bound", degree);
                }
            }
        }
    }
    fn log_value(&self) -> Value {
        match self.degrees {
            Degrees::Single(degree) => json!({"variables": self.variables, "degree_bound": degree}),
            Degrees::Grouped(degrees) => json!({"variables": self.variables,
                "group_count": degrees.len(), "degree_bounds": degrees}),
        }
    }
}
