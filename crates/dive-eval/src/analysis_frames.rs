//! Lexical binding shapes for value-independent, first-order predicate analysis.
use std::collections::HashMap;
use std::sync::{Arc, Weak};

use dive_ir::{DefId, ExprId};
use dive_value::Value;

use crate::env::{Env, Frame, Sv};
use crate::{Evaluator, Scope};

/// Interns lexical binding shapes within a single evaluator. Equal shapes preserve
/// every captured expression, binding kind and lexical parent. Concrete scalar
/// values can differ. A shape can only identify an analysis that does not inspect
/// those values. Retain each original scope for execution.
///
/// ```
/// # fn example(evaluator: &dive_eval::Evaluator, scope: &dive_eval::Scope) {
/// let mut frames = dive_eval::AnalysisFrames::new(evaluator);
/// if let Some(id) = frames.identify(scope) {
///     assert_eq!(frames.identify(&scope.clone()), Some(id));
/// }
/// # }
/// ```
pub struct AnalysisFrames<'a> {
    evaluator: &'a Evaluator,
    shapes: HashMap<Shape, usize>,
    slots: usize,
    // Holding the Weak prevents address reuse. A copy-on-write mutation either
    // allocates a fresh frame or dissociates its weak references.
    identities: HashMap<usize, (Weak<Frame>, Option<usize>)>,
}

#[derive(Eq, PartialEq, Hash)]
struct Shape {
    def: DefId,
    depth: u32,
    parent: Option<usize>,
    slots: Vec<Binding>,
}

#[derive(Eq, PartialEq, Hash)]
enum Binding {
    Unset,
    StateUnknown,
    ConstantUnknown,
    Scalar,
    Lazy(ExprId, usize, bool),
}

impl<'a> AnalysisFrames<'a> {
    /// Starts an empty table for one planning invocation.
    #[must_use]
    pub fn new(evaluator: &'a Evaluator) -> Self {
        Self {
            evaluator,
            shapes: HashMap::new(),
            slots: 0,
            identities: HashMap::new(),
        }
    }

    /// Returns a pool-local shape for scalar, first-order lexical environments.
    /// Opaque/dynamic bindings, foreign scopes and table exhaustion decline reuse.
    pub fn identify(&mut self, scope: &Scope) -> Option<usize> {
        if scope.wrapper.is_some() || self.evaluator.validate_scope(scope).is_err() {
            return None;
        }
        self.env(&scope.env, 0, &mut 2048)
    }

    fn env(&mut self, env: &Env, depth: usize, work: &mut usize) -> Option<usize> {
        if env.dynamic.at.is_some() || env.dynamic.cut.is_some() || env.dynamic.subst.is_some() {
            return None;
        }
        self.frame(&env.frame, depth, work)
    }

    fn frame(&mut self, frame: &Arc<Frame>, depth: usize, work: &mut usize) -> Option<usize> {
        let address = Arc::as_ptr(frame) as usize;
        if let Some((_, id)) = self.identities.get(&address) {
            return *id;
        }
        if depth > 128 || self.shapes.len() >= 16_384 || self.slots >= 65_536 {
            return None;
        }
        *work = work.checked_sub(frame.slots.len().saturating_add(1))?;
        let shape = self.shape(frame, depth, work);
        let id = shape.map(|shape| {
            let index = self.shapes.len();
            *self.shapes.entry(shape).or_insert_with_key(|shape| {
                self.slots += shape.slots.len();
                index
            })
        });
        // The address table only accelerates structural comparison. Dropping
        // entries cannot change the stable IDs of the interned shapes.
        if self.identities.len() >= 16_384 {
            self.identities.clear();
        }
        self.identities.insert(address, (Arc::downgrade(frame), id));
        id
    }

    fn shape(&mut self, frame: &Frame, depth: usize, work: &mut usize) -> Option<Shape> {
        let parent = match &frame.parent {
            Some(parent) => Some(self.frame(parent, depth + 1, work)?),
            None => None,
        };
        let mut slots = Vec::with_capacity(frame.slots.len());
        for slot in &frame.slots {
            slots.push(match slot {
                Sv::Unset => Binding::Unset,
                Sv::AnalysisBound => Binding::StateUnknown,
                Sv::AnalysisConstant => Binding::ConstantUnknown,
                Sv::Val(Value::Bool(_) | Value::Int(_) | Value::Str(_) | Value::Model(_)) => {
                    Binding::Scalar
                }
                Sv::Thunk(t) => {
                    Binding::Lazy(t.expr, self.env(&t.env, depth + 1, work)?, t.cachable)
                }
                Sv::Val(_) | Sv::Op(_) | Sv::Fcn(_) => return None,
            });
        }
        Some(Shape {
            def: frame.def,
            depth: frame.depth,
            parent,
            slots,
        })
    }
}
