// Authenticate the three rejection-product endpoints in the same source bits.
use super::super::hash_to_point_selection::REJECTION_ROW_LOG;
use super::*;

pub(super) struct RejectionWeights {
    local: Vec<F>,
    instances: Vec<F>,
    scales: [F; 3],
    target: F,
}

impl RejectionWeights {
    pub(super) fn new(
        layout: &FalconSourceLayout,
        proof: FalconPiopClaimRef<'_>,
        eta: F,
        field: &Cfg,
    ) -> Result<Self, FalconError> {
        let claim = &proof.h2p_rejection;
        if claim.point.len() != REJECTION_ROW_LOG + layout.capacity().ilog2() as usize {
            return Err(piop(
                "HashToPoint rejection binding point dimension mismatch",
            ));
        }
        let local = eq_table(&claim.point[..REJECTION_ROW_LOG], field)
            .map_err(|error| piop(error.to_string()))?;
        let instances = eq_table(&claim.point[REJECTION_ROW_LOG..], field)
            .map_err(|error| piop(error.to_string()))?;
        let mut scale = eta;
        for _ in 0..2 {
            scale = field.mul(&scale, &eta);
        }
        let scales = [
            scale,
            field.mul(&scale, &eta),
            field.mul(&field.mul(&scale, &eta), &eta),
        ];
        let target = claim
            .terminal
            .iter()
            .zip(scales)
            .fold(field.zero(), |sum, (value, scale)| {
                field.add(&sum, &field.mul(value, &scale))
            });
        Ok(Self {
            local,
            instances,
            scales,
            target,
        })
    }

    pub(super) fn target(&self) -> F {
        self.target
    }
    pub(super) fn instance_weights(&self) -> &[F] {
        &self.instances
    }

    pub(super) fn emit_local(
        &self,
        sink: &mut impl CoefficientSink,
        layout: &FalconSourceLayout,
        field: &Cfg,
    ) {
        self.emit_scaled(sink, layout, 0, field.one(), field);
    }

    #[cfg(test)]
    pub(super) fn emit(
        &self,
        sink: &mut impl CoefficientSink,
        layout: &FalconSourceLayout,
        field: &Cfg,
    ) {
        for instance in sink.instances(layout.batch()) {
            self.emit_scaled(
                sink,
                layout,
                instance * layout.signature_stride(),
                self.instances[instance],
                field,
            );
        }
    }

    fn emit_scaled(
        &self,
        sink: &mut impl CoefficientSink,
        layout: &FalconSourceLayout,
        base: usize,
        instance: F,
        field: &Cfg,
    ) {
        let offsets = layout.offsets();
        for (j, &local) in self.local.iter().take(HASH_TO_POINT_SAMPLES).enumerate() {
            let weight = field.mul(&instance, &local);
            for (address, scale) in [
                (offsets.hash_quotients + 3 * j + 2, self.scales[0]),
                (offsets.hash_quotients + 3 * j, self.scales[1]),
                (offsets.hash_accept_ands + j, self.scales[2]),
            ] {
                sink.add(base + address, field.mul(&weight, &scale));
            }
        }
    }
}
