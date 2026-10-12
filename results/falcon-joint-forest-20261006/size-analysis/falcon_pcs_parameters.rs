use bitz::ligerito_flock::LigeritoSelection;
#[path = "../../../src/ligerito_flock/grinding_plan.rs"]
mod grinding_plan;
fn main() {
    for target in [100usize,128] {
        for rate in 1..=4 {
            let resolved=LigeritoSelection::CustomUdr {log_inv_rate:rate,initial_k:4,fold_grinding:true}.resolve(24,target).unwrap();
            let security=resolved.security();
            let first=grinding_plan::GrindingPlan::resolve(security,target as u32).unwrap();
            let pcs_target=target+2+first.blocks.len().next_power_of_two().ilog2() as usize;
            let plan=grinding_plan::GrindingPlan::resolve(security,pcs_target as u32).unwrap();
            let err:f64=plan.blocks.iter().map(|b| b.raw_error*2f64.powi(-(b.bits as i32))).sum();
            println!("target={target} rate=1/{} blocks={} pcs_target={} pcs_error_bits={:.9}",1<<rate,plan.blocks.len(),pcs_target,-err.log2());
            for (i,p) in security.levels.iter().enumerate() {println!("level={i} depth={} lane_log={} folds={} queries={} qbits={:.6} pgbits={:.6}",p.log_msg_cols+p.log_inv_rate,p.log_num_interleaved,p.k_recursive,p.queries,p.paper_predicted_bits().1,p.paper_predicted_bits().0);}
            for b in &plan.blocks {println!("block={} bits={}",b.label,b.bits);}
        }
    }
}
