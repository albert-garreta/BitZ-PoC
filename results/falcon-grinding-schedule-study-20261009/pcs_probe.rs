#[path = "../../src/ligerito_flock/grinding_plan.rs"]
mod grinding_plan;
use bitz::ligerito_flock::LigeritoSelection;
use serde_json::json;
fn main() {
    for packed_log in [18,19,23,24] {
        for rate in [1,2] {
            for initial_k in [3,4,5,6] {
                let selection = LigeritoSelection::CustomUdr {log_inv_rate:rate,initial_k,fold_grinding:true};
                let resolved=selection.resolve(packed_log,128).unwrap();
                let plan=grinding_plan::GrindingPlan::resolve(resolved.security(),128).unwrap();
                let blocks:Vec<_>=plan.blocks.iter().map(|b|json!({"label":b.label,"raw_normalized":b.raw_error*2f64.powi(128),"min_bits":b.native_bits.unwrap_or(0)})).collect();
                println!("{}",json!({"packed_log":packed_log,"rate":rate,"initial_k":initial_k,"blocks":blocks,"security":resolved.security()}));
            }
        }
    }
}
