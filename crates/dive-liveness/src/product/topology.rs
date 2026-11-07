//! Checked borrowed adjacency for compact behavior producers.

use super::ProductError;
use crate::GraphError;

/// A validated borrowed CSR behavior topology, without state values or labels.
///
/// `offsets` has one more entry than the number of vertices; each adjacent
/// pair selects a row in `targets`. Targets use 32-bit dense vertex ids. Roots,
/// parallel edges, row order and empty rows are retained exactly. A dead end
/// does not acquire an implicit stutter.
///
/// The view neither owns nor copies the arrays. It certifies their references,
/// not graph completion, temporal acceptance or a bound specification.
///
/// ```
/// use dive_liveness::product::CsrTopology;
/// let offsets = [0, 2, 3];
/// let targets = [1, 0, 1];
/// let roots = [0];
/// let topology = CsrTopology::new(&offsets, &targets, &roots)?;
/// assert_eq!(topology.vertex_count(), 2);
/// assert_eq!(topology.edge_count(), 3);
/// assert_eq!(topology.roots(), &[0]);
/// # Ok::<(), dive_liveness::product::ProductError>(())
/// ```
#[derive(Clone, Copy, Debug)]
pub struct CsrTopology<'a> {
    offsets: &'a [usize],
    targets: &'a [u32],
    roots: &'a [usize],
}

impl<'a> CsrTopology<'a> {
    /// Checks the full CSR layout and every root and target reference.
    ///
    /// Layout checks precede reference checks. As in [`crate::Graph::new`],
    /// roots are checked before targets, which are checked in row order.
    /// The work is linear in the supplied physical slices and allocates no
    /// adjacency storage.
    ///
    /// # Errors
    /// An empty, non-monotone or out-of-range offset table; trailing or omitted
    /// targets; a vertex domain wider than 32 bits; or an invalid root/target.
    pub fn new(
        offsets: &'a [usize],
        targets: &'a [u32],
        roots: &'a [usize],
    ) -> Result<Self, ProductError> {
        let bad = |why: &str| ProductError::Shape(why.into());
        let Some(vertices) = offsets.len().checked_sub(1) else {
            return Err(bad("CSR offsets must contain an initial zero"));
        };
        if u32::try_from(vertices.saturating_sub(1)).is_err() {
            return Err(bad("CSR vertex ids exceed 32 bits"));
        }
        if offsets.first() != Some(&0)
            || offsets.last() != Some(&targets.len())
            || offsets
                .windows(2)
                .any(|pair| pair[0] > pair[1] || pair[1] > targets.len())
        {
            return Err(bad("CSR offsets do not exactly partition targets"));
        }
        let malformed =
            |vertex, source| ProductError::Shape(GraphError { vertex, source }.to_string());
        for &root in roots {
            if root >= vertices {
                return Err(malformed(root, None));
            }
        }
        for (source, row) in offsets.windows(2).enumerate() {
            for &target in &targets[row[0]..row[1]] {
                let target = usize::try_from(target)
                    .map_err(|_| bad("CSR target does not fit the host vertex domain"))?;
                if target >= vertices {
                    return Err(malformed(target, Some(source)));
                }
            }
        }
        Ok(Self {
            offsets,
            targets,
            roots,
        })
    }

    /// Borrows references whose layout was already checked under producer
    /// control. This grants topology access only, never completion authority.
    pub(crate) fn from_capture(graph: &dive_runtime::capture::CsrCapture<'a>) -> Self {
        Self {
            offsets: graph.offsets(),
            targets: graph.targets(),
            roots: graph.roots(),
        }
    }

    /// Number of behavior vertices, including unreachable vertices.
    #[must_use]
    pub const fn vertex_count(&self) -> usize {
        self.offsets.len() - 1
    }

    /// Number of supplied behavior edges, including duplicates and stutters.
    #[must_use]
    pub const fn edge_count(&self) -> usize {
        self.targets.len()
    }

    /// Initial roots in the caller's original order, including duplicates.
    #[must_use]
    pub const fn roots(&self) -> &'a [usize] {
        self.roots
    }

    pub(super) fn row_len(&self, source: usize) -> usize {
        self.offsets[source + 1] - self.offsets[source]
    }

    pub(super) fn target(&self, source: usize, edge: usize) -> usize {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "the constructor checked every target against the host vertex domain"
        )]
        let target = self.targets[self.offsets[source] + edge] as usize;
        target
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_layouts_fail_before_any_adjacency_access() {
        for (offsets, targets) in [
            (vec![], vec![]),
            (vec![1], vec![]),
            (vec![0, 2, 1], vec![0]),
            (vec![0, 2], vec![0]),
            (vec![0, 1], vec![0, 0]),
            (vec![0, usize::MAX], vec![]),
        ] {
            assert!(matches!(
                CsrTopology::new(&offsets, &targets, &[]),
                Err(ProductError::Shape(_))
            ));
        }
    }

    #[test]
    fn invalid_roots_and_targets_follow_dense_reference_order() {
        let offsets = [0, 2, 3];
        let targets = [1, 9, 8];
        for roots in [vec![7, 6], vec![6], vec![]] {
            let expected = crate::Graph::new(vec![vec![1, 9], vec![8]], roots.clone())
                .err()
                .map(|error| ProductError::Shape(error.to_string()));
            let actual = CsrTopology::new(&offsets, &targets, &roots).err();
            assert_eq!(actual, expected);
        }
        assert!(CsrTopology::new(&[0], &[], &[0]).is_err());
        assert!(CsrTopology::new(&[0], &[], &[]).is_ok());
    }

    #[test]
    fn topology_borrows_physical_arrays_without_deduplicating_or_reordering()
    -> Result<(), ProductError> {
        let offsets = [0, 3, 3, 4];
        let targets = [2, 1, 1, 2];
        let roots = [2, 0, 2];
        let topology = CsrTopology::new(&offsets, &targets, &roots)?;
        assert_eq!(topology.offsets.as_ptr(), offsets.as_ptr());
        assert_eq!(topology.targets.as_ptr(), targets.as_ptr());
        assert_eq!(topology.roots().as_ptr(), roots.as_ptr());
        assert_eq!(topology.vertex_count(), 3);
        assert_eq!(topology.edge_count(), 4);
        assert_eq!(topology.row_len(1), 0);
        assert_eq!(topology.target(0, 0), 2);
        assert_eq!(topology.target(0, 1), 1);
        assert_eq!(topology.target(0, 2), 1);
        Ok(())
    }
}
