//! Composite messages for the compact mod-q opening and its early OOD round.
use super::{
    Commitment, FIELD_BYTES, FIELD_GF128, FIELD_U8, FIELD_U64, Gf, HashKind, IntegerMatrixLayout,
    LigeritoStatementConfig, ModQOpeningKind, OOD_ROUND_DOMAIN, STATEMENT_FRAME_DOMAIN,
    StatementField, ligerito_profile_code, merkle_hash_code, statement_field_context,
};
use crate::transcript::{
    logging::Hex,
    messages::{Absorbable, FramedBytes, TranscriptField},
};
use serde_json::{Value, json};

pub(super) struct OodParameters {
    pub packed_variables: usize,
    pub grinding_bits: u32,
}
impl Absorbable for OodParameters {
    fn kind(&self) -> &'static str {
        "opening.ood_parameters"
    }
    fn visit_chunks(&self, emit: &mut dyn FnMut(&[u8])) {
        FramedBytes(OOD_ROUND_DOMAIN).visit_chunks(emit);
        FramedBytes(&(self.packed_variables as u64).to_le_bytes()).visit_chunks(emit);
        FramedBytes(&self.grinding_bits.to_le_bytes()).visit_chunks(emit);
    }
    fn log_value(&self) -> Value {
        json!({"domain": String::from_utf8_lossy(OOD_ROUND_DOMAIN),
            "packed_variables": self.packed_variables, "grinding_bits": self.grinding_bits})
    }
}

pub(super) struct OpeningStatement<'a, C> {
    pub opening_kind: ModQOpeningKind,
    pub commitment: &'a Commitment,
    pub layout: &'a IntegerMatrixLayout,
    pub statement_digest: &'a [u8; 32],
    pub q_bits: usize,
    pub generator: Gf,
    pub config: &'a C,
}

/// Read-only field descriptions reuse the same encoder as standalone fields.
enum FieldValue<'a> {
    Bytes(&'a [u8]),
    Byte(u8),
    Size(usize),
    Sizes(&'a [usize]),
    Gf(Gf),
}
impl FieldValue<'_> {
    fn visit(&self, tag: u8, emit: &mut dyn FnMut(&[u8])) {
        let (kind, count) = match self {
            Self::Bytes(v) => (FIELD_BYTES, v.len()),
            Self::Byte(_) => (FIELD_U8, 1),
            Self::Size(_) => (FIELD_U64, 1),
            Self::Sizes(v) => (FIELD_U64, v.len()),
            Self::Gf(_) => (FIELD_GF128, 1),
        };
        let _context = statement_field_context(tag, kind, count);
        StatementField {
            tag,
            kind,
            count,
            encode: |emit: &mut dyn FnMut(&[u8])| match self {
                Self::Bytes(v) => emit(v),
                Self::Byte(v) => emit(&[*v]),
                Self::Size(v) => emit(&(*v as u64).to_le_bytes()),
                Self::Sizes(v) => {
                    for &value in *v {
                        emit(&(value as u64).to_le_bytes());
                    }
                }
                Self::Gf(v) => {
                    for word in v.words() {
                        emit(&word.to_le_bytes());
                    }
                }
            },
            // Only visit_chunks is called: the parent supplies one diagnostic object.
            value: || Value::Null,
        }
        .visit_chunks(emit);
    }
}

