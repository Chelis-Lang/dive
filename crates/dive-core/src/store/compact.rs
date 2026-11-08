//! Quiescent reclamation of unreachable persistent values.
//!
//! Compiled handles retain their exact startup coordinates. Every other handle
//! must cross the explicit remapping transaction. A new owner is committed only
//! after every live root has been copied.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::{InternCache, Store, snapshot};
use crate::handle::Tag;
use crate::node::flags;
use crate::{Error, Handle};

fn invalid(message: impl Into<String>) -> Error {
    let message = message.into();
    if message.contains("allocation") || message.contains("address space") {
        Error::internal(format!("DIVE-S0203: native Store reclamation: {message}"))
    } else {
        Error::unsupported(format!("native Store reclamation: {message}"))
    }
}

/// Immutable compiled startup prefix whose handles may occur in the plan.
///
/// This owns a typed root image, never the growing search Store. It shares value
/// structure/authority checks with recovery without checkpoint wire admission.
pub struct Permanent {
    image: snapshot::Image,
}

impl Permanent {
    /// Captures the actual compiled startup before search work begins.
    ///
    /// # Errors
    /// Invalid ownership, unsupported runtime hosts, allocation or cancellation.
    pub fn capture(store: &Store, proceed: &(dyn Fn() -> bool + Sync)) -> Result<Self, Error> {
        Ok(Self {
            image: snapshot::memory_image(store, proceed).map_err(|error| {
                if error.message().contains("allocation") {
                    invalid(error.message())
                } else {
                    error
                }
            })?,
        })
    }

    /// Starts a replacement owner at a joined, completed-level boundary.
    ///
    /// The source remains live until all roots are copied. Dynamic name tokens
    /// retain their encounter order.
    ///
    /// # Errors
    /// A changed prefix, effectful/poisoned Store, allocation or cancellation.
    pub fn begin(
        &self,
        source: &Arc<Store>,
        proceed: &(dyn Fn() -> bool + Sync),
    ) -> Result<Remap, Error> {
        if source.poisoned() || source.draws().is_some() || source.registers().is_some() {
            return Err(invalid("poisoned or effectful owner"));
        }
        let mut image = self.image.duplicate(proceed)?;
        let names = source.strings.texts_from(0);
        if image.semantics != snapshot::semantics(source)
            || image.sealed != source.strings.is_sealed()
            || image.names.len() > names.len()
            || image.names.iter().zip(&names).any(|(a, b)| a != b.as_ref())
        {
            return Err(invalid("compiled semantics/name prefix changed"));
        }
        let mut mapped = HashMap::new();
        mapped
            .try_reserve(image.nodes.len())
            .map_err(|_| invalid("permanent node index allocation"))?;
        for (i, node) in image.nodes.iter_mut().enumerate() {
            if i.is_multiple_of(128) && !proceed() {
                return Err(Error::stop());
            }
            let handle = Handle(node.handle);
            let v = source
                .retained_view(handle)
                .ok_or_else(|| invalid("compiled node missing"))?;
            if v.kind != node.kind
                || v.flags & !flags::CONCRETE != node.flags & !flags::CONCRETE
                || v.lo != node.lo
                || v.hash != node.hash
                || v.sum != node.sum
                || v.data != node.data
            {
                return Err(invalid("compiled node changed"));
            }
            // The image drops the cached [`flags::CONCRETE`] proof. The node, its
            // children and every name token are unchanged in the replacement, so
            // the source's proof still holds there; without it every membership
            // or comparison against a compiled constant re-walks the value.
            node.flags = v.flags;
            mapped.insert(handle, handle);
        }
        image.names = names.iter().map(ToString::to_string).collect();
        let target = snapshot::install(source, &image, proceed)?;
        Ok(Remap {
            source: Arc::clone(source),
            target,
            mapped,
            cache: InternCache::default(),
        })
    }
}

/// One uncommitted replacement Store and its live-root identity mapping.
pub struct Remap {
    source: Arc<Store>,
    target: Arc<Store>,
    mapped: HashMap<Handle, Handle>,
    cache: InternCache,
}

impl Remap {
    /// The replacement owner, usable only with roots returned by this mapping.
    #[must_use]
    pub fn store(&self) -> &Arc<Store> {
        &self.target
    }

