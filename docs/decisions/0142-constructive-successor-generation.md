# 0142: Constructive successor generation

- **Status:** accepted
- **Date:** 2026-11-02
- **Extends:** [ADR 0116](0116-simulation-observable-contract.md)

## Context

Ordinary finite simulation enumerates an action's complete candidate vector
before choosing a successor. Large nondeterministic relations can spend most
of a sampling run constructing discarded candidates. Pinned TLC `-generate`
selects simulation with constructive successor traversal; it is not a dump or
trace-export command. Its random draw history is excluded from observable
parity by ADR 0116.

## Decision

Expose the existing TLC mode as `simulate --generate`, `SimulateRequest::generation`
and `dive_sim::Options::generation`, default false. The TLC shim transports only
explicit finite `-generate num=N` requests. Both existing producer engines
implement the feature; actual worker count remains one. Additive report fields
name the actual `constructive` or `enumerating` algorithm. Report schema 3 and
trace versions retain their meanings.

Only actual Next disjunctions and bounded existential contexts consume the
selected operational generation stream. Each index walk chooses a random
start and stride, falling back to stride one when the sampled stride is not
coprime with the domain length. It visits every index at most once per traversal
using bounded index state. Empty domains never draw; out-of-range low-level
chooser results return typed unsupported. Intervals use direct arithmetic;
stored sets preserve ordinary normalization before indexing existing members.
No candidate vector or interval expansion is needed merely to choose a member.

Evaluate and admit every binder source domain before context traversal, retain
ordinary first-unit-fastest odometer order and tuple/scalar binding semantics,
and never re-evaluate inner source domains per outer assignment. Predicate
quantifiers, CHOOSE, Init, function construction and ENABLED stay ordinary.
Opaque or unproved domains refuse at demand, without speculative effect retries.
CASE and action composition generation retain explicit refusals.

The driver stops only after its actual complete candidate passes ordinary
invariant/action-property checks and constraints. Rejected constraints continue
traversal; discarded choices cannot establish deadlock. Partial rows remain
errors. Chosen native handles survive cleanup, and selected sink Stop is distinct
from cancellation or deferred evaluator errors. This algorithm changes sampling
choices, not the relation being sampled. It grants no general RandomElement
proof or producer-supplied RNG certificate authority. Reproduction requires the
same seed, engine, source and options; TLC draw equality is not required.

Introduce an explicit independent sample-validation operation for native Behavior
words: finite Init/Next, constraints, invariants and implied initial/action
obligations are checked on fresh runtimes. Ordinary Behavior callers keep their
existing relation-only semantics. Sample validation asserts no exhaustive,
deadlock or liveness result; fairness-only specifications are admissible, while
configured temporal properties and unproved projections refuse. Error and
violation words retain their ordinary independent proof operation. CLI
`validate-trace --sample` labels JSON `validation_goal: sample` without changing
the word wire format.

The narrow native Simulate conformance path requires actual schema, request,
profile, algorithm, execution stage, engine, work partition and physical exit
facts, and fresh independent validation of every retained word. Producer
validation booleans cannot authorize results. Complete samples remain explicitly
non-exhaustive; general sample count/draw equality is not a gate. Deterministic
tiny controls retain exact observable checks. Runtime-error wrappers lacking an
independently owned endpoint remain unsupported. Exhaustive comparison rules
are unchanged.

Simulation accepts additive `--exit-codes native|tlc` and API exit-convention
methods. The absent flag preserves existing dispositions; the TLC shim and
conformance path explicitly request actual TLC exits.

## Consequences

Wide nondeterminism no longer requires retaining all successors in the producer.
Independent relational replay may still inspect many choices; no constant-time
end-to-end claim follows. Memory budgets remain sampled process limits, not a
proof of total constant memory. General effects, RandomElement relations,
additional workers, DFID and coverage remain separate work. Provenance and
minimal surprising controls live in `docs/provenance.md` and the constructive
generation corpus.
