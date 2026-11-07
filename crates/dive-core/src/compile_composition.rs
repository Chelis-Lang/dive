//! The bounded mathematical composition slice: enumerable complete first
//! stages followed by pure, unprimed, fully assigning deterministic tails.

use dive_ir::ExprId;

use super::{Compiler, Mode};
use crate::Error;
use crate::plan::{G, GNode, Node, P};

/// Compilation state of source-changing composition.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum State {
    /// No compiled role contains composition.
    Absent,
    /// An operand is being compiled; nested composition refuses.
    Compiling,
    /// At least one compiled role changes its source row.
    Compiled,
}

/// A compiled composition, whose thunk resets are filled in once the whole
/// plan is compiled.
#[derive(Clone, Copy)]
pub(super) enum Site {
    /// A Boolean composition (`Node::Composition`).
    Value(P),
    /// A generated composition (`GNode::Composition`).
    Generator(G),
}

impl Compiler<'_> {
    pub(super) fn composition_value(
        &mut self,
        first: ExprId,
        then: ExprId,
        env: usize,
        mode: Mode,
    ) -> Result<P, Error> {
        if mode == Mode::Primed {
            return Err(Error::unsupported("primed action composition"));
        }
        let (first, then, free) = self.composition_parts(first, then, env)?;
        let p = self.push(Node::Composition {
            first,
            then,
            free: free.into(),
            reset: Box::new([]),
        })?;
        self.compositions.push((self.frame, Site::Value(p)));
        Ok(p)
    }

    pub(super) fn composition_gen(&mut self, first: G, then: G) -> G {
        let g = self.gpush(GNode::Composition {
            first,
            then,
            reset: Box::new([]),
        });
        self.compositions.push((self.frame, Site::Generator(g)));
        g
    }

    pub(super) fn composition_parts(
        &mut self,
        first: ExprId,
        then: ExprId,
        env: usize,
    ) -> Result<(G, G, Vec<u32>), Error> {
        if !self.h.action_composition {
            return Err(Error::unsupported("action composition is disabled"));
        }
        if self.init_mode || self.composition == State::Compiling || !self.lifts.is_empty() {
            return Err(Error::unsupported(
                "initial, nested or recursive action composition",
            ));
        }
        let nodes = self.info.len();
        let slots = self.next_slot;
        let previous = std::mem::replace(&mut self.composition, State::Compiling);
        let result = (|| {
            let first = self.g(first, env)?;
            let then = self.g(then, env)?;
            if self.info[nodes..].iter().any(|i| i.effect || i.calls) {
                return Err(Error::unsupported(
                    "action composition with effects or unproved calls",
                ));
            }
            let assigned = self.composition_tail(then, 0)?;
            let all = (0..self.ir.vars().len()).fold(0u64, |mask, var| mask | (1u64 << var));
            if assigned != all {
                return Err(Error::unsupported(
                    "action composition without a fully assigning deterministic tail",
                ));
            }
            let mut free = Vec::new();
            self.g_free(first, &mut free, 0, false)?;
            self.g_free(then, &mut free, 0, false)?;
            free.retain(|&slot| slot < slots);
            Ok((first, then, free))
        })();
        self.composition = if result.is_ok() {
            State::Compiled
        } else {
            previous
        };
        result
    }

    fn composition_tail(&self, g: G, assigned: u64) -> Result<u64, Error> {
        match self.code.gnodes.get(g as usize) {
            Some(GNode::Conj(items)) => items
                .iter()
                .try_fold(assigned, |seen, (g, _)| self.composition_tail(*g, seen)),
            Some(GNode::Scope { body, .. }) => self.composition_tail(*body, assigned),
            Some(GNode::Assign { var, rhs }) => {
                let bit = 1u64 << var;
                let info = &self.info[*rhs as usize];
                if assigned & bit != 0 || info.primed != 0 || info.effect || info.calls {
                    return Err(Error::unsupported(
                        "action composition tail with repeated, primed or unproved assignments",
                    ));
                }
                Ok(assigned | bit)
            }
            _ => Err(Error::unsupported(
                "action composition without a fully assigning deterministic tail",
            )),
        }
    }

    /// A composition evaluates its stages on rows other than its frame's
    /// source and successor. Keyed memo sites still read their actual state
    /// keys; the frame's state-dependent thunks are instead reset by the
    /// composition whenever it changes rows. Thunks of other frames need no
    /// reset: a callee frame starts empty on each call, and call arguments
    /// are values, so the stages read no other frame's slots.
    pub(super) fn reset_composition_thunks(&mut self) {
        if self.composition != State::Compiled {
            return;
        }
        for (frame, site) in std::mem::take(&mut self.compositions) {
            let mut slots: Vec<u32> = self
                .lazies
                .iter()
                .filter(|&&(f, _)| f == frame)
                .filter_map(|&(_, p)| {
                    let info = self.info.get(p as usize)?;
                    let state = !info.effect && (info.vars != 0 || info.primed != 0 || info.calls);
                    match self.code.nodes.get(p as usize) {
                        Some(Node::Lazy { slot, .. }) if state => Some(*slot),
                        _ => None,
                    }
                })
                .collect();
            slots.sort_unstable();
            slots.dedup();
            let slots: Box<[u32]> = slots.into();
            match site {
                Site::Value(p) => {
                    if let Some(Node::Composition { reset, .. }) =
                        self.code.nodes.get_mut(p as usize)
                    {
                        *reset = slots;
                    }
                }
                Site::Generator(g) => {
                    if let Some(GNode::Composition { reset, .. }) =
                        self.code.gnodes.get_mut(g as usize)
                    {
                        *reset = slots;
                    }
                }
            }
        }
    }
}
