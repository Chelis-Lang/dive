//! Pure mathematical composition, using the ordinary generator for each
//! stage. Intermediate rows are private and never count as final successors.

use crate::eval::Machine;
use crate::plan::{Code, G};
use crate::{Error, ErrorKind, Handle, Sink};

impl Machine {
    fn composition_intermediates(
        &mut self,
        code: &Code,
        first: G,
    ) -> Result<Vec<Vec<Handle>>, Error> {
        struct Collect(Vec<Vec<Handle>>);
        impl Sink for Collect {
            fn emit(&mut self, machine: &mut Machine, _: &Code) -> Result<(), Error> {
                machine.charge(machine.s1.len() + 1)?;
                if machine.s1.iter().any(|h| h.is_empty()) {
                    return Err(Error::unsupported(
                        "action composition with an incomplete intermediate state",
                    ));
                }
                // Source enumeration may roll back its transient candidate
                // after this callback. Keep a closed row, including duplicates.
                let mut row = Vec::new();
                row.try_reserve_exact(machine.s1.len()).map_err(|_| {
                    Error::unsupported("action composition intermediate allocation")
                })?;
                for index in 0..machine.s1.len() {
                    row.push(machine.heap.intern(machine.s1[index])?);
                }
                self.0.try_reserve(1).map_err(|_| {
                    Error::unsupported("action composition intermediate allocation")
                })?;
                self.0.push(row);
                Ok(())
            }
        }
        let saved = std::mem::take(&mut self.s1);
        let assigned = self.assigned;
        self.s1 = vec![Handle::EMPTY; saved.len()];
        let mut rows = Collect(Vec::new());
        let result = self.run_action(code, first, &mut rows);
        self.s1 = saved;
        self.assigned = assigned;
        result.map(|()| rows.0)
    }

    /// Empties the frame's state-dependent thunk slots `reset`, so that each
    /// is forced again against the rows now current.
    fn clear_thunks(&mut self, reset: &[u32]) {
        for &slot in reset {
            self.set_slot(slot, Handle::EMPTY);
        }
    }

    /// Writes `values` back into the thunk slots `reset`.
    fn restore_thunks(&mut self, reset: &[u32], values: &[Handle]) {
        for (&slot, &value) in reset.iter().zip(values) {
            self.set_slot(slot, value);
        }
    }

