# Independent trace validation

Validate a saved checker witness against the same specification and configuration:

```sh
dive check docs/user/examples/Counter.tla --config docs/user/examples/CounterViolation.cfg --compat=tlc --seed 7 --fp 0 --output json > counter-result.json
# The expected invariant violation exits 12; retain the document.
python3 -c 'import json; r=json.load(open("counter-result.json")); json.dump(r["counterexample"]["trace"], open("counter-trace.json", "w"))'
dive validate-trace docs/user/examples/Counter.tla --config docs/user/examples/CounterViolation.cfg --trace counter-trace.json --compat=tlc --seed 7 --fp 0 --output json
```

The JSON validation report uses `schema_version: 3`, `command: "validate-trace"`
and `result: "valid" | "invalid" | "unsupported"`. Its report version is
independent of the native trace document's `format: "dive-trace"` and payload
version (1 for existing endings, 2 for named invariant errors).
Existing trace tags, endings and loop indexing follow the compatibility policy
in [reports and automation](reports-and-errors.md).

A final invariant evaluation error carries `kind: "invariant-error"`, its
configured `name`, and the underlying error's `ec`, for example
`TLC_VALUE_ASSERT_FAILED`. The check report normally uses the deciding class
`TLC_INVARIANT_EVALUATION_FAILED` (2111) and TLC exit 76. These identify different
parts of the same result: the validator must reproduce the claimed cause at the
named invariant, after all earlier states and earlier configured invariants
pass. A false Boolean invariant is not an evaluation error. Wrong names, wrong
causes, an absent error, or an earlier failure invalidate the claim. An
unsupported evaluation remains unsupported. The harness requires this proof
before counting the witness as validated.

A later diagnostic replay can encounter a different error. In the qualified
native invariant-site case with first report 2111 and deciding
`TLC_NESTED_EXPRESSION` (2103), the word still names the original invariant and
its original cause. Independent validation proves that named error on the
finite word. It does not prove the later diagnostic replay's class or counts.
The strict harness still refutes this wrapper/word pair; matching metadata
supplies no replacement proof. Anonymous expansion errors gain no exception,
and unclassified oracle results remain unclassified.

This new closed ending requires native trace version 2 (ADR 0118). Existing
endings still write version 1; readers accept both. A version-1 document carrying
`invariant-error`, an unknown version, or an unknown ending fails closed. Consumer
readers must support version 2 and its named error obligation before updating a
producer pin; they must not relabel the payload as version 1. Example:

```json
{"format":"dive-trace","version":2,"variables":["x"],"states":[{"action":null,"values":{"x":0}},{"action":null,"values":{"x":1}},{"action":null,"values":{"x":2}}],"ending":{"kind":"invariant-error","name":"Inv","ec":"TLC_VALUE_ASSERT_FAILED"}}
```

