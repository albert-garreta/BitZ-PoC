//! Ordinary cubic Spartan with UDR opening, without a univariate skip or OOD.
use super::{
    U32MulLayout, U32MulWitness, IopInstanceFacts, IopSecurityParams, Lambda100,
    f2z::u32_assignment_binding,
    protocol::{self, BindingHasher, BlockTable, Domains, Kernel, MatrixSource,
        PiopWitness, PreparedRelation, ProtocolError, ProveOptions, RelationSpec, FieldConfig},
};
use crate::ligerito_flock::LigeritoSelection;
use flock_core::pcs::commit::Commitment;
use flock_core::pcs::ligerito::ProverConfig;

#[derive(Clone, Copy, Debug)]
pub struct PlainU32Relation(pub U32MulLayout);

impl PlainU32Relation {
    pub fn prepare(layout: U32MulLayout) -> Result<PreparedRelation<Self>, ProtocolError> {
        PreparedRelation::new_with_profile_and_ligerito::<Lambda100>(Self(layout), LigeritoSelection::MATCHED_UDR)
    }
}

impl RelationSpec for PlainU32Relation {
    type Coefficient = bool;
    type Witness = U32MulWitness;
    type Map = <U32MulLayout as RelationSpec>::Map;

    fn domains(&self) -> &'static Domains { self.0.domains() }
    fn committed_layout(&self) -> crate::pcs::IntegerMatrixLayout { self.0.committed_layout() }
    fn gate_vars(&self) -> usize { self.0.gate_vars() }
    fn instance_facts(&self) -> IopInstanceFacts {
        let mut facts = self.0.instance_facts();
        facts.piop_degree = 3;
        facts
    }
    fn matrices(&self) -> Result<MatrixSource<bool>, ProtocolError> { self.0.matrices() }
    fn validate_geometry(&self) -> Result<(), ProtocolError> { self.0.validate_geometry() }
    fn block_table(&self) -> BlockTable { self.0.block_table() }
    fn kernel(&self) -> Kernel { Kernel::Plain }
    fn schedule(&self) -> protocol::Schedule {
        protocol::Schedule { ood_round: false, ..self.0.schedule() }
    }
    fn opener_grinding_bits(&self, security: &IopSecurityParams) -> u32 {
        self.0.opener_grinding_bits(security)
    }
    fn check_witness(&self, witness: &U32MulWitness) -> Result<(), ProtocolError> {
        self.0.check_witness(witness)
    }
    fn assignment_binding(&self, commitment: &Commitment, security: &IopSecurityParams, config: &ProverConfig) -> Result<[u8;32], ProtocolError> {
        if security.ood.is_some() || config.ood_samples.iter().any(|&n| n != 0) {
            return Err(ProtocolError::LigeritoConfig("plain U32 requires UDR without OOD".into()));
        }
        u32_assignment_binding(&self.0, commitment, security, config,
            b"f2z/spartan-u32-mul/assignment/plain/v1", 0, 3)
    }
    fn hash_bridge_constants(&self, hasher: &mut BindingHasher) -> Result<(), ProtocolError> {
        self.0.hash_bridge_constants(hasher)
    }
    fn piop_witness<'w>(&self, witness: &'w U32MulWitness, config: &FieldConfig, options: ProveOptions) -> Result<PiopWitness<'w>, ProtocolError> {
        self.0.piop_witness(witness, config, options)
    }
}