    /// Runs each tail against its complete intermediate source, then exposes
    /// the actual original-to-final edge to the caller's sink. Every buffer
    /// and source is restored even when a tail, caller or checkpoint stops.
    ///
    /// `reset` lists this frame's thunk slots that read only the source row.
    /// A thunk forced against one source is never read against another: the
    /// slots hold the caller's values outside the stages and at the sink,
    /// and are emptied whenever a stage starts on a different source. The
    /// source is fixed within a stage. Thunks that read primed variables
    /// are evaluated by name in a composing frame, because a stage changes
    /// the successor it builds (`Compiler::reset_composition_thunks`).
    pub(super) fn composition(
        &mut self,
        code: &Code,
        first: G,
        then: G,
        reset: &[u32],
        sink: &mut dyn Sink,
    ) -> Result<(), Error> {
        struct Final<'a> {
            source: &'a [Handle],
            reset: &'a [u32],
            held: &'a [Handle],
            sink: &'a mut dyn Sink,
        }
        impl Sink for Final<'_> {
            fn emit(&mut self, machine: &mut Machine, code: &Code) -> Result<(), Error> {
                let intermediate = std::mem::replace(&mut machine.s0, self.source.to_vec());
                let mark = machine.saved.len();
                for &slot in self.reset {
                    let value = machine.slot(slot);
                    machine.saved.push(value);
                }
                machine.restore_thunks(self.reset, self.held);
                let result = self.sink.emit(machine, code);
                let forced = machine.saved.split_off(mark);
                machine.restore_thunks(self.reset, &forced);
                machine.s0 = intermediate;
                result
            }
        }
        let held: Vec<Handle> = reset.iter().map(|&slot| self.slot(slot)).collect();
        // Finish the source search before exposing any final row: a source
        // error must not disappear behind an earlier sink stop.
        self.clear_thunks(reset);
        let rows = self.composition_intermediates(code, first);
        let rows = match rows {
            Ok(rows) => rows,
            Err(error) => {
                self.restore_thunks(reset, &held);
                return Err(error);
            }
        };
        let source = std::mem::take(&mut self.s0);
        let successor = std::mem::take(&mut self.s1);
        let assigned = self.assigned;
        self.s1 = vec![Handle::EMPTY; successor.len()];
        let result = (|| {
            let mut output = Final {
                source: &source,
                reset,
                held: &held,
                sink,
            };
            for row in rows {
                self.charge(row.len() + 1)?;
                self.s0 = row;
                self.clear_thunks(reset);
                self.run_action(code, then, &mut output)?;
            }
            Ok(())
        })();
        self.restore_thunks(reset, &held);
        self.s0 = source;
        self.s1 = successor;
        self.assigned = assigned;
        result
    }

    /// The Boolean form used by step checks and all-assigned continuations.
    /// It uses the same two-stage relation, with an independent saved target.
    pub(crate) fn composition_holds(
        &mut self,
        code: &Code,
        first: G,
        then: G,
        reset: &[u32],
    ) -> Result<Handle, Error> {
        struct Match {
            target: Vec<Handle>,
            found: bool,
        }
        impl Sink for Match {
            fn emit(&mut self, machine: &mut Machine, _: &Code) -> Result<(), Error> {
                for (index, &target) in self.target.iter().enumerate() {
                    if !machine.demand_equals(machine.s1[index], target)? {
                        return Ok(());
                    }
                }
                self.found = true;
                Err(Error::stop())
            }
        }
        if self.s1.iter().any(|h| h.is_empty()) {
            return Err(Error::unsupported(
                "action composition predicate without a complete successor",
            ));
        }
        let mut matched = Match {
            target: self.s1.clone(),
            found: false,
        };
        match self.composition(code, first, then, reset, &mut matched) {
            Ok(()) => Ok(Handle::bool(matched.found)),
            Err(error) if matched.found && error.kind() == ErrorKind::Stop => Ok(Handle::TRUE),
            Err(error) => Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, reason = "small explicit machine controls")]

    use std::sync::Arc;

    use crate::eval::WorkControl;
    use crate::memo::Memo;
    use crate::plan::{GNode, Node};
    use crate::{Opts, Store};

    use super::*;

    struct Output {
        source: Handle,
        rows: Vec<Handle>,
        stop: bool,
    }
    impl Sink for Output {
        fn emit(&mut self, machine: &mut Machine, _: &Code) -> Result<(), Error> {
            assert_eq!(machine.s0, [self.source], "the actual edge's source");
            self.rows.push(machine.s1[0]);
            if self.stop {
                Err(Error::stop())
            } else {
                Ok(())
            }
        }
    }

    fn control() -> (Machine, Code) {
        let mut machine = Machine::new(
            Arc::new(Store::default()),
            1,
            Opts::default(),
            Memo::default(),
        );
        machine.s0[0] = Handle::int(0).expect("source");
        machine.s1[0] = Handle::int(9).expect("saved target");
        machine.assigned = 1;
        let code = Code {
            nodes: vec![
                Node::Const(Handle::int(1).expect("intermediate")),
                Node::Const(Handle::int(2).expect("final")),
            ],
            gnodes: vec![
                GNode::Assign { var: 0, rhs: 0 },
                GNode::Assign { var: 0, rhs: 1 },
                GNode::Disj(Box::new([0, 0])),
                GNode::Guard(0),
                GNode::Disj(Box::new([0, 3])),
            ],
            ..Code::default()
        };
        (machine, code)
    }

    fn restored(machine: &Machine) {
        assert_eq!(machine.s0, [Handle::int(0).expect("source")]);
        assert_eq!(machine.s1, [Handle::int(9).expect("saved target")]);
        assert_eq!(machine.assigned, 1);
        assert!(machine.conts.is_empty());
        assert_eq!(machine.saved, []);
        assert_eq!(machine.cvals, []);
    }

    #[test]
    fn composition_preserves_duplicates_and_restores_buffers_on_sink_stops() {
        for stop in [false, true] {
            let (mut machine, code) = control();
            let mut rows = Output {
                source: machine.s0[0],
                rows: Vec::new(),
                stop,
            };
            let result = machine.composition(&code, 2, 1, &[], &mut rows);
            if stop {
                assert_eq!(result.expect_err("sink stop").kind(), ErrorKind::Stop);
            } else {
                result.expect("complete");
            }
            assert_eq!(
                rows.rows,
                vec![Handle::int(2).expect("final"); if stop { 1 } else { 2 }]
            );
            restored(&machine);
        }
    }

    #[test]
    fn source_errors_precede_outward_rows_and_restore_the_caller() {
        let (mut machine, code) = control();
        let mut rows = Output {
            source: machine.s0[0],
            rows: Vec::new(),
            stop: true,
        };
        assert_eq!(
            machine
                .composition(&code, 4, 1, &[], &mut rows)
                .expect_err("source error")
                .kind(),
            ErrorKind::Eval
        );
        assert_eq!(rows.rows, []);
        restored(&machine);
    }

    #[test]
    fn composition_predicates_match_the_final_row_and_restore_the_target() {
        for target in [2, 3] {
            let (mut machine, code) = control();
            let successor = Handle::int(target).expect("target");
            machine.s1[0] = successor;
            assert_eq!(
                machine
                    .composition_holds(&code, 0, 1, &[])
                    .expect("relation"),
                Handle::bool(target == 2)
            );
            assert_eq!(machine.s0, [Handle::int(0).expect("source")]);
            assert_eq!(machine.s1, [successor]);
            assert_eq!(machine.assigned, 1);
            assert_eq!(machine.saved, []);
            assert!(machine.conts.is_empty());
        }
    }

    #[test]
    fn incomplete_intermediates_tail_errors_and_work_stops_restore_the_caller() {
        #[derive(Debug)]
        struct Halt;
        impl WorkControl for Halt {
            fn proceed(&self) -> bool {
                false
            }
        }
        for failure in 0..3 {
            let (mut machine, mut code) = control();
            if failure == 0 {
                code.nodes.push(Node::Const(Handle::TRUE));
                code.gnodes.push(GNode::Guard(2));
            } else if failure == 2 {
                machine.set_work_control(Some(Arc::new(Halt)));
            }
            let mut rows = Output {
                source: machine.s0[0],
                rows: Vec::new(),
                stop: false,
            };
            let first = if failure == 0 { 5 } else { 0 };
            let then = if failure == 1 { 3 } else { 1 };
            let error = machine
                .composition(&code, first, then, &[], &mut rows)
                .expect_err("failure");
            assert_eq!(
                error.kind(),
                [ErrorKind::Unsupported, ErrorKind::Eval, ErrorKind::Stop][failure]
            );
            assert_eq!(rows.rows, []);
            restored(&machine);
        }
    }

    #[test]
    fn state_thunks_are_forced_per_row_and_the_caller_keeps_its_own() {
        struct Thunk {
            held: Handle,
            rows: Vec<Handle>,
        }
        impl Sink for Thunk {
            fn emit(&mut self, machine: &mut Machine, _: &Code) -> Result<(), Error> {
                assert_eq!(machine.slot(0), self.held, "the caller's thunk at the sink");
                self.rows.push(machine.s1[0]);
                Ok(())
            }
        }
        let (mut machine, _) = control();
        let frame = machine.enter(1);
        let held = Handle::int(7).expect("caller's thunk");
        machine.set_slot(0, held);
        // first: x' \in {1, 2}; then: LET m == x IN x' = m
        let code = Code {
            nodes: vec![
                Node::Const(Handle::int(1).expect("one")),
                Node::Const(Handle::int(2).expect("two")),
                Node::Var(0),
                Node::Lazy { slot: 0, body: 2 },
            ],
            gnodes: vec![
                GNode::Assign { var: 0, rhs: 0 },
                GNode::Assign { var: 0, rhs: 1 },
                GNode::Disj(Box::new([0, 1])),
                GNode::Assign { var: 0, rhs: 3 },
            ],
            ..Code::default()
        };
        let mut rows = Thunk {
            held,
            rows: Vec::new(),
        };
        machine
            .composition(&code, 2, 3, &[0], &mut rows)
            .expect("complete");
        assert_eq!(
            rows.rows,
            [Handle::int(1).expect("one"), Handle::int(2).expect("two")]
        );
        assert_eq!(machine.slot(0), held);
        machine.leave(frame);
        restored(&machine);
    }
}
