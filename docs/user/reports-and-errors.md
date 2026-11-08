# Reports, errors and automation

## JSON report contract

Request `--output json` to receive one report object. Command reports use
`schema_version: 3`, also exposed as `dive_api::REPORT_SCHEMA_VERSION`.
The native trace nested inside a counterexample has its own version; do not
interpret it as a report version.

For `check`, inspect `verdict` and `exit_code` first. `statistics`, `tlc` and
`counterexample` are nullable: missing checking work or a refusal does not
establish a result. A `stopped` report records an incomplete search and its
budget; it is never a successful verification. Liveness reports include the
complete independently validated `properties` list for the reported word.

The workspace gate pins complete Core JSON examples for success, invariant
violation, deadlock, liveness violation, evaluation error, named refusal and
budget stop. Tests normalize only fixture paths, elapsed timings and the host's
memory metric; semantic counts, names, error codes and trace content remain
checked. See the [fixture guide](../../crates/dive-cli/tests/fixtures/report-schema/README.md)
and the [compatibility policy](#report-compatibility).

`core.metrics.store.memory.construction_history_view_bytes` is deprecated and
always 0: Core no longer keeps construction history (issue #791). Schema 3
keeps the field; a later schema version will remove it.

A whole-function failure inside a pure initial invariant can report
`TLC_FINGERPRINT_EXCEPTION` (2147) after diagnostic replay constructs the actual
source-bearing function receiver. The underlying error remains unchanged.
An unsourced value-method failure does not inherit verified classification from
its underlying module error; unproved receiver lifetimes remain unverified.

Source-qualified tuple/record comparison failures in the first initial
invariant can use the same deciding class after actual Init and diagnostic
replay. TLC records an initial diagnostic state without a formal counterexample
for these controls. The report therefore keeps `counterexample: null` and
provides optional `diagnostic_witness: {"trace": ...}`. Extract
`diagnostic_witness.trace` as the existing native version-2 named invariant-error
word containing the original cause, not the deciding wrapper.
This field is separate from the partial-Init `error_witness`.

For this newly admitted initial classification, automation must require both
independent checks: the named cause word and fresh first-Init/invariant
diagnostic execution proving the receiver's source and lifetime. A valid word
alone, a producer verification flag or matching metadata cannot establish the
2147/2102 relationship. Missing or unproved diagnostic evidence remains a
refusal, including when TLC has no formal trace. VIEW, symmetry, constraints,
effects, earlier invariants and unproved state history remain outside this
bounded route (ADR 0133). Report schema 3 and existing native payload versions
retain their meanings.

A division-by-zero Next error can retain a configured pure state recurrence
when independent replay proves the error occurs before temporal analysis. The
supported two-source case reconstructs one initial state and one new successor,
checks that each recurrence is total and true on the completed first row, then
repeats the actual primitive error and diagnostic at the next source. It uses
the existing version-1 `next-ordered-terminal` endpoint with reported/deciding
2179 and exit 255; it proves a finite error, not a temporal property. VIEW,
symmetry, extra safety roles, effects, other row shapes and unproved exception
lifetimes retain named refusals. For example:

```sh
dive check conformance/corpus/next-error-temporal/specifications/GenError/GenError.tla --config conformance/corpus/next-error-temporal/specifications/GenError/Record.cfg --compat=tlc --workers 1 --seed 0 --fp 0 --output json
```

A pure Next-side whole-function error before any successor can also establish
2147 after actual fresh Init, ordinary Next and diagnostic Next execution. This
bounded route requires one exhausted initial emission and the first source row;
the primary and diagnostic errors must have the same original cause, constructor
and source location, with a newly constructed diagnostic function receiver. Its
formal `counterexample.trace` retains the single original source row and uses
the existing version-1 `next-ordered-terminal` endpoint. The underlying `ec` and
reported code remain unchanged; the deciding wrapper is 2147 and physical exit
75. No initial diagnostic or partial-state witness is added. For example:

```sh
dive check crates/dive-api/tests/fixtures/next-receiver-lifetime/EmptyLazyQuery.tla --compat=tlc --workers 1 --seed 0 --fp 0 --output json
```

A source-bound consumer must independently validate this endpoint before
admitting the wrapper; a producer flag or an underlying-error word alone does
not suffice. Effects, VIEW, symmetry, temporal obligations, earlier candidates,
multiple initial emissions and later source lifetimes remain unverified/refused
until their complete diagnostic histories are proved. In those cases the actual
original error, output and stopping counts remain available. Both engines and
both semantic profiles use this proof; report schema and trace versions retain
their meanings (#680).

An initial invariant can fail with a real underlying error while its printed
initial-state context and nested-expression stack do not establish a deciding
TLC class. These reports keep `tlc.verified: false`, the observed wrapper 2103,
reported context 2102, physical exit 75 and actual stopping counts. They carry
`counterexample: null` and no `diagnostic_witness` or partial-Init `error_witness`.
The original cause and recorded initial state remain in the message; a note
explains that no formal behavior word or deciding-class proof exists. Automation
must treat the unverified classification as a refusal, even when its numbers
match another report. For example:

```sh
dive check crates/dive-api/tests/fixtures/comparison-receiver-unclassified/InitialTool.tla --compat=tlc --workers 1 --seed 19 --fp 0 --output json
```

The condition `IF 1 THEN TRUE ELSE FALSE` raises the original GENERAL error.
Its pinned oracle stream remains Unclassified; no deciding golden is inferred
from the context-only 2103 stack. Classified initial module exceptions and the
source-proved 2147 receiver route above keep their existing contracts (#685).

A supported pure VIEW error while expanding an independently admitted source uses
native trace version 6. Its `view-source-terminal` endpoint retains the original
source span/instance, underlying `ec`, and untrusted reported/deciding codes.
The final complete row is the source: the failing generated candidate is absent.
For the existing `ErViewChoose` example the source word is `(0,0), (1,0)`;
VIEW fails on independently generated `(2,0)`. The independent reader requires the actual run fingerprint polynomial (retain
`--seed`/`--fp` metadata) and reconstructs one exhausted Init, earlier admitted
source expansions, actual key insertions and the terminal primary/diagnostic
sequence. Report schema 3 remains unchanged.

```sh
dive check crates/dive-api/tests/fixtures/checker/errors/specs/ErViewChoose/ErViewChoose.tla --compat=tlc --workers 1 --seed 0 --fp 0 --output json
```

This reports the original underlying error, deciding2103/physical75 and two
complete source rows; use the whole `counterexample.trace` for validation.

Consumers must accept native version 6 and validate this endpoint with a fresh
source-bound fallible session before advancing their producer pin. An old reader
must reject it, without relabelling the document as version 1 or the final source
as a version-4 candidate. Unknown stages/versions still fail closed. Advance
[notice to known consumers](https://github.com/Chelis-Lang/oyster/issues/61#issuecomment-6558501056)
was posted before the version-6 writer landed in #712.

VIEW proof supports scalar states and scalar/tuple VIEW keys only (64 key
nodes, tuple depth eight), at most 64 admitted keys and 256 emissions.
Additional VIEW Init emissions, mutable effects, symmetry, ALIAS, temporal roles
and unproved diagnostic receiver histories explicitly refuse. Earlier admitted
sources are reconstructed only within those same finite bounds. A compound
concrete state is refused before a VIEW that ignores it can certify a failure;
ignored state fields never waive required normalization demand.

Named invariant certification additionally reconstructs multiple Init emissions
and bounded materialized record state fields. An earlier source with no
successors and enabled deadlock checking stops admission of a later error word.
Actual Print/PrintT argument,
rendering and value semantics can execute with the output sink suppressed; this
proves no output transcript. Randomness, register/I/O operations and shared lazy
effects remain refused. A pure integer-equality property `[]<>(v = k)` can
preserve an invariant safety error at the first actually admitted initial source
before any temporal row completes. The fresh Init admission ledger determines
that source, independently of the submitted word's first state.
Other temporal lifecycles, including the unconfirmed temporal generator errors,
retain their existing refusals.

`Session::invariant_error_class` and `SpecModel::invariant_error_class` separately
prove the diagnostic identity of an existing version-2 named invariant-error
word using the same private prefix. Their returned certificate is execution
state, never serialized proof. Ordinary word validation remains insufficient.
Every supported successor-invariant class, including 2111, requires that fresh
certificate before producer reporting, source-bound conformance mapping or
independent report-word admission. Source-free mapping refuses it. Diagnostic
completion or a quiet predicate/partial-state stop can retain 2111; an actual
later exception decides its own class. Correct two-, three- and four-state
words, including primary value-method failures with completed diagnostics,
retain their existing version-2 meaning.
`ErInvConstrained` and `ErInvReplayError` retain their actual named causes and
canonical Unclassified oracle streams. Independently valid words and proved
native diagnostic identities supply no pinned-TLC agreement by themselves.

Coordinating a forged deciding class/name, report exit and physical exit no
longer authorizes a 2111 report (#713). The correct original named word remains
logically valid, but a separate fresh certificate must prove its actual deciding
identity. Canonical TLC class/status and observed physical status are checked
against that proof and the caller's selected exit convention. The conformance
reader and validator use TLC statuses independently of semantic profile;
`check_result_with_model_and_exit_convention` and
`validate_counterexample_with_exit_convention` explicitly support native status
13 for these errors. Report options cannot select the convention on the
caller's behalf.

The existing VIEW endpoint additionally handles proved assertion, division,
ENABLED and TLC-overflow failures. For example `ErViewDivZero` retains deciding
2179/TLC status 255 and independently reconstructs its source-final word.
Matching primary codes or a shared status alone cannot certify the diagnostic
execution. Unknown canonical oracle streams remain unknown.

Returned non-Boolean constraint/action-constraint values use the existing
version-4 endpoint after the actual source-qualified Boolean-role boundary
raises `TLC_EXPECTED_VALUE`. An inner non-Boolean expression is a different
operation; the reader independently discovers which error actually occurs.

Request `--output json` and retain the whole document, source/configuration,
resolved options and replay seed. Every public operation now emits report
`schema_version: 3`, with `command` identifying the operation. `check` also emits
`check_kind: "safety" | "temporal"`. The kind identifies the request/report
family, not execution or proof: final/short safety stops remain temporal-family
reports with their actual `capture_completed: false` evidence. Ordinary safety
reports carry no temporal capture or exhaustive-temporal claim.
Use its result fields; do not interpret all commands through safety-check keys.

For a liveness violation, `property` is the display form of the complete
`properties` list: `P`, `P and Q`, or `P, Q, and R`. The human headline and Rust
word's `properties` name that same list, independently established on the actual
reported word. Another valid counterexample, including TLC's, can have a different
false-property set. Names are never copied from another behavior or unioned over
the model. The Rust `target_property` identifies the analysis target, while
`properties` carries complete attribution (ADR 0126).
Nested configuration and diagnostic objects inherit their enclosing operation's
report version; the Rust `ConfigReport::to_json` helper emits such a fragment,
not an independent CLI operation envelope.

| Operation | What to inspect |
| --- | --- |
| Parse/analyze | `accepted`, `diagnostics`; no property check |
| Translate | Schema 3: `result`, `accepted`, `stage`, artifact/replacement, complete-module analysis and effective options |
| Safety check | `verdict`, counts, options, warnings, collision and counterexample |
| Final/short check | Schema 3, complete `properties`, execution and validated closing edge |
| Simulation | `result`, `exhaustive: false`, `model_proved: false`, observed/pending validation |
| DFID | `termination`, `model_proved`, iterations and validated counterexample |
| Trace validation | `result`; valid/invalid/unsupported is separate from search verdicts |
| Checkpoint pause | `checkpoint.status: paused-incomplete`; incomplete execution |
| Budget stop | `verdict: stopped` and `stopped` (resource, limit, observed, unit, phase, hint); progress statistics; exit 5 |

A false `ASSUME` reports `verdict: "assumption-failure"`, TLC code 2104 and
TLC exit 10. Its `property` is the evaluated expression's source image, such as
`line 4, col 8 to line 4, col 12 of module FalsePlain`, including for named
assumptions. Earlier assumptions retain their configured/source priority. An
error evaluating an assumption retains code 2105 and a null `property`.

```sh
dive check conformance/corpus/assumption-reporting/specifications/Assumptions/FalsePlain.tla --compat=tlc --workers 1 --seed 0 --fp 0 --output json
```

A temporal `evaluation-error` at `error_site: "temporal-state-row"` retains
actual partial statistics and `capture_completed: false`. Its finite trace may
include the independently replayed error endpoint from ADR 0113, with no
violated property or lasso claim. Human reports print the same certified finite
states as JSON; an uncertified diagnostic prefix does not acquire a behavior
claim. Capture statistics say whether capture completed or remained incomplete.
The report family remains temporal.
A qualified `error_site: "temporal-source-row"` reports GENERAL1000/TLC exit 255
after actual source-row execution, diagnostic suppression and fresh independent
replay. It likewise has partial actual statistics, `capture_completed: false`,
a finite source word and no property/lasso. This bounded one-worker path uses
native trace version5; unsupported source order or a different primary/candidate
ending cannot inherit that verified class.
A validation operation aborted by a resource or execution exception emits
`validation_completed: false` and an `error` envelope instead of a completed
`result`; it must not be interpreted as `unsupported`, `valid` or a model verdict.

`unsupported` names a feature that was not checked. A resource limit, timeout,
incomplete bound, failed storage or internal error supplies no successful model
certificate. Missing counters/worker evidence remain absent instead of invented.
A valid finite implementation trace says that trace satisfies the checked
relation; it does not establish every possible behavior of the implementation.

CLI parse/analysis/translation use their own exit conventions. `dive check`
uses TLC's deciding physical status in both semantic profiles: the included
invariant example exits 12 and a cyclic final/short liveness violation exits 13.
Both conventions keep 0 for success, 2 for usage/I/O, 3 for unsupported/resource
limits, 4 for internal failure and 5 for a run stopped by `--time-limit` or
`--max-memory` (ADR 0107). `--exit-codes native` selects the historical
statuses: 1 for rejected input, 10 for property violation, 11 for deadlock,
12 for assumption failure and 13 for evaluation error. `--compat=tlc` also
selects TLC evaluation; it is independent of the chosen exit convention.
DFID and simulation require their report qualifications even on exit zero.

Schema stability for budgets: `options.budget` and the `stopped` block are
retained in `dive check` schema 3 with their earlier meanings. The `stopped`
`verdict` appears only when a budget runs out. Automation that pins
the verdict vocabulary should treat an unknown verdict as "no result".

Stable diagnostic codes and spans belong to the report; display text is intended
for people. Full TLC deciding-error-class coverage is unfinished and tracked in
the status. Generated harness reports in
[conformance](../conformance.md) are development evidence, not the schema for
user commands. Keep profile and schema version in any automation protocol.

Use `dive explain DIVE-L0020` or
`dive explain DIVE-L0020 --output json` for a cause and a next action. This
catalogue covers every assigned parser `DIVE-S` code, temporal `DIVE-L0020`–`DIVE-L0038` codes that are
currently assigned, temporal API `DIVE-A0030`–`DIVE-A0034`, watchdog startup
`DIVE-R0001`, memory observation `DIVE-R0002`, and command adapter
`DIVE-X0001`–`DIVE-X0004`. It does not yet
cover every semantic-analysis diagnostic, or evaluation or storage diagnostics. `dive parse` supplies
a code-specific `help` action when the parser has no more specific suggestion;
messages, severity, real source spans and acceptance are preserved. For example,
`dive explain DIVE-S0012` explains how to resolve overlapping operator precedence
with parentheses. These suggestions require checking the intended expression;
they do not automatically edit the source. An unknown code exits
2; JSON reports `known: false` and `explanation: null`. Explanations have
`command: "explain"` and the common report schema version. They are
informational and contain no model verdict.

Semantic argument-shape explanations cover `DIVE-E0107` (operator call count),
`DIVE-E0110` (function index count), `DIVE-E0112` (an operator argument is
required), `DIVE-E0113` (operator-argument arity), and `DIVE-E0114` (LAMBDA
arity). `dive analyze` adds this guidance only when the producer supplied no
help. Existing suggestions, messages, SANY codes, source spans and acceptance
remain unchanged; the raw Rust `Analysis` retains its original diagnostics.
Function brackets and operator-call parentheses have different meanings. A
tuple-pattern parameter counts as one function index; the diagnostic does not
establish that an arbitrary index belongs to the function's runtime domain.
Other semantic families remain outside this partial explanation catalogue.

Unresolved-reference explanations cover `DIVE-E0100` (an unknown name or
qualified operator) and `DIVE-E0101` (an undefined operator symbol). Check the
intended spelling, declaration or definition, module import and named INSTANCE
qualification. Importing a standard module can resolve its names and operators;
it does not fix an unrelated typo or an argument with the wrong arity. The same
fallback-help rule applies to operator arguments, proof steps and INSTANCE
substitutions: a producer's more specific suggestion always takes precedence.
These report-only explanations do not resolve a name or change which sources
analysis accepts.

Temporal failure diagnostics retain `code` and `message` and add the original
`cause`, a `help` action and the available cause `span`/`location`. A span's
`start` and `end` are UTF-8 byte offsets with an exclusive end; `file` is the
IR source file ID, while `location` names the module and source position.
This temporal span is byte-only, with its file ID inside the span; frontend
diagnostics retain their existing span shape and line/column fields. A formula
construction bound retains its recorded predicate context, which need not be
the exact child where the bound was reached.
Failures in the finite callback source walk retain the innermost inspected valid
expression through aliases, lazy actuals and substituted scopes. Projection and
opaque-storage failures may still have only their enclosing context. This adds
source guidance without evaluating the callback or widening its support.
Stage-wide failures have `span: null` and `location: null`; a root expression
is not substituted for a missing cause location. The particular report's
cause takes precedence over the general catalogue explanation. Budget
exhaustion uses its resource-specific `stopped.hint`. Raising a time or memory
budget does not remove a named algorithmic count limit or implement a missing
feature.

Conformance reports compare the physical numeric exit exactly, even when a
worker-specific comparison omits counts. Native 10 versus TLC 12 is a real
observable difference. The harness can label only an explicitly registered
exact TLC/native pair as `expected-divergence`, conditioned on its declared
verdict, permitted other fields and independent trace validation. It never
counts that result as `agree`; absent, signaled or unrelated exits fail.
See [D-023](../tlc-differences.md#d-023-default-process-exits-are-distinct-conformance-observables)
for the historical accounting correction and current CLI decision. Historical
Rust `CheckReport::exit_code()` remains profile-based; select a convention with
`exit_code_for`.

An exact registered generated-count pair also remains mandatory at multiple
workers, where native comparison omits generated-count differences. The harness
checks both observed count values against the registry; absent or different
values cannot match. At one worker the pair must additionally appear in the
compared differences. These results remain `expected-divergence`.

A safety, action-property, liveness or deadlock verdict needs a whole independently
validated counterexample to earn conformance agreement, including in
worker-specific comparisons that omit witness shape. Missing or metadata-only
words cannot be replaced by a claimed validation result. A selected oracle error
witness also remains required when verdicts match and its payload is unavailable; the metadata
provides no certification and cannot waive an observed word. Legitimate success, assumption, frontend and
evaluation outcomes whose contract carries no witness need none. Numeric exit
pairs cannot excuse absent, invalid, unconfirmed or refused required words.

## Report compatibility

Report compatibility is a versioned contract. Within a major version, existing field
names, types, required presence, null meanings, units and result meanings stay
stable. The same applies to existing native `dive-trace` version 1 fields, tagged
values, property endings and zero-based `ending.back_to` loop targets. The outer
check report's `counterexample.back_to` summary is one-based. The trace payload version
is independent of the report version: `validate-trace` emits report version 3
while reading and validating native trace versions 1 through 6. Version 2 adds the
closed `invariant-error` ending with an exact configured name and underlying error
cause (ADR 0118). Writers retain version 1 for all existing ending kinds; readers
retain legacy version-1 support and reject a version-1 document using the new
ending. Consumers update their readers before advancing an immutable producer
pin. The outer report schema remains 3.

Version 3 adds the closed partial terminal form (ADR 0123). Complete prefix rows
stay in `states`; assigned/unassigned terminal bindings and generating source
stay in the ending. Logical word length includes that candidate. Version 3 is
reserved for this form, and versions 1/2 cannot carry it. Readers must support
the separate bindings and independent replay obligation before using such words.

Version 4 adds an independently replayed candidate-side terminal error
(ADR 0128). Its last complete row is the generated candidate; the preceding rows
form the source prefix. `kind: "error"` carries the closed endpoint stage
`candidate-ordered-terminal`, site (`constraint`, `action-constraint` or
`implied-action`), original source and numeric `reported`/`deciding` identities.
The validator discovers the actual first primary event and candidate in source
order, then executes diagnostic replay independently. Matching headers alone
supply no proof. Version 4 is reserved for this form; versions 1/2/3 cannot carry
it, and plain errors/earlier endpoint stages keep their meanings. Consumers must
update readers before advancing a producer pin; see the exact wire example and
landing notice requirement in [ADR 0128](../decisions/0128-candidate-terminal-error-replay.md).

The same version-4 endpoint also covers assertions, division by zero (including
inside `ENABLED`) and TLC-mode integer arithmetic overflow at these three sites.
Their deciding codes remain 2132, 2179 and 2178; they do not become nested errors.
Both normal and diagnostic execution must independently reproduce the claimed
identities. The file producer performs this fresh controlled replay before
publishing a verified result. If diagnostic execution differs or remains
unproved, the report returns unsupported (exit 3), omits the formal word/TLC class,
and retains actual search statistics/output plus the observed primary in notes.
A canonical oracle stream with physical 255 can remain unclassified even when
the native word validates: division and overflow share that exit, so a printed
primary cause alone does not identify the diagnostic cause.
The bounded assertion classifier also requires the expected message types and
severities throughout; unexpected frames leave its deciding class unproved.
For example, the existing action-constraint division control runs as:

```sh
dive check crates/dive-api/tests/fixtures/checker/errors/specs/ErActConstrDivZero/ErActConstrDivZero.tla --compat=tlc --workers 1 --seed 0 --fp 0 --output json
```

It reports evaluation-error, deciding code 2179/TLC exit 255 and a version-4 final
candidate word. Default mode has the same division behavior; integer overflow is
specific to TLC integer mode. Set-sizing and unproved value-method exceptions
receive no new constructor or replay authority.

Version 5 reserves the `temporal-source-row` error endpoint (ADR 0132). All
complete `states` form the original source prefix. Its last state is the failing
source, including when Next failed before emitting a complete candidate.
`endpoint` contains only `stage` and the existing four-field `source` descriptor;
top-level `ec` is an untrusted underlying error name. Readers independently replay
unique exhausted Init generation, the first source's Next completion/error, the
local singleton row, ordered action/state demand and the row diagnostic before
accepting GENERAL1000/255. The finite word contains one state; multiple initial
emissions or an earlier unproved row are outside this version's bounded route.
There are no serialized selectors, proof flags or wrapper overrides. Versions1–4 retain their meanings
and cannot carry this endpoint. Known consumers must receive advance notice and
implement the new fresh fallible replay before advancing producer pins; see the
exact wire/reader example in [ADR 0132](../decisions/0132-temporal-source-row-error-replay.md).

A non-Boolean value returned by a named constraint/action/property role remains
unproved by this terminal lane. This differs from an actual source-qualified
error raised inside expression evaluation, such as `[A]_e` rejecting a
non-Boolean inner action: fresh replay can prove that constructor and its
wrapper identities. The distinction uses actual execution and source, never a
fixture name or rendered exception message.

Readers select the operation and supported version before interpreting results.
They may ignore unknown optional fields and preserve unfamiliar diagnostic codes,
resource names or phase descriptions as data. They must reject unknown versions,
check kinds, verdicts, result dispositions, trace tags and endings; none supplies
a successful certificate. New optional fields and diagnostic codes may be added
without a version bump. Existing fields are never removed, repurposed, made
mandatory or changed in type within a version. Adding a new closed outcome also
requires a new version. Human display text, timings and incidental JSON member
order are not stable. Different valid lassos remain allowed by the observable
parity contract; their independent validation requirements stay unchanged.

An incompatible report or native trace change requires a new format version,
a CHANGELOG entry describing migration, and advance notice to known consumers
before it lands. Consumers update their readers before moving an immutable pin.
The schema-2 migration was announced in
[Oyster #61](https://github.com/Chelis-Lang/oyster/issues/61). The conformance
harness retains version-1 safety/parse/analyze input support and legacy version-2
temporal input support; current producers emit version 3 for every operation.
A legacy version-2 check without `check_kind` still undergoes all strict temporal
checks. Relabelling a temporal result as safety cannot bypass those checks.

Report major 3 removes the ambiguous `options.temporal_bounds.atom_cells`.
Read `cell_limits.graph_labels`, `cell_limits.attribution_rows` and
`cell_limits.word_truth` separately. A graph policy of `{"kind":"run-budgets"}`
means no logical graph-cell ceiling; the actual time/memory settings remain in
`options.budget`. `{"kind":"cells","cells":n}` is an explicit finite ceiling,
including zero. Attribution and word-truth defaults remain 8,000,000 cells.
`acceptance_policy` declares actual legacy cells or byte/work buffers. The
existing byte/work fields are null in legacy mode, and actual integers in buffer
mode. These are configured settings, never proof that execution reached them.
No DFID/simulation result disposition changes. Readers must support major 3
before moving a producer pin; [Oyster #70](https://github.com/Chelis-Lang/oyster/issues/70)
records the advance notice. See [ADR 0111](../decisions/0111-graph-label-policy-and-report-v3.md).

For example:

```sh
dive check docs/user/examples/Counter.tla --compat=tlc --seed 0 --fp 0 --workers 1 --output json
```

This reports schema 3, `command: "check"`,
`check_kind: "safety"`, `verdict: "success"`, and five distinct states. It does
not report completed temporal capture. Parse/analysis and translation acceptance,
simulation samples, DFID bounds and trace validation retain their operation's
original proof qualifications. This wire policy does not promise a stable Rust
library API; that interface has its own delivery milestone.


## Memory observations

Check reports retain `resources.memory` and the existing check verdict tags in
schema 3.
The configured ceiling remains in `options.budget.max_memory_bytes`; its
requested source is retained separately from enforcement observations.
`metric` is `rss+swap` on Linux or `rss` on macOS. `samples` and
`observed_max_bytes` describe successful observations, with `null` maximum when
none succeeded; they are not a true process-peak measurement.
`initial_sampled`, `final_sampled` and `unavailable_at` describe the boundaries
actually reached. `status` distinguishes sampled, not-started, disabled and
unavailable observations. Disabled means an explicit unlimited memory setting;
it never means an unknown measurement was replaced by zero.

A known initial excess uses the existing `stopped` outcome and exit 5, phase
`check-startup`, no statistics and `resources.checking_started: false`; binding
and graph capture have not started. Safety envelopes omit `execution`; temporal
envelopes also report `execution.started: false` and `capture_completed: false`.
An unavailable observation or default/auto RAM lookup
fails with `DIVE-R0002` using the existing internal-error/no-verdict transport,
exit 4. It has no invented `stopped.observed` byte count or TLC class.
Terminal excess uses phase/stage `finalization`, retaining actual capture
completion and progress. Stops and measurement failures mark retained statistics
`exhaustive: false`; completed capture alone does not prove the checked claim.
`DIVE-R0001` remains the separate watchdog-thread startup failure. A genuine
earlier error or already validated witness keeps its actual precedence; a later
sampling failure is still reported as unavailable rather than an enforced
ceiling. Optional diagnostic identifiers and resource metadata are additive
under ADR 0108; these resource fields do not change native trace encoding.


For D-005, check reports may add `options.symmetry_liveness` and its source,
and a nullable bound `symmetry_liveness` summary. Legacy schema-3 reports may
omit these fields; readers do not infer an unreduced mode from their absence.
A supplied requested mode has the closed vocabulary `unreduced`, `refuse`, `tlc`;
unknown or contradictory values fail closed. The summary retains the declared
name and actual bound temporal applicability. It is neither an execution claim
nor a proof. Existing verdicts, execution facts and independent counterexample
validation remain required. This symmetry metadata does not change native trace encoding.

Finite simulation and DFID reports also include `engine`, `engine_requested`
and nullable `engine_fallback`. Core never falls back and `--engine auto` was
removed, so `engine_fallback` is always `null`; the field is retained within
report schema 3, and older reports may carry `stage: static` with a reason.
Check reports no longer carry the Auto-only `engine_requested` and
`engine_fallback` fields. Native trace versions 1–6 retain their existing meanings.
