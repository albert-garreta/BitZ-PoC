//! Non-hiding proofs of the Falcon-512 ring equation and exact integer norm.
//! Public h and t use externally computed targets; SHAKE is not proved here.
const DEGREE: usize = 512;
macro_rules! algebraic_1024_tests {
    ($($item:item)*) => {};
}
include!("falcon1024_algebraic/backend.rs");
