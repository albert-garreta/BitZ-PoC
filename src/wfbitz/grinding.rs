//! Native challenge blocks and the work actually executed before their draws.
//!
//! A block of `k` field coordinates has the conservative local algebraic bound
//! `3k / 2^128`. Work contributes to the host's economic model, not to an
//! unconditional statistical bound. Flock's proximity/query policy is separate.

pub(crate) const MAX_GRINDING_BITS: u32 = 24;
const MAX_VARIABLES: usize = 35;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub(crate) enum Error {
    #[error("native security target exceeds the 128-bit commitment field")]
    Target,
    #[error("native grinding exceeds the supported work bound")]
    Work,
    #[error("invalid native opening geometry")]
    Geometry,
    #[error("native challenge does not match the prepared schedule")]
    Sequence,
    #[error("native challenge schedule was not completely executed")]
    Incomplete,
}

/// A caller's declared target and applicable stage minima. `None` makes no
/// production target claim; it retains the declared minima without top-ups.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Policy {
    target: Option<u32>,
    reduction_minimum: u32,
    ring_minimum: u32,
}

impl Policy {
    pub(crate) const UNGRINDED: Self = Self {
        target: None,
        reduction_minimum: 0,
        ring_minimum: 0,
    };

    pub(crate) fn new(target: Option<u32>, reduction_minimum: u32, ring_minimum: u32) -> Result<Self, Error> {
        if target.is_some_and(|bits| bits > 128) {
            return Err(Error::Target);
        }
        if reduction_minimum.max(ring_minimum) > MAX_GRINDING_BITS {
            return Err(Error::Work);
        }
        Ok(Self { target, reduction_minimum, ring_minimum })
    }

    pub(crate) fn work(self, stage: Stage, coordinates: usize) -> Result<u32, Error> {
        if coordinates == 0 {
            return Ok(0);
        }
        let numerator = coordinates.checked_mul(3).ok_or(Error::Work)?;
        let ceil_log = usize::BITS - (numerator - 1).leading_zeros();
        let top_up = self.target.map_or(0, |target| (target + ceil_log).saturating_sub(128));
        let minimum = match stage {
            Stage::RingBatch | Stage::OodBatch => self.ring_minimum,
            _ => self.reduction_minimum,
        };
        let bits = minimum.max(top_up);
        if bits > MAX_GRINDING_BITS { Err(Error::Work) } else { Ok(bits) }
    }
}

/// Stable wire discriminants. Row and column binary rounds share a stage;
/// their first canonical index distinguishes the two phases.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum Stage {
    FoldPoint = 0,
    GkrRound = 1,
    GkrClose = 2,
    BinaryRound = 3,
    RingBatch = 4,
    OodBatch = 5,
}

impl Stage {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::FoldPoint => "wfbitz/fold-point",
            Self::GkrRound => "wfbitz/gkr-round",
            Self::GkrClose => "wfbitz/gkr-close",
            Self::BinaryRound => "wfbitz/binary-round",
            Self::RingBatch => "wfbitz/ring-batch",
            Self::OodBatch => "wfbitz/ood-batch",
        }
    }
}

/// Public route dimensions, never inferred from proof lengths or nonces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Geometry {
    pub(crate) integer: Option<(usize, usize)>,
    pub(crate) binary: Option<(usize, usize)>,
    pub(crate) ring: bool,
    pub(crate) ood: bool,
}

impl Geometry {
    pub(crate) fn opening(
        derived: &crate::pcs::IntegerMatrixLayout,
        committed: &crate::pcs::IntegerMatrixLayout,
        direct: bool,
    ) -> Self {
        Self {
            integer: Some((derived.row_vars, derived.col_vars)),
            binary: Some(if direct { (derived.row_vars, derived.col_vars) }
                else { (committed.row_vars + committed.col_vars, 0) }),
            ring: true,
            ood: false,
        }
    }

    pub(crate) fn prefix(rows: usize, columns: usize) -> Self {
        Self { integer: Some((rows, columns)), binary: None, ring: false, ood: false }
    }
}

/// An aggregated accounting entry from the same schedule the transcript uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Term {
    pub(crate) stage: Stage,
    pub(crate) coordinates: usize,
    pub(crate) occurrences: usize,
    pub(crate) grinding_bits: u32,
}

