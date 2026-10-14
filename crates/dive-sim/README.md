# Finite seeded simulation

`dive simulate Spec.tla --traces 3 --depth 100 --seed 7 --output json`

`dive simulate Spec.tla --compat=tlc --traces 3 --depth 100 --seed 7 --aril 3 --output json`

Rust callers use `dive_api::simulate_file` and `SimulateRequest`. The backend
consumes the existing bound model through `dive_sim::simulate`; it does not
call the exhaustive checker or the trace validator.

## Implemented contract

This first slice implements ordinary, single-worker, finite safety simulation
on pure predicates. TLC's optional JVM `Tool.probabilistic` mode is a different
algorithm and is explicitly unsupported. The initial predicates are enumerated
once, with duplicates. The entire initial vector is collected before any
completeness, invariant or constraint check. Initial-generation failures retain
the separate `initial_candidates` prefix; TLC's generated counter is assigned
only after enumeration finishes. Every initial invariant is checked before
constraints.
For each trace, select an initial state, choose an action, draw the prime retry
stride even when there is only one action, try disabled actions in TLC's stride
order, check every generated candidate, and select a surviving successor.
Invariant and action-property checks precede state/action constraints.

Under `--compat=tlc`, the worker selection stream is Java's RNG seeded from
`RandomGenerator(seed, aril).nextLong()`, kept across traces. As TLC does,
`--aril` has no effect without an explicitly supplied `--seed`. In the default
profile each trace has the existing counter-based stream keyed by seed and trace
number. Adding more traces preserves earlier trace documents. A fresh seed is
always reported when none is supplied. Nonzero aril in the default profile is
refused, rather than pretending it has TLC's meaning.

`--traces` is required and positive. Depth defaults to TLC's 100 transitions.
The final chosen state is included, so an uncompressed trace can contain
`depth + 1` states. No infinite trace count is silently selected.

## Basic action flows

`--stats basic --output json` (`SimulateRequest::action_stats = true`) records
chosen action pairs, including initial-predicate to first-action transitions.
This corresponds to TLC `-simulate num=N,stats=basic`; statistics default to off.
The sparse `action_flow.transitions` array retains raw counts, including stutters.
`actions` groups parameter contexts by the predicate's source file and location,
as TLC's basic action graph does. A group's label represents its first context;
`bound_action_ids` identifies all combined contexts. No DOT file, graph layout,
wall-clock update schedule or logarithmic display weight is emulated.

Counting runs after a candidate is chosen. Generated-state totals still count
all complete candidates checked, before constraints and properties, as before.
A duplicate candidate affects that total but contributes an action-flow edge only
if chosen. Excluded candidates, disabled attempts, violated invariants/action
properties and evaluation failures do not count as chosen transitions. Init
failures and depth-zero samples have empty flows. Resource limits retain the
completed chosen prefix. Disabled/preflight-refused statistics are `null`.
This does not implement source-expression coverage, full context statistics,
probabilistic generation or a new scheduling algorithm. Independent validation
and existing one-worker restrictions continue to apply.

## Evidence and limits

`tests/fixtures/choice-oracle.json` records ten clean pinned TLC runs: five
seeds and two arils, three traces each, with disabled actions and several
candidates per action. All 30 published traces (172 states after TLC's finite
stutter removal) and all 304 generated candidates/initial states match. Twelve
additional probes compare deciding message codes, exit statuses, available
counts, and invariant/action/deadlock counterexamples. Raw transcripts and
trace modules are retained. `tests/fixtures/capture.py` regenerates them from
an explicitly supplied jar and JRE and verifies the pinned jar's SHA-256.
No `-dumpTrace` flag changes the primary oracle's module overrides.

Every concrete trace is validated independently through `dive-trace`. An
undecidable violation becomes `unsupported`, with the observed candidate code
and validator refusal retained. A rejected candidate becomes `internal-error`.
Only an independently valid candidate is returned as a certified violation.
Negative controls corrupt a sampled edge; the validator rejects it. Another
control reaches a deadlock the finite-cover validator cannot decide; the API
refuses certification instead of accepting the checker candidate.

`document` contains the uncompressed concrete trace that was validated.
`tlc_states` is TLC's display projection with adjacent equal states removed;
it is not substituted for the concrete validation input. All reports contain
`exhaustive: false` and `model_proved: false`, even when sampling exits zero.
`sampled` means the requested finite sample completed without finding a bug.

Temporal/fairness simulation, runtime effects and random operators in the
specification, unresolved constant-operator effects, compound initial-state
deep normalization, implied-initial properties, action properties combined with state/action constraints, ALIAS, POSTCONDITION,
probabilistic/RL simulation, generation, distributed execution, continuation
after violations, and additional workers remain named refusals. The core
sampling stream is replayable; this slice does not claim worker scaling or
simulation-wide feature parity. TLC's asynchronous coordinator can execute
additional work after a worker finds a violation; the error controls use one
trace to avoid treating that race as a stable count contract.

The backend limits initial/candidate vectors to one million entries, retained
trace states to one million, generated states to ten million, and aril skipping
to one million doubles. Hitting a bound returns `resource-limit` with reached
execution stage and completed-prefix statistics, never `sampled`. The retained
bound includes initial invariant failures and failing successor states. When a
complete counterexample cannot fit, `pending_violation` retains the observed
property and code for diagnosis; it makes no certified violation claim. Preflight
refusals report no backend model evaluation; initialization and sampling
failures retain their actual worker use. These bounds protect this vertical
slice until streaming traces and broader execution modes land.

To reproduce the native CLI control report after building `dive-cli`:

```sh
python3 crates/dive-sim/tests/fixtures/compare.py --dive target/debug/dive \
  --report conformance/reports/simulation-controls.json
```

The report records the actual binary hash and all 23 selected controls, and
claims only their measured agreement. It does not certify the full simulation
feature family.

The pinned `SimConstrainedAction` probe reports action-property violation 2112
before its candidate is excluded by a state constraint. The exhaustive trace
validator currently requires constraints on property-ending edges. This slice
refuses the combination by name before sampling rather than weakening that
validator or reporting a false internal error. The control report accounts for
22 agreements and this one named refusal separately.

If a chosen value cannot be represented as a concrete JSON trace, or TLC
projection cannot compare two states, the API returns named `unsupported`
while retaining actual worker use and backend statistics. `statistics.scope`
describes backend work: a completed requested sample can retain complete
statistics even though its evidence cannot be serialized or certified.

The independent review
records the additional 1,344 budget controls and the discovered validator
contract boundary. F-024 and F-035 remain partial and audit-required.
