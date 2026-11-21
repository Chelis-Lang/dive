# Independent trace validation

`dive validate-trace` replays a trace against a specification and
configuration, independently of the checker that produced it, and answers
`valid`, `invalid` or `unsupported`. Use it on a saved counterexample, a
simulation sample, or a state sequence your implementation logged.

```sh
dive check docs/user/examples/Counter.tla --config docs/user/examples/CounterViolation.cfg --compat=tlc --seed 7 --fp 0 --output json > counter-result.json
# The invariant violation exits 12; keep the report.
python3 -c 'import json; r=json.load(open("counter-result.json")); json.dump(r["counterexample"]["trace"], open("counter-trace.json", "w"))'
dive validate-trace docs/user/examples/Counter.tla --config docs/user/examples/CounterViolation.cfg --trace counter-trace.json --compat=tlc --seed 7 --fp 0 --output json
```

Pass the seed and fingerprint settings of the original run, which binding may
depend on, and the same profile. The JSON report has `schema_version: 4`,
`command: "validate-trace"` and `result`. The report version is separate from
the trace document's own version. TLC `dumpTrace` JSON is also accepted.

## What validation checks

The validator rebuilds binding and startup from the original source and
configuration for every validation, with its own evaluator, interner,
registers and caches. It then checks:

- that the first state is one Init produces, and every later state
  follows from the previous one by the original next-state relation;
- every configured safety obligation along the way, and the claimed ending;
- for a lasso, the closing edge, the complete property claims and fairness.

Every state must assign every variable; missing values are never guessed.
Whether a state has a successor (inside a predicate, for a deadlock ending,
and for `WF` and `SF` on a lasso) is decided by the same `ENABLED` search the
checker and TLC use. An `ENABLED` with side effects, or one that cannot be
decided, makes the result `unsupported`.

`unsupported` names what could not be checked; it is never a decision. These
always give it: executed random draws (`RandomElement`, `Randomization`),
register access after Init, missing finite deadlock or `ENABLED` covers,
effects, and unknown closing evidence. Decoding a document is separate from
validating it, and the trace format has known representation gaps: a checker
can emit a JSON set holding a sequence before a record that the decoder then
rejects, in either profile.

The lasso back edge in a trace document is zero-based; the checker summary's
`back_to` is one-based. Validate the trace document rather than rebuilding a
lasso from displayed states.

## Simulation samples

For a finite simulation word, add `--sample`. This checks Init, Next and all
configured finite safety obligations on the rows (constraints, invariants and
implied initial and action properties), and the report says
`validation_goal: "sample"`. A plain Behavior validation checks only the
relation. A sample may also end in `deadlock`: every successor of the last
state must then fall outside the `CONSTRAINT`s and `ACTION_CONSTRAINT`s.
Other endings are rejected, and unproved projections refuse. A valid sample
claims only its finite obligations: configured temporal properties and fairness
constrain infinite behaviors, which a finite sample does not have, and
simulation refutes a temporal property only with a lasso or a `bad-prefix`
word. A valid sample proves neither exhaustive safety nor liveness. Rust callers use
`SpecModel::try_validate_sample`, or `Session::read_sampled` and
`Session::read_sampled_deadlock`.

## Trace format versions

A counterexample is a JSON document with `"format": "dive-trace"` and a
`version` set by how the trace ends:

| Version | Ending | Section |
| ---: | --- | --- |
| 1 | Invariant, action-property and liveness violations, deadlock, and the error endpoints `next-before-successor`, `next-ordered-terminal` and temporal state rows | [Error endpoints](#error-endpoints) |
| 2 | `invariant-error`: an error while evaluating a named invariant | [Named invariant errors](#named-invariant-errors) |
| 3 | A partial terminal candidate: a Next step that leaves variables unassigned, or an incomplete Init | [Partial candidates](#partial-candidates) |
| 4 | `candidate-ordered-terminal`: an error on a generated candidate | [Candidate errors](#candidate-errors) |
| 5 | `temporal-source-row`: an error in a temporal property's source row | [Temporal source-row errors](#temporal-source-row-errors) |
| 6 | `view-source-terminal`: an error while evaluating VIEW | [VIEW errors](reports-and-errors.md#view-errors) |
| 7 | `bad-prefix`: a finite refutation of a temporal property | [Bad prefixes](#bad-prefixes) |

Each ending has exactly one version, and a document may not be relabeled with
another. Readers accept all seven and fail closed on an unknown version or
ending. Tags, endings and loop indexing follow the
[compatibility policy](reports-and-errors.md#report-compatibility).

Validation of an error ending re-runs the real evaluation on fresh runtimes:
the claimed source and codes are hints to confirm, never instructions about
what to evaluate. Use the fallible Rust entry points for these endings (see
[Rust APIs](#rust-apis)); the legacy infallible ones refuse them.

## Endings

### Bad prefixes

`{"kind": "bad-prefix", "name": P}` claims that every infinite behavior that
starts with the trace's states violates the configured property `P`, as
`(x = 0) => [](x < 3)` is violated by any behavior starting `x = 0, 1, 2, 3`.
Simulation reports these words (`dive simulate`). The validator checks every
state and step as for an invariant ending (Init, Next, constraints,
invariants, implied initial and action properties), then evaluates `P` on the
finite word in three-valued logic: state predicates are known at every
state, an action is unknown on the step out of the last state, and `[]`,
`<>`, `~>`, `WF` and `SF` see an unknown future after it. The word is valid
only if `P` is definitely false at its first state. A property such as
`<>Q` or `[]<>Q` can never be refuted this way.

### Named invariant errors

An invariant evaluation error carries `kind: "invariant-error"`, the configured
`name` and the underlying error's `ec`, for example `TLC_VALUE_ASSERT_FAILED`.
The check report normally uses the deciding class
`TLC_INVARIANT_EVALUATION_FAILED` (2111) and TLC exit 76. The validator must
reproduce the cause at the named invariant after all earlier states and
earlier invariants pass. A wrong name or cause, an absent error or an earlier
failure makes the claim invalid; a false Boolean invariant is not an
evaluation error.

```json
{"format":"dive-trace","version":2,"variables":["x"],"states":[{"action":null,"values":{"x":0}},{"action":null,"values":{"x":1}},{"action":null,"values":{"x":2}}],"ending":{"kind":"invariant-error","name":"Inv","ec":"TLC_VALUE_ASSERT_FAILED"}}
```

This ending requires version 2. A later diagnostic replay can hit a different
error, so the report's deciding code (for example `TLC_NESTED_EXPRESSION`,
2103, after a first report of 2111, or the underlying cause's own code) can
differ from the trace. The trace keeps the original invariant and cause, and
validation proves that claim only, not the later class or counts.

An initial receiver error keeps `counterexample` null and puts its one-state
word at `diagnostic_witness.trace`, still version 2 with the invariant name and
`GENERAL` cause. Validating it proves that cause, not the fingerprint wrapper
2147 or first report 2102. To confirm those, reconstruct the first Init
emission and invariant with `SpecModel::initial_invariant_class`.

### Constraints on the last state

For a named invariant violation or evaluation error, or a named initial or
action property violation, the last state may be outside `CONSTRAINT`, or its
step outside `ACTION_CONSTRAINT`. Every earlier state and step must satisfy
them. Constraints are evaluated in order and stop at the first false one; an
evaluation error or non-Boolean result invalidates a claim to a later failure.
An anonymous error ending cannot stand in for a named invariant error.

### Error endpoints

An evaluation error can end with `kind: "error"` and an `endpoint` naming its
stage and source:

- `next-before-successor`: an error in Next before the first candidate. Replay
  runs the configured actions in order and must hit the claimed code and source
  before any complete or partial successor. Emitting a candidate first refuses;
  finishing without an error invalidates the claim.
- `next-ordered-terminal`: also carries `reported` and `deciding` codes, which
  replay confirms by running the per-action diagnostic sequence. Exact
  assertion 2132 also needs this.
- A temporal state-row error reports its stage and source; see
  [errors on a source row](temporal.md#errors-on-a-source-row).

```json
{"kind":"error","ec":"TLC_MODULE_DIVISION_BY_ZERO","endpoint":{"stage":"next-before-successor","source":{"module":"M","start":40,"end":49,"instance":null}}}
```

These words are version 1. An endpoint is not a lasso, violation, count or
completion proof, and a generic error without one is `unsupported`. Earlier
complete candidates get the usual completeness, constraint, fingerprint and
safety checks. These refuse by name: unknown newness or alias history, raw or
symbolic fingerprints, `VIEW`, symmetry, temporal obligations, executed effects,
and nested, effectful or non-TLA composition. Pure bounded composition in
`tla` mode can reconstruct the source error.

### Partial candidates

A Next action that leaves variables unassigned ends with a version-3 partial
descriptor. `states` holds only the complete prefix; the ending names the
assigned values, the unassigned variables (together they cover every variable)
and the generating source. Logical `length` counts the partial candidate:
`ErNextIncomplete` has three complete rows plus a partial `x = 3` with `y`
unassigned, so length four. Use `logical_length()` rather than counting rows.

```sh
dive check crates/dive-api/tests/fixtures/checker/errors/specs/ErNextIncomplete/ErNextIncomplete.tla --workers 1 --seed 0 --fp 0 --output json > partial-result.json
# The incomplete successor exits 75; keep the report.
python3 -c 'import json; r=json.load(open("partial-result.json")); json.dump(r["counterexample"]["trace"], open("partial-trace.json", "w"))'
dive validate-trace crates/dive-api/tests/fixtures/checker/errors/specs/ErNextIncomplete/ErNextIncomplete.tla --trace partial-trace.json --seed 0 --fp 0 --output json
```

From a source checkout, this replays the prefix with every constraint,
invariant and implied safety property, then the generator, which must produce
the same first incomplete candidate. Earlier complete candidates get the same
ordered checks; `RoActions` and `SplitThrows` are examples. A resource or
control failure returns no completed validation report.

An incomplete Init keeps `counterexample` null and gives an `error_witness`
instead; extract that in the example above and use `ErInitIncomplete`. Its
error exits 255, and the version-3 word has no complete rows, logical length
one and `init-partial-state` with `TLC_INITIAL_STATE`. Later Init candidates
that need global newness refuse. `TLCGet("level")` is 1 in initial candidate
checks and 0 while Init generates; action checks use their predecessor's level.
A primed `TLCGet("level")` refuses the validation.

### Candidate errors

An error on a generated candidate ends with version 4 and a
`candidate-ordered-terminal` endpoint carrying `site`, `source`, `reported`
and `deciding` codes. The last complete row is the candidate itself, not the
state whose expansion failed. Sites are `constraint`, `action-constraint` and
`implied-action`. Replay must find the same first error on the same candidate
and run the diagnostic checks to confirm the deciding code. `VIEW`, unknown
newness or alias history, a named role that returns a non-Boolean, raw
fingerprints and executed effects refuse. An expression such as `[A]_e` whose
inner action is not Boolean raises a source error that replay can confirm
(the `ErPropNonBool` control). The witness proves no BFS counts.

### Temporal source-row errors

An error in a temporal property's source row after Next ends with version 5 and
a `temporal-source-row` endpoint (`module`, byte `start` and `end`, nullable
`instance`); `ec` names the underlying error. The last complete row is the
failing source. Replay enumerates Next and its candidate checks, confirms that
the source's only successor is itself, runs the temporal checks in order and
derives GENERAL (1000), exit 255. The word has one state and exactly one Init
emission; longer prefixes, several Init emissions, other targets and reached
tableau work refuse. No lasso or property violation is claimed.

## TLC-profile names and randomness

Under `--compat=tlc`, strings and model values compare in the order TLC first
saw them. The validator knows that order for every name created while the
model binds. If reading or checking the trace creates another name (a string
the specification never mentions, or one that `ToString` or `Tail` builds),
the result is `unsupported: runtime string encounter order`. Replay in this
profile also accepts bounded tuple-valued variables, tuple sequence calls and
finite tuple stacks owned by a process; arbitrary states and general saved
stack histories refuse.

Randomness with `VIEW` or `SYMMETRY` refuses before binding, even in unused
definitions, untaken branches or configuration-selected operators. The one
random Init supported is a single variable with `x = RandomElement(S)`, where
`S` is a nonempty interval or a stored finite scalar set of at most 65,536
entries. This checks that the witness is in the domain, not that a seed
produced it: for `RandomElement(1..1000000)` with `Inv == x<0`, both 730968 and
777 are valid initial witnesses, 1000001 is outside Init, and an in-domain
value whose claimed ending is true is invalid.  Compound or lazy sets, empty, non-set or infinite domains, multiple
variables, conditional or wrapped draws, random Next, `VIEW` or properties,
and startup random definitions refuse. A function-valued non-set domain keeps
EC2176.

## Registers and startup

Integer and named registers work during constant binding and `ASSUME`s. If
Init reads registers, the validator builds a single candidate in source order
from conjunctions, definitions and `INSTANCE` expansion, variable equalities on
constants or already set variables, and Boolean filters, then compares it with
the first state. Disjunctions, quantified or membership assignments and
unresolved reads refuse. Any register access after Init refuses, including in
initial invariants and constraints, because other states may have changed the
registers. Validation never reads or writes the caller's registers.

```tla
ASSUME TLCSet(1,42)
Init == x=0 /\ TLCGet(1)=42
Next == x'=x+1
```

A behavior starting at `x=0` validates against this. `Print` and `PrintT` keep
their errors and return values during replay, but their output is discarded.
Wall-clock and host I/O, exit or pause, live counters and system properties
refuse when executed. `TLCGet("config")` reads the supplied seed and
fingerprint, and is available after constant binding and before `ASSUME`s.

## Language notes

- Init predicates are replayed as generators, so empty product and record
  branches produce no states. Replay stops at a matching emission; exhausting
  Init without one invalidates the trace.
- Lazy arguments and functions use the evaluator's caching, as TLC's
  `LazyValue` does. An `ENABLED` query can use `SelectSeq` callbacks that read
  primed data through an `INSTANCE`, keeping the hidden-variable cutoff when
  inner and outer variables share a name. Effectful callbacks refuse.
- Before entering an `INSTANCE ... WITH` substitution, the validator refuses a
  binding from another model or one not marked pure.
- Action composition (`A \cdot B`) validates in `tla` mode when both operands
  are pure and finite, every first-stage state is complete and each
  intermediate has at most one complete tail successor. `(x'=1) \cdot (x'=2)`
  accepts the edge from `x=0` to `x=2` and rejects `x=9`. Fairness compares
  the original and final states, so `(x'=1) \cdot (x'=0)` from `x=0` has a
  successor but no nonstuttering one, and weak and strong fairness hold on the
  `x=0` stuttering word. TLC's experimental implementation differs
  ([D-022](../tlc-differences.md#d-022-action-composition-works-by-default-with-its-tla-meaning)).
  Modes `off` and `tlc`, nested composition, partial rows, several tail
  successors, effects and preassigned variables refuse. The runnable sources
  and TLC facts are in `crates/dive-trace/tests/fixtures/composition/`; run
  `cargo test -p dive-trace --test composition --locked`.

## Rust APIs

Validate your own observed state sequence:

```rust
use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use serde_json::json;

let source = "---- MODULE Steps ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' = x + 1\n====\n";
let model = SpecModel::from_source("Steps.tla", source,
    "INIT Init\nNEXT Next\n", &TraceOptions::default())?;
let trace = TraceDoc::behavior(vec![json!({"x": 0}), json!({"x": 1})])?;
let result = model.validate(&trace);
assert_eq!(result.validation, Validation::Valid);
```

Call it from a function that returns `Result`, with `serde_json` as a
dependency. `Behavior` checks the supplied finite execution, not an invariant
counterexample or an exhaustive proof.

- For error endings, use `SpecModel::try_validate` or `try_read`, or the
  fallible or controlled `dive_trace::Session` entry points (`Session::try_read`,
  `read_controlled`). Resource, interruption and execution failures return no
  validation report. `dive validate-trace` uses the fallible path: an aborted
  run reports `validation_completed: false` and an `error` with phase and
  progress but no `result`, and exits 3, 5 or 4 for a resource, interruption or
  internal failure.
- `dive_trace::validate(&model, &trace)` and
  `validate_doc(&model, &document, &model_values)` take no random replay
  token, and raw bound-model validation refuses every executed draw and
  register access.
- `dive_trace::validate_controlled` takes a `dive_eval::HostControl` and returns
  `Result<Report, ValidationRunError>`. An interrupted check returns a typed
  interruption with its progress, never a result. The scope restores the
  caller's host on return or unwind and refuses a registration from another
  thread. It adds no resource policy to the legacy functions; binding,
  `ASSUME`, value operations and document encoding are not interruptible.
  Keep the `dive_eval::RECOMMENDED_STACK` thread size.

### Negative controls

`SpecModel::controls` and `SpecModel::mutants` build altered traces that must
be invalid, and require a known shortest BFS counterexample. For simulation,
DFID, multi-worker or liveness words, use `controls_with_mode` or
`mutants_with_mode` with `ControlMode::Counterexample`, which keeps
changed-value, truncated-claim, changed-claim and false-label controls and
drops the distance-based ones, since a shorter or reordered word may also be
valid. `ControlMode::ShortestBfs` adds drop-state, drop-first and swap
controls. Every mutant must be invalid both directly and after a document
round trip; a valid or unsupported mutant is a control failure, and an empty
suite counts for nothing. Random redraw mutations are not available.

In the `mode-controls` fixture, simulation visits `0, 1, 4, 2, 3` while BFS
visits `0, 2, 3`, so dropping `4` leaves another valid counterexample and is not
a negative control for that simulation. From a source checkout:

```sh
cargo test -p dive-api --test trace_control_modes --locked
```

See the `dive_api::trace` API documentation and the
[temporal contract](../temporal-api.md).