impl<C: LigeritoStatementConfig> Absorbable for OpeningStatement<'_, C> {
    fn kind(&self) -> &'static str {
        "opening.statement"
    }
    fn visit_chunks(&self, emit: &mut dyn FnMut(&[u8])) {
        for domain in [STATEMENT_FRAME_DOMAIN, self.opening_kind.statement_domain()] {
            let _context = crate::transcript_context!(
                "statement.domain_separator",
                domain = tracing::field::display(String::from_utf8_lossy(domain))
            );
            FramedBytes(domain).visit_chunks(emit);
        }
        use FieldValue::*;
        let p = &self.commitment.params;
        let c = self.config;
        for (tag, value) in [
            (0x01, Bytes(&self.commitment.root)),
            (0x02, Size(p.m)),
            (0x03, Size(p.log_inv_rate)),
            (0x04, Size(p.log_batch_size)),
            (0x05, Byte(ligerito_profile_code(p.profile))),
            (0x06, Byte(merkle_hash_code(p.merkle_hash))),
            (0x08, Sizes(c.log_inv_rates())),
            (0x09, Size(c.recursive_steps())),
            (0x0a, Size(c.initial_log_msg_cols())),
            (0x0b, Size(c.initial_log_num_interleaved())),
            (0x0c, Size(c.initial_k())),
            (0x0d, Sizes(c.recursive_log_msg_cols())),
            (0x0e, Sizes(c.recursive_ks())),
            (0x0f, Sizes(c.queries())),
            (0x10, Sizes(c.grinding_bits())),
            (0x11, Sizes(c.fold_grinding_bits())),
            (0x12, Sizes(c.ood_samples())),
            (0x13, Byte(merkle_hash_code(c.merkle_hash()))),
            (0x20, Size(self.layout.row_vars)),
            (0x21, Size(self.layout.col_vars)),
            (0x22, Size(self.layout.word_bits)),
        ] {
            value.visit(tag, emit);
        }
        crate::transcript_context!("statement.opening_claim_digest" => Bytes(self.statement_digest).visit(0x30, emit));
        crate::transcript_context!("statement.modulus_bit_length" => Size(self.q_bits).visit(0x31, emit));
        crate::transcript_context!("statement.generator" => Gf(self.generator).visit(0x32, emit));
    }
    fn log_value(&self) -> Value {
        let p = &self.commitment.params;
        let c = self.config;
        json!({
            "domains": {"frame": String::from_utf8_lossy(STATEMENT_FRAME_DOMAIN),
                "protocol": String::from_utf8_lossy(self.opening_kind.statement_domain())},
            "commitment": {"root_hex": Hex(&self.commitment.root).to_string(), "variables": p.m,
                "log_inverse_rate": p.log_inv_rate, "log_batch_size": p.log_batch_size,
                "profile_code": ligerito_profile_code(p.profile), "merkle_hash": hash_name(p.merkle_hash)},
            "ligerito_configuration": {"log_inverse_rates": c.log_inv_rates(), "recursive_steps": c.recursive_steps(),
                "initial": {"log_message_columns": c.initial_log_msg_cols(),
                    "log_interleaving": c.initial_log_num_interleaved(), "fold_count": c.initial_k()},
                "recursive_log_message_columns": c.recursive_log_msg_cols(), "recursive_fold_counts": c.recursive_ks(),
                "query_counts": c.queries(), "query_grinding_bits": c.grinding_bits(),
                "fold_grinding_bits": c.fold_grinding_bits(), "ood_sample_counts": c.ood_samples(),
                "merkle_hash": hash_name(c.merkle_hash())},
            "layout": {"row_variables": self.layout.row_vars, "column_variables": self.layout.col_vars,
                "word_bits": self.layout.word_bits},
            "opening_claim_digest_hex": Hex(self.statement_digest).to_string(),
            "modulus_bit_length": self.q_bits, "generator": self.generator.log_value(),
        })
    }
}

fn hash_name(hash: HashKind) -> &'static str {
    match hash {
        HashKind::Blake3 => "blake3",
        HashKind::Sha256 => "sha256",
    }
}

#[cfg(test)]
mod tests {
    use super::super::{LigeritoSelection, PcsParams, StatementFrame, ligerito};
    use super::*;
    use crate::transcript::{
        Blake3Transcript,
        logging::{self, Event, TranscriptLogs},
        traits::{ConstTranscribable, Transcript},
    };
    use crate::utils::primality::PrimalityTest;
    use crypto_primitives::ConstIntSemiring;
    use tracing_subscriber::prelude::*;

