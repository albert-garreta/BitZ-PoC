from pathlib import Path
import subprocess
root=Path('/tmp/bitz-falcon-poc-push-20261012')
out=Path(__file__).resolve().parent
p=root/'benches/common/falcon_stage_timings.rs'
original=subprocess.check_output(['git','show','HEAD:benches/common/falcon_stage_timings.rs'],cwd=root).decode()
assert p.read_text()==original
names=['falcon_bench:commit','falcon_bench:prove','falcon_bench:verify',
'falcon_algebraic:commit','falcon_algebraic:prove','falcon_algebraic:verify',
'falcon_algebraic:witness','falcon_algebraic:ring','falcon_algebraic:arithmetic',
'falcon_algebraic:binding','falcon_algebraic:integer_outer','falcon_algebraic:binding_construction',
'falcon_algebraic:word_first_fold','falcon_algebraic:bitz','falcon_algebraic:pcs',
'falcon_algebraic_ring:coordinates','falcon_algebraic_ring:carries','falcon_algebraic_ring:collapse',
'falcon_hybrid:witness_commit','falcon_hybrid:shake_witness','falcon_hybrid:arithmetic_witness',
'falcon_hybrid:source_construction','falcon_hybrid:source_packing','shared_source:encode','shared_source:merkle',
'falcon_hybrid:arithmetic_prefix','falcon_hybrid:binary_bridge','falcon_hybrid:keccak_prefix',
'falcon_hybrid:link_coefficients','falcon_hybrid:joint_sumcheck','falcon_hybrid:shared_opening',
'falcon_arithmetic:binding_inner','falcon_arithmetic:integer_outer','falcon_shared_ring:prove',
'falcon_bridge:integer_folds','falcon_bridge:column_packing','falcon_bridge:power_tables','falcon_bridge:wfbitz_forest',
'op:initial_messages','op:deferred_first_fold','op:ring_switch','op:extra_bases','op:ligerito']
needle='''    fn on_new_span(
'''
method='''    // Measurement-only filter: avoid enabling per-round grinding and fold spans.
    fn enabled(&self, metadata: &tracing::Metadata<'_>, _ctx: tracing_subscriber::layer::Context<'_, S>) -> bool {
        matches!(metadata.name(),
'''+ '\n'.join('            '+('' if i==0 else '| ')+'"'+name+'"' for i,name in enumerate(names))+'''
        )
    }

'''
assert original.count(needle)==1
p.write_text(original.replace(needle,method+needle))
(out/'light-profile-subscriber.patch').write_bytes(subprocess.check_output(['git','diff','--','benches/common/falcon_stage_timings.rs'],cwd=root))