    /// Copies a closed persistent root.
    ///
    /// Shared nodes remain shared and no normalization/evaluation demand is
    /// introduced.
    ///
    /// # Errors
    /// A stale/transient/legacy-history value, new deferred provider, allocation
    /// or cancellation.
    #[allow(
        clippy::too_many_lines,
        reason = "one iterative child-first owned remapping transaction"
    )]
    pub fn copy(
        &mut self,
        root: Handle,
        proceed: &(dyn Fn() -> bool + Sync),
    ) -> Result<Handle, Error> {
        let mut stack = vec![(root, false)];
        let mut active = HashSet::new();
        let mut steps = 0usize;
        while let Some((h, leaving)) = stack.pop() {
            if steps.is_multiple_of(128) && !proceed() {
                return Err(Error::stop());
            }
            steps = steps.saturating_add(1);
            if h.tag() != Tag::Persistent {
                let valid = match h.tag() {
                    Tag::Int => true,
                    Tag::Bool => h.as_bool().is_some(),
                    Tag::Str | Tag::Model => h
                        .as_str_id()
                        .or_else(|| h.as_model_id())
                        .is_some_and(|id| self.target.strings.text(id).is_some()),
                    _ => false,
                };
                if !valid {
                    return Err(invalid("invalid scalar or transient live root"));
                }
                continue;
            }
            if self.mapped.contains_key(&h) {
                continue;
            }
            let v = self
                .source
                .retained_view(h)
                .ok_or_else(|| invalid("unknown live coordinate"))?;
            if v.is_lazy() || v.kind.is_lazy() {
                return Err(invalid("deferred provider added after startup"));
            }
            if v.flags & flags::HISTORY != 0 {
                return Err(invalid("legacy construction history"));
            }
            active
                .try_reserve(1)
                .map_err(|_| invalid("active traversal allocation"))?;
            if leaving {
                active.remove(&h);
                let mut children = Vec::new();
                children
                    .try_reserve_exact(v.data.len())
                    .map_err(|_| invalid("live child allocation"))?;
                for &child in v.data {
                    children.push(if child.is_persistent() {
                        *self
                            .mapped
                            .get(&child)
                            .ok_or_else(|| invalid("unmapped live child"))?
                    } else {
                        child
                    });
                }
                let mapped = self.target.intern(
                    &mut self.cache,
                    v.kind,
                    v.flags,
                    v.lo,
                    v.hash,
                    v.sum,
                    &children,
                );
                if mapped.is_empty() || self.target.poisoned() {
                    return Err(invalid("replacement node allocation"));
                }
                self.mapped
                    .try_reserve(1)
                    .map_err(|_| invalid("live node index allocation"))?;
                self.mapped.insert(h, mapped);
            } else {
                if !active.insert(h) {
                    return Err(invalid("cyclic live values"));
                }
                stack
                    .try_reserve(v.data.len() + 1)
                    .map_err(|_| invalid("live traversal allocation"))?;
                stack.push((h, true));
                stack.extend(v.data.iter().rev().map(|&child| (child, false)));
            }
        }
        Ok(self.mapped.get(&root).copied().unwrap_or(root))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, reason = "owned live-root reclamation controls")]
    use super::*;
    use crate::Heap;

    #[test]
    fn typed_permanent_preserves_canonical_wire_image() {
        let source = Arc::new(Store::with_semantics(true, crate::Semantics::tlc()));
        let mut heap = Heap::new(Arc::clone(&source), true, false);
        let one = Handle::int(1).expect("one");
        let two = Handle::int(2).expect("two");
        let local = heap.canonical_test_set(&[two, one, two]).expect("literal");
        let raw = heap.intern_exact(local).expect("raw");
        let normalized = heap.norm(local).expect("canonical");
        let normalized = heap.intern_exact(normalized).expect("normalized");
        let wire = snapshot::capture(&source, 1 << 20, &|| true).expect("wire image");
        let permanent = Permanent::capture(&source, &|| true).expect("typed image");
        let remap = permanent.begin(&source, &|| true).expect("replacement");
        assert_eq!(
            snapshot::capture(remap.store(), 1 << 20, &|| true).expect("same wire image"),
            wire
        );
        assert_eq!(remap.store().view(raw).data, &[one, two]);
        assert_eq!(remap.store().view(normalized).data, &[one, two]);
    }

    #[test]
    fn replacement_keeps_the_concrete_proof_of_compiled_constants() {
        let source = Arc::new(Store::with_semantics(true, crate::Semantics::tlc()));
        let mut heap = Heap::new(Arc::clone(&source), true, false);
        let one = Handle::int(1).expect("one");
        let two = Handle::int(2).expect("two");
        let inner = heap.canonical_test_set(&[two, one]).expect("literal");
        let inner = heap.norm(inner).expect("canonical");
        let constant = heap.tuple_literal(&[inner, one]);
        let constant = heap.intern_exact(constant).expect("constant");
        assert!(heap.proved_concrete(constant), "source proof");
        let permanent = Permanent::capture(&source, &|| true).expect("permanent");
        let remap = permanent.begin(&source, &|| true).expect("begin");
        let replaced = Heap::new(Arc::clone(remap.store()), true, false);
        assert!(replaced.proved_concrete(constant), "replacement proof");
        assert_eq!(
            remap.store().view(constant).flags & !flags::CONCRETE,
            source.view(constant).flags & !flags::CONCRETE
        );
    }

    #[test]
    fn copies_live_canonical_aliases_and_releases_expanded_values() {
        for hashcons in [false, true] {
            let source = Arc::new(Store::with_semantics(hashcons, crate::Semantics::tlc()));
            let mut heap = Heap::new(Arc::clone(&source), true, false);
            let one = Handle::int(1).expect("one");
            let two = Handle::int(2).expect("two");
            let constant = heap.tuple_literal(&[one]);
            let constant = heap.intern_exact(constant).expect("constant");
            let permanent = Permanent::capture(&source, &|| true).expect("permanent");
            for i in 0..2000 {
                let garbage = heap.tuple_literal(&[Handle::int(i).expect("integer"), two]);
                heap.intern_exact(garbage).expect("expanded value");
            }
            let local = heap.canonical_test_set(&[two, one, two]).expect("literal");
            let raw = heap.intern_exact(local).expect("raw");
            let normalized = heap.norm(local).expect("canonical");
            let normalized = heap.intern_exact(normalized).expect("normalized");
            let aliases = heap.tuple_literal(&[raw, raw, normalized, constant]);
            let aliases = heap.intern_exact(aliases).expect("aliases");
            let old_count = source.stats().nodes;
            let mut remap = permanent.begin(&source, &|| true).expect("begin");
            let aliases = remap.copy(aliases, &|| true).expect("copy");
            let target = remap.store();
            let view = target.view(aliases);
            assert_eq!(view.data[0], view.data[1]);
            assert_eq!(view.data[3], constant);
            assert_eq!(target.view(view.data[0]).data, &[one, two]);
            assert_eq!(target.view(view.data[2]).data, &[one, two]);
            assert!(old_count > target.stats().nodes + 1900);
            assert!(remap.copy(raw, &|| false).is_err());
        }
    }
}
