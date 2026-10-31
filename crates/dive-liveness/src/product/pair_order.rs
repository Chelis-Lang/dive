//! Deterministic Cartesian ordering independent of the private pair index.

use super::{ProductError, csr};

/// Returns discovery IDs in Cartesian order and equally sized reusable scratch.
/// Growth is fallible; every 1,024 initialized or merged IDs polls the run budget.
pub(super) fn cartesian(
    pairs: &[(usize, usize)],
) -> Result<(Vec<usize>, Vec<usize>), ProductError> {
    order(pairs, &mut csr::poll)
}

fn order(
    pairs: &[(usize, usize)],
    poll: &mut impl FnMut() -> Result<(), ProductError>,
) -> Result<(Vec<usize>, Vec<usize>), ProductError> {
    let mut ids = csr::reserve(pairs.len())?;
    let mut scratch = csr::reserve(pairs.len())?;
    for id in 0..pairs.len() {
        if id.is_multiple_of(1024) {
            poll()?;
        }
        ids.push(id);
        scratch.push(0);
    }
    let mut width = 1_usize;
    while width < pairs.len() {
        let mut start = 0;
        while start < pairs.len() {
            let middle = start.saturating_add(width).min(pairs.len());
            let end = middle.saturating_add(width).min(pairs.len());
            let (mut left, mut right) = (start, middle);
            for (at, output) in scratch.iter_mut().enumerate().take(end).skip(start) {
                if at.is_multiple_of(1024) {
                    poll()?;
                }
                if right == end || (left < middle && pairs[ids[left]] <= pairs[ids[right]]) {
                    *output = ids[left];
                    left += 1;
                } else {
                    *output = ids[right];
                    right += 1;
                }
            }
            start = end;
        }
        std::mem::swap(&mut ids, &mut scratch);
        width = width.saturating_mul(2);
    }
    Ok((ids, scratch))
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    reason = "finite ordering and interruption assertions"
)]
mod tests {
    use super::*;

    #[test]
    fn cartesian_order_matches_ordered_exact_pairs_at_merge_boundaries() {
        for count in [0, 1, 2, 3, 31, 32, 33, 1023, 1024, 1025, 8193] {
            let pairs: Vec<_> = (0..count)
                .map(|index| ((index * 37) % 257, count - index))
                .collect();
            let expected: Vec<_> = pairs
                .iter()
                .copied()
                .enumerate()
                .map(|(id, pair)| (pair, id))
                .collect::<std::collections::BTreeMap<_, _>>()
                .into_values()
                .collect();
            let (ordered, scratch) = cartesian(&pairs).expect("Cartesian order");
            assert_eq!(ordered, expected);
            assert_eq!(scratch.len(), pairs.len());
        }
    }

    #[test]
    fn a_budget_stop_during_merging_returns_no_partial_order() {
        let pairs: Vec<_> = (0..4097).rev().map(|id| (id, id % 3)).collect();
        let mut inspections = 0;
        let result = order(&pairs, &mut || {
            inspections += 1;
            if inspections == 7 {
                Err(ProductError::Limit(crate::budget::EXHAUSTED))
            } else {
                Ok(())
            }
        });
        assert_eq!(result, Err(ProductError::Limit(crate::budget::EXHAUSTED)));
        assert_eq!(inspections, 7);
    }

    #[test]
    fn an_actual_expired_run_budget_refuses_ordering() {
        let watchdog = crate::budget::expired_for_test();
        let _scope = crate::budget::enter(watchdog.budget());
        assert_eq!(
            cartesian(&[(0, 0)]),
            Err(ProductError::Limit(crate::budget::EXHAUSTED))
        );
    }
}
