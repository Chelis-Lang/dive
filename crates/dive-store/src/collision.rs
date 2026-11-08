//! TLC's minimum fingerprint gap (ADR 0015 §5), the input of TLC's "based on the
//! actual fingerprints" collision estimate.
//!
//! Ported from tlaplus `94d0c50` (`tlatools/org.lamport.tlatools/src/`): `d` is the
//! smallest difference between two fingerprints in TLC's set. TLC's default set is a
//! `MultiFPSet` of `2^fpBits` sub-sets (default `fpBits` = 1) split by the
//! fingerprints' top bits, and each sub-set's `DiskFPSet.checkFPs` scans its own
//! sorted fingerprints; the minimum is over sub-sets (`MultiFPSet.checkFPs`). So `d`
//! ignores pairs that straddle a sub-set boundary.

/// TLC's `Long.MAX_VALUE`, the minimum gap of a set with no two fingerprints in one
/// sub-set.
pub(crate) const NO_GAP: u64 = i64::MAX.unsigned_abs();

/// Whether `a` and `b` lie in the same one of TLC's `2^fp_bits` sub-sets.
fn same_subset(a: u64, b: u64, fp_bits: u32) -> bool {
    let shift = 64 - fp_bits.clamp(1, 63);
    a >> shift == b >> shift
}

/// Folds the gap between consecutive sorted fingerprints `a < b` into `gap`.
pub(crate) fn closer(gap: u64, a: u64, b: u64, fp_bits: u32) -> u64 {
    if b > a && same_subset(a, b, fp_bits) {
        gap.min(b - a)
    } else {
        gap
    }
}

/// The minimum gap TLC's `checkFPs` reports for `fps` (sorted, consecutive pairs);
/// the reference the disk store's quadratic scan is checked against.
#[cfg(test)]
pub(crate) fn tlc_min_gap(fps: impl IntoIterator<Item = u64>, fp_bits: u32) -> u64 {
    let mut v: Vec<u64> = fps.into_iter().collect();
    v.sort_unstable();
    v.dedup();
    v.windows(2)
        .fold(NO_GAP, |gap, w| closer(gap, w[0], w[1], fp_bits))
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    reason = "test code: a failed expectation is a test failure"
)]
mod tests {
    use super::{NO_GAP, tlc_min_gap};

    /// TLC's fingerprints of every distinct state of a model
    /// (`tests/fixtures/tlc-fingerprints`).
    fn read_fps(model: &str) -> Vec<u64> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/tlc-fingerprints")
            .join(format!("{model}.fp64"));
        let bytes = std::fs::read(path).expect("fingerprint fixture");
        let (words, rest) = bytes.as_chunks::<8>();
        assert_eq!(rest, &[] as &[u8]);
        words.iter().map(|w| u64::from_le_bytes(*w)).collect()
    }

    /// TLC's printed "actual" figure: `1 / gap` to two significant digits.
    #[allow(clippy::cast_precision_loss, reason = "a printed two-digit estimate")]
    fn figure(gap: u64) -> String {
        format!("{:.1E}", 1.0 / gap as f64)
    }

    #[test]
    fn sub_sets_split_by_top_bits() {
        assert_eq!(tlc_min_gap([10, 3, 7], 1), 3);
        // 0x7FFF… and 0x8000… are adjacent but in different sub-sets.
        assert_eq!(
            tlc_min_gap([0x7fff_ffff_ffff_ffff, 0x8000_0000_0000_0000], 1),
            NO_GAP
        );
    }

    #[test]
    fn gap_matches_tlc_on_tlc_fingerprints() {
        // The figures TLC printed for these runs (and their goldens).
        let voteproof = read_fps("VoteProof");
        assert_eq!(voteproof.len(), 6_962);
        assert_eq!(figure(tlc_min_gap(voteproof, 1)), "8.6E-12");
        let nbacg = read_fps("nbacg_guer01");
        assert_eq!(nbacg.len(), 24_922);
        assert_eq!(figure(tlc_min_gap(nbacg.iter().copied(), 1)), "1.5E-11");
        // Without the sub-set rule, the closest pair straddles the top-bit split
        // and the figure would be 1.7E-10, not TLC's 1.5E-11.
        let masked = tlc_min_gap(nbacg.iter().map(|fp| fp & !(1 << 63)), 1);
        assert_eq!(figure(masked), "1.7E-10");
    }
}
