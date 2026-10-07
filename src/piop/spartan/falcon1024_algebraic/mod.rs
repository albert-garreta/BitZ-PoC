//! Non-hiding proofs of the Falcon-1024 ring equation and exact integer norm.
//!
//! Public inputs are canonical polynomials `h` and `t`. The caller is responsible
//! for connecting `t` to a message and nonce. Both signature components are
//! witnesses, authenticated by one BitZ commitment; this module generates no
//! SHAKE or HashToPoint trace.

const DEGREE: usize = 1024;
macro_rules! algebraic_1024_tests { ($($item:item)*) => { $($item)* }; }
include!("backend.rs");
