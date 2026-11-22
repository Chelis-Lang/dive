//! Bounded ordered product rows for multiple walks of the same unchanged graph.

use super::{Adjacency, ImplicitProduct, ProductError, SearchError, TruthAccess};
use crate::product::csr;

pub(super) struct RepeatedRows<'a> {
    roots: &'a [usize],
    offsets: Vec<u32>,
    // Each original behavior edge is needed even when targets are identical.
    edges: Vec<(u32, u32)>,
}

impl<'a> RepeatedRows<'a> {
    pub(super) fn buffer_bytes(&self) -> usize {
        self.offsets.capacity() * size_of::<u32>() + self.edges.capacity() * size_of::<(u32, u32)>()
    }

    pub(super) fn build<T: TruthAccess>(
        product: &'a ImplicitProduct<'_, T>,
        byte_limit: usize,
    ) -> Result<Option<Self>, ProductError> {
        let vertices = product.vertex_count();
        let count = product.edge_count;
        let size = |rows: usize, edges: usize| {
            rows.checked_mul(size_of::<u32>())?
                .checked_add(edges.checked_mul(size_of::<(u32, u32)>())?)
        };
        let Some(rows) = vertices.checked_add(1) else {
            return Ok(None);
        };
        if u32::try_from(vertices).is_err()
            || u32::try_from(count).is_err()
            || size(rows, count).is_none_or(|needed| needed > byte_limit)
        {
            return Ok(None);
        }
        let mut offsets = Vec::new();
        let mut edges = Vec::new();
        if offsets.try_reserve_exact(rows).is_err()
            || edges.try_reserve_exact(count).is_err()
            || size(offsets.capacity(), edges.capacity()).is_none_or(|needed| needed > byte_limit)
        {
            return Ok(None);
        }
        offsets.push(0);
        for v in 0..vertices {
            csr::poll()?;
            let mut cursor = product.cursor(v);
            while let Some((target, label)) = product
                .advance(v, &mut cursor)
                .map_err(ProductError::search)?
            {
                if edges.len().is_multiple_of(1024) {
                    csr::poll()?;
                }
                let (Ok(target), Ok(label)) = (u32::try_from(target), u32::try_from(label)) else {
                    return Ok(None);
                };
                if edges.len() >= count {
                    return Err(ProductError::Shape(
                        "more product edges than discovered".into(),
                    ));
                }
                edges.push((target, label));
            }
            offsets.push(
                u32::try_from(edges.len()).map_err(|_| ProductError::Limit("product edges"))?,
            );
        }
        if edges.len() != count {
            return Err(ProductError::Shape(
                "fewer product edges than discovered".into(),
            ));
        }
        Ok(Some(Self {
            roots: product.roots(),
            offsets,
            edges,
        }))
    }
}

impl Adjacency for RepeatedRows<'_> {
    type Cursor = usize;

    fn vertex_count(&self) -> usize {
        self.offsets.len() - 1
    }

    fn roots(&self) -> &[usize] {
        self.roots
    }

    fn cursor(&self, source: usize) -> usize {
        self.offsets[source] as usize
    }

    fn advance(
        &self,
        source: usize,
        cursor: &mut usize,
    ) -> Result<Option<(usize, usize)>, SearchError> {
        if *cursor == self.offsets[source + 1] as usize {
            return Ok(None);
        }
        let (target, label) = self.edges[*cursor];
        *cursor += 1;
        Ok(Some((target as usize, label as usize)))
    }
}
