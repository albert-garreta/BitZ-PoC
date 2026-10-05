//! Affine authentication of native-ring projections and compaction leaves.
use super::*;

pub(super) struct LeafWeights {
    pub local: Vec<F>,
    pub instances: Vec<F>,
    pub beta: Vec<F>,
    pub forest: Vec<F>,
}

impl LeafWeights {
    pub(super) fn new(
        layout: &FalconSourceLayout,
        proof: &FalconPiopProof,
        field: &Cfg,
    ) -> Result<Self, FalconError> {
        let leaf = &proof.compaction_leaf;
        let d = layout.capacity().ilog2() as usize;
        if leaf.point.len() != 11 + d || leaf.instance_point.len() != d {
            return Err(piop("compaction leaf binding dimensions"));
        }
        let point = &proof
            .compaction
            .first()
            .ok_or_else(|| piop("missing compaction forest"))?
            .candidate
            .terminal_point;
        if point.len() != 11
            || proof.compaction.iter().any(|pair| {
                pair.candidate.terminal_point != *point || pair.output.terminal_point != *point
            })
        {
            return Err(piop("compaction forest endpoint mismatch"));
        }
        Ok(Self {
            local: eq_table(&leaf.point[..11], field).map_err(|e| piop(e.to_string()))?,
            instances: eq_table(&leaf.point[11..], field).map_err(|e| piop(e.to_string()))?,
            beta: eq_table(&leaf.instance_point, field).map_err(|e| piop(e.to_string()))?,
            forest: eq_table(point, field).map_err(|e| piop(e.to_string()))?,
        })
    }
}

#[cfg(test)]
pub(super) fn add_leaf_claims(
    sink: &mut impl CoefficientSink,
    target: &mut F,
    scale: &mut F,
    eta: F,
    layout: &FalconSourceLayout,
    proof: &FalconPiopProof,
    field: &Cfg,
) -> Result<(), FalconError> {
    let weights = LeafWeights::new(layout, proof, field)?;
    add_leaf_claims_prepared(sink, target, scale, eta, layout, proof, field, &weights)
}

pub(super) fn add_leaf_claims_prepared(
    sink: &mut impl CoefficientSink,
    target: &mut F,
    scale: &mut F,
    eta: F,
    layout: &FalconSourceLayout,
    proof: &FalconPiopProof,
    field: &Cfg,
    weights: &LeafWeights,
) -> Result<(), FalconError> {
    let leaf = &proof.compaction_leaf;
    let offsets = layout.offsets();
    for coordinate in 0..3 {
        let mut constant = field.zero();
        for s in sink.instances(layout.batch()) {
            let base = s * layout.signature_stride();
            let instance_scale = field.mul(scale, &weights.instances[s]);
            let third_scale = field.mul(&instance_scale, &weights.beta[s]);
            for i in 0..HASH_TO_POINT_SAMPLES {
                let weight = field.mul(&instance_scale, &weights.local[i]);
                if coordinate < 2 {
                    let index = if coordinate == 0 {
                        offsets.hash_accept_ands + i
                    } else {
                        offsets.hash_prefixes + 11 * i + 10
                    };
                    sink.add(base + index, field.neg(&weight));
                    if sink.needs_constants() {
                        constant = field.add(&constant, &weight);
                    }
                } else {
                    let weight = field.mul(
                        &third_scale,
                        &field.mul(&weights.local[i], &weights.forest[i]),
                    );
                    sink.add_word(
                        base + offsets.hash_prefixes + 11 * i,
                        11,
                        field.mul(&weight, &proof.compaction_rank_scale),
                        field,
                    );
                    add_value_scaled(sink, base + offsets.hash_remainders + 14 * i, weight, field);
                    if sink.needs_constants() {
                        constant = field.add(
                            &constant,
                            &field.mul(&weight, &field.sub(&proof.compaction_gamma, &field.one())),
                        );
                    }
                }
            }
        }
        add_claim_target(target, *scale, leaf.terminal[coordinate], constant, field);
        *scale = field.mul(scale, &eta);
    }
    Ok(())
}

impl BindingForm<'_> {
    pub(super) fn add_native_claim(
        &self,
        sink: &mut impl CoefficientSink,
        target: &mut F,
        scale: F,
    ) -> Result<(), FalconError> {
        let Some(claim) = &self.native_claim else {
            return Ok(());
        };
        let field = self.field;
        if claim.weights.len() != self.layout.batch() * N {
            return Err(piop("native ring source dimensions"));
        }
        let offsets = self.layout.offsets();
        let mut constant = field.zero();
        for s in sink.instances(self.layout.batch()) {
            let base = s * self.layout.signature_stride();
            for j in 0..N {
                let weight = field.mul(&scale, &claim.weights[s * N + j]);
                sink.add_word(base + offsets.hash_point + 14 * j, 14, weight, field);
                add_value_scaled(sink, base + offsets.s1 + 14 * j, field.neg(&weight), field);
                if sink.needs_constants() {
                    constant = field.add(&constant, &mul_i(weight, 6144, field));
                }
            }
        }
        add_claim_target(target, scale, claim.target, constant, field);
        Ok(())
    }
}