    #[derive(Default)]
    struct Chunks(Vec<Vec<u8>>);
    impl Transcript for Chunks {
        fn absorb_inner(&mut self, bytes: &[u8]) {
            self.0.push(bytes.to_vec());
        }
        fn get_challenge<T: ConstTranscribable>(&mut self) -> T {
            panic!("encoder cannot squeeze")
        }
        fn get_prime<R: ConstIntSemiring + ConstTranscribable, T: PrimalityTest<R>>(
            &mut self,
        ) -> R {
            panic!("encoder cannot sample primes")
        }
    }

    #[test]
    fn opening_and_ood_objects_preserve_every_legacy_chunk() {
        let resolved = LigeritoSelection::JOHNSON.resolve(15, 100).unwrap();
        let config = resolved.prover();
        let commitment = Commitment {
            root: [7; 32],
            params: PcsParams {
                m: 22,
                log_inv_rate: 1,
                log_batch_size: 4,
                profile: ligerito::LigeritoProfile::Fast,
                merkle_hash: HashKind::Blake3,
            },
        };
        let layout = IntegerMatrixLayout {
            row_vars: 15,
            col_vars: 7,
            word_bits: 1,
        };
        for opening_kind in [
            ModQOpeningKind::U32Mul,
            ModQOpeningKind::U64Mul,
            ModQOpeningKind::U128Mul,
            ModQOpeningKind::BabyBearMul,
        ] {
            let object = OpeningStatement {
                opening_kind,
                commitment: &commitment,
                layout: &layout,
                statement_digest: &[9; 32],
                q_bits: 111,
                generator: Gf::from_words([2, 0]),
                config,
            };
            let mut legacy = Chunks::default();
            let mut frame = StatementFrame::new(&mut legacy, opening_kind.statement_domain());
            frame.commitment(&commitment);
            frame.ligerito_config(config);
            frame.int_eval_params(&layout);
            frame.bytes(0x30, object.statement_digest);
            frame.usize(0x31, object.q_bits);
            frame.gf128(0x32, object.generator);
            let mut actual = Chunks::default();
            actual.absorb(&object);
            assert_eq!(actual.0, legacy.0);
            assert_eq!(actual.0.len(), 139);
            assert_eq!(
                object.log_value()["commitment"]["root_hex"],
                "07".repeat(32)
            );
            assert_eq!(
                object.log_value()["ligerito_configuration"]["query_counts"],
                json!(config.queries())
            );
        }
        for grinding_bits in [0, 12] {
            let object = OodParameters {
                packed_variables: 15,
                grinding_bits,
            };
            let mut legacy = Chunks::default();
            legacy.absorb_slice(OOD_ROUND_DOMAIN);
            legacy.absorb_slice(&15u64.to_le_bytes());
            legacy.absorb_slice(&grinding_bits.to_le_bytes());
            let mut actual = Chunks::default();
            actual.absorb(&object);
            assert_eq!(actual.0, legacy.0);
        }

        let dir = tempfile::tempdir().unwrap();
        let logs = TranscriptLogs::new(true);
        let _subscriber =
            tracing::subscriber::set_default(tracing_subscriber::registry().with(logs.layer()));
        let trial = logs.begin_trial(dir.path().join("opening.jsonl")).unwrap();
        let mut transcript = trial.wrap("verifier", Blake3Transcript::new());
        transcript.absorb(&OpeningStatement {
            opening_kind: ModQOpeningKind::U32Mul,
            commitment: &commitment,
            layout: &layout,
            statement_digest: &[9; 32],
            q_bits: 111,
            generator: Gf::from_words([2, 0]),
            config,
        });
        transcript.absorb(&OodParameters {
            packed_variables: 15,
            grinding_bits: 0,
        });
        drop(transcript);
        let path = trial.finish().unwrap().unwrap();
        let events: Vec<_> = logging::read_events(path)
            .unwrap()
            .map(|e| match e.unwrap() {
                Event::Logical(e) => e,
                _ => panic!("expected logical record"),
            })
            .collect();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].purpose, "opening.statement");
        assert_eq!(events[0].wire.len(), 139);
        assert_eq!(events[1].purpose, "opening.ood_parameters");
        assert_eq!(events[1].wire.len(), 9);
        assert!(events.iter().all(|e| e.complete && e.role == "verifier"));
    }
}
