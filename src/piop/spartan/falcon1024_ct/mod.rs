//! Falcon-1024 compatibility API. Typed profiles live in `falcon_profiles`.
const DEGREE: usize = 1024;
const EXTENSION: usize = 11;
macro_rules! falcon_tests {
    ($($item:item)*) => { $(#[cfg(test)] $item)* };
}
include!("backend.rs");
