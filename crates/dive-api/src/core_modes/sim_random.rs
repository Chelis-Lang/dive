//! `RandomElement` and the `Randomization` operators in simulation (ADR 0162
//! §8): each draw comes from `Streams::simulation_random`, keyed by the
//! trace, the step, the action and the attempt, so draws are a function of
//! the seed and the trace position, whatever worker samples the trace.
//!
//! A draw stands for some member of its set (owner ruling, 2026-11-20): the
//! machine logs every value drawn (`Machine::random_log`), the trace
//! document records the draws of each step, and the validator checks each
//! recorded draw against its own evaluation of the set. A draw is never
//! evidence that no value works: a step whose draws admit no successor is
//! drawn again and never reported as a deadlock, a draw in a state check
//! (invariant, constraint, property) or inside `ENABLED` is refused by name.

use dive_core::random::DrawStream;
use dive_core::{Error, Machine};
use dive_ir::StdOp;
use dive_stdlib::random::{Stream, Streams, random_element, random_set_of_subsets, random_subset};
use dive_value::{Ctx, Value};

/// The draw stream of one trace position.
#[derive(Debug)]
struct Position {
    stream: Stream,
}

impl DrawStream for Position {
    fn draw(&mut self, op: StdOp, args: &[Value], ctx: Ctx) -> Result<Value, Error> {
        match (op, args) {
            (StdOp::TlcRandomElement, [s]) => random_element(s, ctx, &mut self.stream)
                .map_err(|e| crate::core_random::draw_error(&e)),
            (StdOp::RandomizationRandomSubset, [k, s]) => {
                random_subset(k, s, ctx, &mut self.stream)
                    .map_err(|e| crate::core_random::draw_error(&e))
            }
            (StdOp::RandomizationRandomSetOfSubsets, [k, n, s]) => {
                random_set_of_subsets(k, n, s, ctx, &mut self.stream)
                    .map_err(|e| crate::core_random::draw_error(&e))
            }
            _ => Err(Error::unsupported(format!(
                "{} in simulation",
                op.qualified_name()
            ))),
        }
    }

    fn simulation(&self) -> bool {
        true
    }
}

/// Init enumerated once, before any trace: a draw there means each trace
/// enumerates Init with its own draws instead. The machine counts the
/// attempt (`Machine::draws`) before this refuses it.
#[derive(Debug)]
struct Probe;

impl DrawStream for Probe {
    fn draw(&mut self, op: StdOp, _: &[Value], _: Ctx) -> Result<Value, Error> {
        Err(Error::unsupported(format!(
            "{}: a draw while Init is enumerated once",
            op.qualified_name()
        )))
    }

    fn simulation(&self) -> bool {
        true
    }
}

/// A state or step check (invariant, constraint, action constraint,
/// implied property): its verdict would rest on one draw.
#[derive(Debug)]
struct Check;

impl DrawStream for Check {
    fn draw(&mut self, op: StdOp, _: &[Value], _: Ctx) -> Result<Value, Error> {
        Err(Error::unsupported(format!(
            "{} in an invariant, constraint or property in simulation, whose verdict would rest on one draw",
            op.qualified_name()
        )))
    }

    fn simulation(&self) -> bool {
        true
    }
}

/// Installs the stream of trace `trace`'s position `[step, action,
/// attempt]` and starts an empty draw log.
pub(super) fn position(machine: &mut Machine, streams: Streams, site: [u64; 4]) {
    let [trace, step, action, attempt] = site;
    machine.draw = Some(Box::new(Position {
        stream: streams.simulation_random(trace, step, action, attempt),
    }));
    machine.random_log = Some(Vec::new());
}

/// Installs the stream that only detects a draw while Init is enumerated
/// once.
pub(super) fn probe(machine: &mut Machine) {
    machine.draw = Some(Box::new(Probe));
    machine.random_log = None;
}

/// Runs `check` with draws refused, then restores the position's stream.
/// Without a stream (model checking, DFID) it runs `check` unchanged.
pub(super) fn checking<T>(machine: &mut Machine, check: impl FnOnce(&mut Machine) -> T) -> T {
    if machine.draw.is_none() {
        return check(machine);
    }
    let stream = machine.draw.replace(Box::new(Check));
    let result = check(machine);
    machine.draw = stream;
    result
}

/// The values drawn at the current position so far, interned.
pub(super) fn drawn(machine: &Machine) -> &[dive_core::Handle] {
    machine.random_log.as_deref().unwrap_or(&[])
}