This support applies to invariant-site errors decided by 2111. Initial-state,
constraint, action-property and successor-expansion error claims retain their
existing checks and refusals; a generic error ending cannot stand in for a named
invariant error. Counterexamples outside constraints retain their existing
validation boundary (#499).

A certified temporal state-row error can carry `kind: "error"` with an optional
`endpoint` describing its stage and original source context (ADR 0113). The
validator reconstructs the full configured demand independently and checks its
first real error after validating the finite path. The descriptor cannot select
an atom or scope. Generic errors without an endpoint remain unsupported; an
endpoint is not a lasso, property violation, BFS-count or completion proof.
Older trace1 readers retain the generic unsupported error ending.

Rust callers must use `SpecModel::try_validate`/`try_read`, or the corresponding
fresh `dive_trace::Session` fallible/controlled entrypoint, for these endpoints.
The legacy infallible entrypoints refuse the request before replay. Resource,
interruption and execution exceptions return no validation report. Standalone
`validate-trace` uses the fallible path: an aborted JSON operation has
`validation_completed: false` and `error` with phase/progress, but no `result`.
Its resource/interruption/internal statuses are 3/5/4. No new budget is installed
by the uncontrolled fallible entrypoint. The completed report vocabulary above
is unchanged.

The validator evaluates Init and the original next-state relation as predicates
over supplied concrete states. It is independent of the checker's successor
enumerator. Every state must assign every spec variable; missing/partial states
are not completed by guessing. Native trace formats normalize value JSON and has known representation
gaps. Successful decoding is separate from semantic validation. TLC dumpTrace
JSON is also an accepted input format within the validator's documented boundary.

Rust applications can validate their own observed state sequence:

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

Use a `Result`-returning surrounding function and add `serde_json` to the caller's
dependencies. `Behavior` checks the supplied finite execution; it does not claim
an invariant counterexample or exhaustive model proof. Counterexample endings
and periodic lassos carry separate claims and closing-edge obligations.

The lower-level `dive_trace` API exposes `validate(&model, &trace)` and
`validate_doc(&model, &document, &model_values)`. Neither takes a random replay
token; a random draw during validation remains explicitly unsupported.

For supplied native `Trace` values, `dive_trace::validate_controlled` accepts a
generic `dive_eval::HostControl` and returns `Result<Report, ValidationRunError>`.
An interrupted Init, Next, label, deadlock/`ENABLED`, or temporal/fairness check
returns a typed interruption with actual completed-check progress; it cannot
become `valid`, `invalid`, or `unsupported`. The owning driver interprets its
opaque identity. Ordinary feature refusals and invalid words keep their existing
reports. The control scope restores the caller's evaluator host and control on
return or unwind, and refuses a registration owned by a different thread.

This API does not add a resource policy to legacy `validate`, `validate_doc`,
TLC interpretation scoring or uncontrolled validation. Their closed report tags
are unchanged. The fresh controlled endpoint path separately retains its real
control through reconstruction, ASSUME, finite checks and ordered replay.
Legacy binding, `ASSUME`,
quiet value work, and pure bulk/native operations retain their cooperative
interruption limitations. Supplied deferred values can retain another
evaluator, whose work this control does not govern; document encoding and
decoding are also not controlled. Callers must retain the documented
`dive_eval::RECOMMENDED_STACK` contract.

`SpecModel::controls` and `SpecModel::mutants` require a known valid shortest
BFS counterexample. For simulation, DFID, multi-worker or liveness words, use
`controls_with_mode` or `mutants_with_mode` with `ControlMode::Counterexample`.
The explicit `ControlMode::ShortestBfs` includes the same drop-state,
drop-first and swap controls as the compatibility methods. Counterexample mode
keeps changed-value, truncated-claim, changed-claim and false-label controls;
it omits the distance-based mutations because a shorter or reordered word may
also be valid. Mode selection does not search for shorter paths.

Every generated mutant must be invalid through both direct validation and its
document round trip. Valid or unsupported mutants are control failures; an
empty negative suite receives no coverage credit. The positive and document
round-trip checks remain the same in both modes and profiles. The harness's
`dive-conformance trace-controls` selects shortest-BFS controls only for
single-worker exhaustive finite counterexamples, using the model mode,
golden worker count and ending. Random redraw mutations remain unavailable
while random validation is unsupported.

The pinned TLC regression fixture in
[`crates/dive-api/tests/fixtures/mode-controls`](../../crates/dive-api/tests/fixtures/mode-controls)
illustrates the distinction: simulation visits `0, 1, 4, 2, 3`, while BFS
visits `0, 2, 3`. Dropping `4` leaves another valid counterexample, so it is
not a negative control for that simulation. The fixture's consuming test runs
both profiles:

```sh
cargo test -p dive-api --test trace_control_modes --locked
```

Validation distinguishes valid, invalid and explicitly refused. Random draws,
search-dependent register access, unavailable finite deadlock/ENABLED covers,
effects and unknown closing evidence do not become `valid`. Supply the original
seed/fp for run-dependent model binding and use the checking profile. The in-memory lasso back edge is zero-based; checker
summary `back_to` display is one-based. Prefer the supplied trace document over reconstructing a lasso from displayed
states, and inspect its decoder and independent-validation disposition.

See the [API rustdoc](../../crates/dive-api/src/trace.rs),
[temporal contract](../temporal-api.md) and [validator independence ADR](../decisions/0016-trace-validator-independence.md).

A known mixed-set limitation remains: a checker can emit a JSON set containing
a sequence before a record that the decoder rejects at the shape check, in
either profile. Such a result receives no validation credit.
[Follow-up #190](https://github.com/Chelis-Lang/dive/issues/190) tracks this boundary.

Under `--compat=tlc`, strings and model values compare in the order TLC first
encountered them. The validator knows that order for every name interned when
the model is bound. If reading the trace or checking it creates any other
name (a string the specification never mentions, or one an operator such as
`ToString` or `Tail` builds on these states), the result is
`unsupported: runtime string encounter order`, never `valid` or `invalid`.
Executed random draws (`RandomElement`, `Randomization`) remain refused.
The private direct initial-membership operation below checks support without
drawing a value.

Before entering an `INSTANCE ... WITH` substitution, whose strict bindings are
evaluated eagerly, the validator refuses a binding from another model's
context or one whose effects annotation is not pure. It trusts the IR's
effects annotations; it does not re-prove them.

Whether a state has a successor is decided by the core evaluator's `ENABLED`,
the same search TLC's `Tool.enabled` performs and the checker uses
([ADR 0103](../decisions/0103-validator-enabled-via-core-evaluator.md)). This
covers `ENABLED` inside any predicate the validator evaluates, deadlock endings
(`ENABLED Next` on the last state) and weak or strong fairness on a lasso. The
validator still replays and checks every state and step of the counterexample
itself; an `ENABLED` whose action has side effects, or that the evaluator cannot
decide, makes the result `unsupported`, never a decision.

### Initial-predicate level

Init runs at startup level 0; generated-state invariants and implied initial properties run at level 1. The independent validator uses a distinct initial context and preserves ordinary state/action contexts thereafter. Random draws and register access after Init remain refused.

Action composition (`A \cdot B`) currently has no independent intermediate-state
validator. A reached composition therefore yields `unsupported`, including
when the workspace enables the checker enumerator. Untaken composition branches
do not run. The lasso API conservatively refuses composition in its predicate
contexts, and `dive_trace::enabled::enabled` (with `ENABLED`, `WF` and `SF` in
the lasso API) refuses a search that reaches a composition with the same named
feature. Checker composition support does not establish trace-validation
agreement.

Replay evaluates lazy arguments and functions with the evaluator's ordinary
caching, as TLC's `LazyValue` does.


### Private startup registers

`SpecModel::validate`, `read`, `validate_trace` and the CLI reconstruct binding
and startup from the original source/configuration for every validation. Each
TLC JSON interpretation has its own interner, evaluator, registers and caches.
Changing the caller's evaluator registers cannot supply an initial value or
change a validation, and validation does not write the caller's register file.

Randomness combined with `VIEW` or `SYMMETRY` refuses before ordinary API
binding or the first private startup attempt. This shares the checker's
source/config policy, including unused definitions, untaken branches and
configuration-selected random operators. The refusal does not establish
purity when it is absent and does not grant a replay or host capability.

Integer and named registers are supported during real constant binding and
ASSUMEs. If Init accesses registers, the validator determines a single candidate
from empty variable slots, in source order, then compares it with the first
trace state. Supported relations are ordered conjunctions, definition and
INSTANCE expansion, direct variable equalities whose right sides use constants
or already determined variables, and Boolean filters on determined variables.
Disjunctions, quantified assignments, membership assignments and unresolved
variable reads remain explicitly unsupported for this operation. When startup
has not accessed a register, ordinary conservative predicate membership remains
available in a new runtime that refuses all register operations. It validates
only the supplied word and makes no claim that unselected Init candidates lack
writers. A witnessed writer or any later register access remains refused. Every
register access after Init, including initial invariants, constraints, optional
labels and temporal checks, remains refused: other explored states may have
changed those registers. The raw `dive_trace::validate(&Model, ...)` API retains
its conservative register refusal.

For example, this startup pattern validates a behavior starting with `x=0`:

```tla
ASSUME TLCSet(1,42)
Init == x=0 /\ TLCGet(1)=42
Next == x'=x+1
```

Use `SpecModel::from_source` with `INIT Init` and `NEXT Next`, then validate
`TraceDoc::behavior(vec![json!({"x":0}), json!({"x":1})])`, as in the Rust
example above. Random draws remain unsupported even in unused constant
start-up definitions that actually execute. Print/PrintT preserve actual argument
and rendering errors and return values during private startup and word validation;
rendered replay output is discarded before any sink callback. Producer binding
and runtime output remain separate from this quiet reconstruction. This grants
no additional random/register/I/O access. Wall-clock/host I/O, exit/pause, live
counters and system properties have no private-startup validation contract and
remain named refusals when executed. Supply the run
seed/fp when reading `TLCGet("config")`; the configuration becomes available
after constant binding and before ASSUMEs, matching ordinary API binding.
This validates the displayed behavior, with no claim of phase or corpus parity.


### Direct random initial membership

Private sessions support exactly one variable and a complete Init consisting
of `x = RandomElement(S)`. Zero-argument definition expansion and labels may
expose that equality; surrounding Boolean relations, parameterized wrappers,
native aliases/redirects and INSTANCE relations have no new random authority.
The shared evaluator evaluates the actual `S` from empty initial slots with
registers and host effects refused. It accepts nonempty intervals and stored
finite scalar sets, using the profile's exact value membership. Stored sets
have a 65,536-entry limit before normalization; compound and lazy set
representations remain unsupported. A pure expression that actually evaluates
to a supported stored set is allowed. Empty/non-set/infinite domains and domain
evaluation errors retain their native error identity in an unsupported report;
they are not reported as an ordinary out-of-domain witness.

This checks witness validity, not reproduction of a seeded producer sample.
For P9's `RandomElement(1..1000000)` and `Inv == x<0`, both 730968 and 777 are
valid initial invariant witnesses; 1000001 is outside Init. An in-domain value
whose claimed ending is true is invalid. The steps, earlier invariants and
claimed ending still pass the normal independent checks. Checker seeds, draws,
counts and goldens are unchanged. Explicit configuration reads in S still use
the supplied run metadata to determine the actual domain. No random host
callback receives the witness,
and the validator never searches for a seed or stream offset.

Randomness with VIEW or SYMMETRY still refuses before binding, including
unused, untaken and config-selected random references. Absence of that refusal
grants no purity or host capability.

Startup register effects cannot be combined with this relation. Executed
startup random definitions (including failed/unused definitions), cached or
coupled draws, multiple variables, conditional/discarded/wrapped draws, random
Next/VIEW/properties and other random operations retain sticky refusal. The
ordinary conservative word-membership fallback retains its existing treatment
of untaken branches; it grants no random or register access and proves no
absence of unselected effects. Raw bound-Model validation still refuses every
executed draw. Every interpretation and repeated validation gets a fresh Core,
interner and caches; caller registers, cached samples and stream position are
not authority. This narrow operation makes no full-gate or corpus parity claim.

Candidates and stored entries are scalar. Strings/model values require tokens
that resolve to their exact text in the private interner. Assigned public states,
compound candidates, compound stored entries and Deferred domains (including
cached Deferred sets) refuse before normalization/member callbacks. The public
helper takes Const or empty Init with the exact slot count and no successor;
it grants no authority to ordinary random evaluation. Function-valued non-set
domains retain EC2176 with a static diagnostic, avoiding nested-value formatting.

A reached `ENABLED` query can use `SelectSeq` callbacks that read primed data
through an `INSTANCE`; replay evaluates them with the core evaluator, which keeps
the hidden-variable cutoff when inner and outer variables have the same name.
Effectful callbacks remain refused. This
query support does not admit action-valued callbacks as ordinary state helpers.