impl Term {
    pub(crate) fn raw_bits(self) -> f64 {
        128.0 - (3.0 * self.coordinates as f64).log2()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Schedule {
    policy: Policy,
    geometry: Geometry,
    work: [u32; 6],
}

impl Schedule {
    pub(crate) fn policy(self) -> Policy { self.policy }

    pub(crate) fn new(policy: Policy, geometry: Geometry) -> Result<Self, Error> {
        for (rows, columns) in [geometry.integer, geometry.binary].into_iter().flatten() {
            if rows.checked_add(columns).is_none_or(|sum| sum > MAX_VARIABLES) {
                return Err(Error::Geometry);
            }
        }
        if geometry.integer.is_some_and(|(rows, _)| rows == 0) || (geometry.ood && !geometry.ring) {
            return Err(Error::Geometry);
        }
        let columns = geometry.integer.map_or(0, |(_, columns)| columns);
        let work = [
            policy.work(Stage::FoldPoint, columns)?,
            policy.work(Stage::GkrRound, 1)?,
            policy.work(Stage::GkrClose, 1)?,
            policy.work(Stage::BinaryRound, 1)?,
            policy.work(Stage::RingBatch, 7)?,
            policy.work(Stage::OodBatch, 1)?,
        ];
        Ok(Self { policy, geometry, work })
    }

    pub(crate) fn terms(self) -> impl Iterator<Item = Term> {
        let (t, s) = self.geometry.integer.unwrap_or((0, 0));
        let (r, c) = self.geometry.binary.unwrap_or((0, 0));
        [
            (Stage::FoldPoint, s, usize::from(s > 0)),
            (Stage::GkrRound, 1, t * s + t * t.saturating_sub(1) / 2),
            (Stage::GkrClose, 1, t),
            (Stage::BinaryRound, 1, r + c),
            (Stage::RingBatch, 7, usize::from(self.geometry.ring)),
            (Stage::OodBatch, 1, usize::from(self.geometry.ood)),
        ].into_iter().filter(|(_, _, occurrences)| *occurrences != 0)
            .map(move |(stage, coordinates, occurrences)| Term {
                stage, coordinates, occurrences, grinding_bits: self.work[stage as usize],
            })
    }

    pub(crate) fn has_work(self) -> bool {
        self.terms().any(|term| term.grinding_bits != 0)
    }

    pub(crate) fn coordinate_count(self) -> usize {
        self.terms().map(|term| term.coordinates * term.occurrences).sum()
    }

    /// Fixed-width v1 policy frame. Every value fits after checked preparation.
    pub(crate) fn frame(self) -> [u8; 16] {
        let (t, s) = self.geometry.integer.unwrap_or((0, 0));
        let (r, c) = self.geometry.binary.unwrap_or((0, 0));
        [1, self.policy.target.map_or(255, |target| target as u8),
         self.policy.reduction_minimum as u8, self.policy.ring_minimum as u8,
         u8::from(self.geometry.integer.is_some()), t as u8, s as u8,
         u8::from(self.geometry.binary.is_some()), r as u8, c as u8,
         u8::from(self.geometry.ring), u8::from(self.geometry.ood), 0, 0, 0, 0]
    }

    pub(crate) fn cursor(self) -> Cursor {
        Cursor { schedule: self, phase: 0, layer: 0, round: 0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Block {
    pub(crate) stage: Stage,
    pub(crate) major: usize,
    pub(crate) minor: usize,
    pub(crate) coordinates: usize,
    pub(crate) bits: u32,
}

impl Block {
    pub(crate) fn frame(self) -> [u8; 8] {
        [self.stage as u8, self.major as u8, self.minor as u8,
         self.coordinates as u8, self.bits as u8, 0, 0, 0]
    }
}

/// No per-round allocation: the cursor follows fold, each GKR layer, binary
/// rows/columns, ring and optional OOD. Even zero-work execution is checked.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Cursor {
    schedule: Schedule,
    phase: u8,
    layer: usize,
    round: usize,
}

impl Cursor {
    fn next(&mut self) -> Option<Block> {
        let (t, s) = self.schedule.geometry.integer.unwrap_or((0, 0));
        let (r, c) = self.schedule.geometry.binary.unwrap_or((0, 0));
        loop {
            let position = match self.phase {
                0 if s > 0 => Some((Stage::FoldPoint, 0, 0, s)),
                1 if self.layer < t && self.round < s + self.layer => Some((Stage::GkrRound, self.layer, self.round, 1)),
                2 if self.layer < t => Some((Stage::GkrClose, self.layer, 0, 1)),
                3 if self.round < r => Some((Stage::BinaryRound, 0, self.round, 1)),
                4 if self.round < c => Some((Stage::BinaryRound, 1, self.round, 1)),
                5 if self.schedule.geometry.ring => Some((Stage::RingBatch, 0, 0, 7)),
                6 if self.schedule.geometry.ood => Some((Stage::OodBatch, 0, 0, 1)),
                7 => return None,
                _ => None,
            };
            if let Some((stage, major, minor, coordinates)) = position {
                return Some(Block { stage, major, minor, coordinates, bits: self.schedule.work[stage as usize] });
            }
            self.phase += 1;
            self.round = 0;
        }
    }

    pub(crate) fn take(&mut self, stage: Stage, coordinates: usize) -> Result<Block, Error> {
        let block = self.next().ok_or(Error::Sequence)?;
        if block.stage != stage || block.coordinates != coordinates {
            return Err(Error::Sequence);
        }
        match self.phase {
            0 => self.phase = 1,
            1 | 3 | 4 => self.round += 1,
            2 => { self.layer += 1; self.round = 0; self.phase = 1; }
            5 | 6 => self.phase += 1,
            _ => return Err(Error::Sequence),
        }
        Ok(block)
    }

    pub(crate) fn continuation(&self, next: Schedule) -> Result<(), Error> {
        if self.schedule.policy != next.policy
            || self.schedule.geometry.binary != next.geometry.binary
            || self.schedule.geometry.ring != next.geometry.ring
            || self.schedule.geometry.ood != next.geometry.ood
        { return Err(Error::Geometry); }
        Ok(())
    }

    pub(crate) fn finish(mut self) -> Result<(), Error> {
        if self.next().is_none() { Ok(()) } else { Err(Error::Incomplete) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_bounds_retain_minima_and_reject_overflow() {
        let p = Policy::new(Some(128), 0, 0).unwrap();
        assert_eq!(p.work(Stage::GkrRound, 0), Ok(0));
        assert_eq!(p.work(Stage::GkrRound, 1), Ok(2));
        assert_eq!(p.work(Stage::RingBatch, 7), Ok(5));
        assert_eq!(p.work(Stage::FoldPoint, 28), Ok(7));
        assert_eq!(p.work(Stage::FoldPoint, usize::MAX), Err(Error::Work));
        let minimum = Policy::new(Some(128), 6, 6).unwrap();
        assert_eq!(minimum.work(Stage::GkrRound, 1), Ok(6));
        assert_eq!(minimum.work(Stage::RingBatch, 7), Ok(6));
        assert!(Policy::new(Some(129), 0, 0).is_err());
        assert!(Policy::new(Some(128), 25, 0).is_err());
    }

    #[test]
    fn retained_low_targets_do_not_add_work() {
        for target in [100, 108, 112, 114] {
            let policy = Policy::new(Some(target), 0, 0).unwrap();
            let geometry = Geometry { integer: Some((7, 28)), binary: Some((35, 0)), ring: true, ood: true };
            assert!(!Schedule::new(policy, geometry).unwrap().has_work());
        }
        assert!(!Schedule::new(Policy::UNGRINDED, Geometry::prefix(35, 0)).unwrap().has_work());
    }

    #[test]
    fn execution_matches_accounting_for_all_shapes_and_query_routes() {
        for t in 1..=35 {
            for s in 0..=35 - t {
                for binary in [None, Some((t, s)), Some((t + s, 0))] {
                    let geometry = Geometry { integer: Some((t, s)), binary, ring: binary.is_some(), ood: binary.is_some() };
                    let schedule = Schedule::new(Policy::new(Some(128), 0, 0).unwrap(), geometry).unwrap();
                    let mut cursor = schedule.cursor();
                    let mut occurrences = [0usize; 6];
                    let mut coordinates = 0;
                    while let Some(block) = cursor.next() {
                        assert_eq!(cursor.take(block.stage, block.coordinates), Ok(block));
                        occurrences[block.stage as usize] += 1;
                        coordinates += block.coordinates;
                    }
                    assert_eq!(cursor.finish(), Ok(()));
                    assert_eq!(coordinates, schedule.coordinate_count());
                    for term in schedule.terms() {
                        assert_eq!(occurrences[term.stage as usize], term.occurrences);
                        assert!(term.raw_bits() + f64::from(term.grinding_bits) >= 128.0);
                    }
                    assert!(coordinates <= 673);
                }
            }
        }
    }

    #[test]
    fn empty_first_layer_and_reordered_or_incomplete_blocks() {
        let schedule = Schedule::new(Policy::UNGRINDED, Geometry::prefix(2, 0)).unwrap();
        let mut cursor = schedule.cursor();
        assert_eq!(cursor.take(Stage::GkrRound, 1), Err(Error::Sequence));
        assert_eq!(cursor.take(Stage::GkrClose, 1).unwrap().major, 0);
        assert_eq!(cursor.finish(), Err(Error::Incomplete));
        let round = cursor.take(Stage::GkrRound, 1).unwrap();
        assert_eq!((round.major, round.minor), (1, 0));
        assert_eq!(cursor.take(Stage::GkrClose, 1).unwrap().major, 1);
        assert_eq!(cursor.finish(), Ok(()));
        assert_eq!(cursor.take(Stage::RingBatch, 7), Err(Error::Sequence));
    }
}

const POW_PREFIX: &[u8] = b"bitz-native-pow1";

pub(crate) fn valid(seed: &[u8; 16], nonce: u64, bits: u32) -> bool {
    if bits == 0 { return nonce == 0; }
    let mut hash = blake3::Hasher::new();
    hash.update(POW_PREFIX);
    hash.update(seed);
    hash.update(&nonce.to_le_bytes());
    let digest = hash.finalize();
    let mut zeros = 0;
    for byte in digest.as_bytes() {
        let n = byte.leading_zeros();
        zeros += n;
        if n < 8 { break; }
    }
    zeros >= bits
}

pub(crate) fn grind(seed: &[u8; 16], bits: u32) -> u64 {
    let mut prefix = [0u8; 32];
    prefix[..16].copy_from_slice(POW_PREFIX);
    prefix[16..].copy_from_slice(seed);
    #[cfg(feature = "parallel")]
    let nonce = crate::utils::blake3x4::smallest_pow_nonce(&prefix, bits);
    #[cfg(not(feature = "parallel"))]
    let nonce = crate::utils::blake3x4::first_pow_nonce(&prefix, 0, u64::MAX, bits);
    nonce.expect("bounded native proof-of-work search exhausted")
}
