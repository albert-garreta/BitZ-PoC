//! Environment configuration shared by the PCS comparisons.
use super::cli;
use clap::{Parser, ValueEnum};

pub fn enum_list<T: ValueEnum>(value: &str) -> Result<Vec<T>, String> {
    cli::list::<String>(value)?
        .into_iter()
        .map(|value| T::from_str(&value, false))
        .collect()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum Backend {
    Bitz,
    #[value(name = "plonky3-whir", alias = "whir")]
    Whir,
    #[value(name = "binius64-basefold", aliases = ["binius", "binius64"])]
    Binius,
    #[value(name = "bitz-ligerito-binary", aliases = ["ligerito", "bitz-ligerito"])]
    Ligerito,
}

impl Backend {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Bitz => "bitz",
            Self::Whir => "plonky3-whir",
            Self::Binius => "binius64-basefold",
            Self::Ligerito => "bitz-ligerito-binary",
        }
    }
    pub const fn display(self) -> &'static str {
        match self {
            Self::Bitz => "BitZ",
            Self::Whir => "Plonky3 WHIR",
            Self::Binius => "Binius64 BaseFold",
            Self::Ligerito => "BitZ Ligerito (binary claim)",
        }
    }
}

#[derive(Parser)]
struct Backends {
    #[arg(env = "BITZ_PCS_COMPARE_BACKENDS", default_value = "bitz plonky3-whir binius64-basefold bitz-ligerito-binary", value_parser = enum_list::<Backend>)]
    selected: cli::List<Backend>,
}

pub fn selected_backends() -> Vec<Backend> {
    let mut selected = Vec::new();
    for backend in cli::environment::<Backends>().selected {
        if !selected.contains(&backend) {
            selected.push(backend);
        }
    }
    selected
}

#[derive(Parser)]
pub struct Whir {
    #[arg(env = "BITZ_WHIR_FOLDING", value_parser = clap::builder::RangedU64ValueParser::<usize>::new().range(2..=12))]
    folding: Option<usize>,
    #[arg(env = "BITZ_WHIR_LOG_INV_RATE", default_value_t = 1, value_parser = clap::builder::RangedU64ValueParser::<usize>::new().range(1..=8))]
    log_inv_rate: usize,
    #[arg(env = "BITZ_WHIR_MAX_POW_BITS", default_value_t = 12, value_parser = clap::builder::RangedU64ValueParser::<usize>::new().range(0..106))]
    max_pow_bits: usize,
}

impl Whir {
    pub fn default_folding(exponent: usize) -> usize {
        (exponent.saturating_mul(3) / 4)
            .saturating_sub(5)
            .clamp(2, 12)
    }
    pub fn for_exponent(&self, exponent: usize) -> (usize, usize, usize) {
        let folding = self
            .folding
            .unwrap_or_else(|| Self::default_folding(exponent));
        (folding, self.log_inv_rate, self.max_pow_bits)
    }
}

#[derive(Parser)]
struct Binius {
    #[arg(env = "BITZ_BINIUS_LOG_INV_RATE", default_value_t = 1, value_parser = clap::builder::RangedU64ValueParser::<usize>::new().range(1..=4))]
    log_inv_rate: usize,
}

pub fn binius_log_inv_rate() -> usize {
    cli::environment::<Binius>().log_inv_rate
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn schemas_and_backend_aliases() {
        Backends::command().debug_assert();
        Whir::command().debug_assert();
        Binius::command().debug_assert();
        assert_eq!(
            enum_list::<Backend>("whir,binius64 ligerito").unwrap(),
            [Backend::Whir, Backend::Binius, Backend::Ligerito]
        );
        assert!(enum_list::<Backend>("").is_err());
        assert!(enum_list::<Backend>("unknown").is_err());
    }
}
