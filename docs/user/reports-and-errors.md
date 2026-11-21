# Reports, errors and automation

## JSON reports

Request `--output json` to receive one report object. Keep the whole document,
the source and configuration, the resolved options and the replay seed. Every
command's report carries `schema_version: 4`, also exposed as
`dive_api::REPORT_SCHEMA_VERSION`, and a `command` field naming the operation.
A counterexample nested inside a report is a `dive-trace` document with its own
version; do not read it as a report version.

`check` also reports `check_kind: "safety" | "temporal"`. The kind names the request and its report family. It does not say what ran or what was proved: a final/short safety stop is still a temporal report, with
`capture_completed: false`. Safety reports carry no temporal capture and make
no temporal claim. Read each command's own result fields rather than
interpreting every command through the safety-check keys.

| Operation | What to inspect |
| --- | --- |
| Parse/analyze | `accepted`, `diagnostics`; no property check |
| Translate | `result`, `accepted`, `stage`, artifact/replacement, complete-module analysis and effective options |
| Safety check | `verdict`, counts, options, warnings, collision and counterexample |
| Final/short check | complete `properties`, execution and validated closing edge |
| Simulation | `result`, `exhaustive: false`, `model_proved: false`, observed/pending validation |
| DFID | `termination`, `model_proved`, iterations and validated counterexample |
| Trace validation | `result`; valid/invalid/unsupported is separate from search verdicts |
| Checkpoint pause | `checkpoint.status: paused-incomplete`; incomplete execution |
| Budget stop | `verdict: stopped` and `stopped` (resource, limit, observed, unit, phase, hint); progress statistics; exit 5 |

For example:

```sh
dive check docs/user/examples/Counter.tla --compat=tlc --seed 0 --fp 0 --workers 1 --output json
```

This reports schema 4, `command: "check"`, `check_kind: "safety"`,
`verdict: "success"` and five distinct states, with no completed temporal
capture.

For `check`, inspect `verdict` and `exit_code` first. `statistics`, `tlc` and
`counterexample` are nullable,
and a null field or a refusal is not a result. A `stopped` report
records an incomplete search and the budget that ended it, never a successful
check. The `stopped` verdict appears
only when a budget runs out, and `options.budget` records the configured
budgets. Automation that pins the verdict vocabulary should treat an unknown
verdict as "no result".

`unsupported` names a feature that was not checked. A resource limit, timeout, incomplete bound, storage failure or internal error
is never a successful result, and counters or worker details dive did not
measure are left out, not invented.
A valid finite implementation trace shows that this trace satisfies the checked
relation; it does not establish every possible behavior of the implementation.
Parse, analysis and translation results, simulation samples, DFID bounds and
trace validation each have their own meaning, described in their guides.

For a liveness violation, `property` is the display form of the complete
`properties` list: `P`, `P and Q`, or `P, Q, and R`. The human headline and the Rust counterexample's `properties` give the same
list, computed on the reported counterexample. Another valid counterexample,
including TLC's, can falsify a different set of properties. In Rust, `target_property` identifies the analysis target and
`properties` carries the complete attribution.

A false `ASSUME` reports `verdict: "assumption-failure"`, TLC code 2104 and
TLC exit 10. Its `property` is the source image of the evaluated expression,
such as `line 4, col 8 to line 4, col 12 of module FalsePlain`, also for named
assumptions. Earlier assumptions keep their configured and source priority. An
error while evaluating an assumption reports code 2105 and a null `property`.

```sh
dive check conformance/corpus/assumption-reporting/specifications/Assumptions/FalsePlain.tla --compat=tlc --workers 1 --seed 0 --fp 0 --output json
```

A validation stopped by a resource limit or an execution exception reports
`validation_completed: false` and an `error` envelope instead of a `result`. It
is neither `unsupported`, `valid` nor a model verdict.

Nested configuration and diagnostic objects take the version of the report that
contains them. The Rust `ConfigReport::to_json` helper emits such a fragment,
not a complete command report.

The test suite pins complete JSON examples for success, invariant
violation, deadlock, liveness violation, evaluation error, named refusal and
budget stop. The tests normalize only fixture paths, elapsed times and the
host's memory metric. See the
[fixture guide](../../crates/dive-cli/tests/fixtures/report-schema/README.md).

Stable diagnostic codes and spans belong to the report; display text is for
people. Not every TLC deciding error class is classified. Record the profile
and schema version in any automation that reads reports.

## Exit codes

Parse, analysis and translation have their own exit conventions. `dive check`
exits with TLC's deciding status in both semantic profiles: the included
invariant example exits 12 and a cyclic final/short liveness violation exits
13. `--exit-codes native` selects dive's own statuses instead. Both conventions
share these codes:

| Exit | Meaning (both conventions) |
| ---: | --- |
| 0 | success |
| 2 | usage or I/O error |
| 3 | unsupported request or resource limit |
| 4 | internal failure |
| 5 | run stopped by `--time-limit` or `--max-memory` |

With `--exit-codes native`:

| Exit | Meaning |
| ---: | --- |
| 1 | rejected input |
| 10 | property violation |
| 11 | deadlock |
| 12 | assumption failure |
| 13 | evaluation error |

`--compat=tlc` selects TLC evaluation and is independent of the exit convention.
DFID and simulation reports must be read for their qualifications even when the
exit is zero. In Rust, `CheckReport::exit_code()` follows the profile; select a
convention explicitly with `exit_code_for`.

## Error counterexamples

When an evaluation error has a TLC class that dive can prove, the report carries
a finite counterexample whose ending names the error. The validator replays
each ending kind below before accepting it: it reruns the specification and
must reproduce the same original error, source location and deciding TLC code.
Flags, matching metadata or a valid trace on their own are not enough. When dive cannot prove the class, the report keeps the error,
output and stopping counts, and is `unsupported` or carries
`tlc.verified: false` instead of a formal word.

These proofs cover pure evaluation. Unless a section says otherwise, the
following refuse by name: VIEW, symmetry, extra safety roles, mutable effects,
earlier unproved candidates, multiple initial emissions and value lifetimes
dive has not proved.

### Initial-state errors

A whole-function failure inside a pure initial invariant can report
`TLC_FINGERPRINT_EXCEPTION` (2147) after a diagnostic replay constructs the
source-bearing function. The underlying error is unchanged. An unsourced
value-method failure does not inherit a verified class from its underlying
module error.

A source-qualified tuple or record comparison failure in the first initial
invariant can use the same deciding class. TLC records an initial diagnostic
state without a formal counterexample here, so the report keeps
`counterexample: null` and adds an optional `diagnostic_witness: {"trace": ...}`.
`diagnostic_witness.trace` is a version-2 named invariant-error word holding the
original cause, not the deciding wrapper. It is separate from the partial-Init
`error_witness`. Automation must require both checks: a valid named-cause word,
and a fresh first-Init and invariant diagnostic run that proves the value's
source and lifetime. Missing or unproved diagnostic evidence is a refusal, also
when TLC has no formal trace.

An initial invariant can also fail with a real error whose printed context does
not establish a deciding TLC class. These reports keep `tlc.verified: false`,
the observed wrapper 2103, reported context 2102, physical exit 75 and the
actual stopping counts. They carry `counterexample: null`, no
`diagnostic_witness` and no `error_witness`; the message keeps the original
cause and the recorded initial state, and a note says that the class is not proved. Treat it as a refusal, even when
its numbers match another report:

```sh
dive check crates/dive-api/tests/fixtures/comparison-receiver-unclassified/InitialTool.tla --compat=tlc --workers 1 --seed 19 --fp 0 --output json
```

Here `IF 1 THEN TRUE ELSE FALSE` raises TLC's GENERAL error, and the pinned TLC
output stays unclassified.

### Errors in Next

A division-by-zero error in Next can keep a configured pure state recurrence
when replay proves the error happens before temporal analysis. The supported
case reconstructs one initial state and one new successor, checks that each
recurrence is total and true on the first row, then repeats the error and its
diagnostic at the next source. The word uses the version-1
`next-ordered-terminal` ending with reported and deciding code 2179 and exit
255. It shows a finite error; it checks no temporal property.

```sh
dive check conformance/corpus/next-error-temporal/specifications/GenError/GenError.tla --config conformance/corpus/next-error-temporal/specifications/GenError/Record.cfg --compat=tlc --workers 1 --seed 0 --fp 0 --output json
```

A pure whole-function error in Next before any successor can also establish
2147, after a fresh Init, an ordinary Next and a diagnostic Next run. It needs
one exhausted initial emission and the first source row, and the primary and
diagnostic errors must share the original cause, constructor and source
location. `counterexample.trace` holds the single source row with the version-1
`next-ordered-terminal` ending. The underlying `ec` and reported code are
unchanged; the deciding wrapper is 2147 and the physical exit 75.

```sh
dive check crates/dive-api/tests/fixtures/next-receiver-lifetime/EmptyLazyQuery.tla --compat=tlc --workers 1 --seed 0 --fp 0 --output json
```

### Named invariant errors

An error while evaluating a named invariant is a version-2 word with an
`invariant-error` ending. Certification can reconstruct several Init emissions
and bounded record-valued state fields. An earlier source with no successors,
under deadlock checking, stops admission of a later error word. `Print` and
`PrintT` arguments are evaluated with output suppressed, which proves nothing
about the output transcript. Randomness, registers, I/O and shared lazy effects
refuse. A pure integer-equality property `[]<>(v = k)` can keep an invariant
error at the first admitted initial source before any temporal row completes;
the fresh Init run, not the submitted word, determines that source.

`Session::invariant_error_class` and `SpecModel::invariant_error_class` prove the
diagnostic identity of a version-2 word. The certificate they return lives in memory and is never serialized; ordinary
trace validation does not produce it. Dive reports a successor-invariant
class, including 2111, only with a fresh certificate, and a conformance mapping
without source refuses it. A
quiet completion or a predicate or partial-state stop can keep 2111; a later
exception decides its own class. A class and exit codes that merely agree with each other are not enough for
2111. The validator checks
the TLC class and physical status against the proof and the caller's exit
convention; `check_result_with_model_and_exit_convention` and
`validate_counterexample_with_exit_convention` accept native status 13 for
these errors. `ErInvConstrained` and `ErInvReplayError` keep their named causes
and unclassified TLC outputs; a valid word supplies no TLC agreement by itself.

### Errors on a generated candidate

An error in a `CONSTRAINT`, `ACTION_CONSTRAINT` or implied action, raised on a
generated candidate state, is a version-4 word. Its last complete row is the
candidate and the earlier rows are the source prefix. The `error` ending carries
stage `candidate-ordered-terminal`, the site (`constraint`, `action-constraint`
or `implied-action`), the original source and the numeric `reported` and
`deciding` codes. The validator finds the first error and candidate in source
order, then runs the diagnostic itself.

The ending covers assertions (2132), division by zero, also inside `ENABLED`
(2179), and TLC-mode integer overflow (2178); none becomes a nested error. If
the diagnostic run differs or cannot be proved, the report is `unsupported`
(exit 3), omits the formal word and TLC class, and keeps the search statistics,
output and the observed error in its notes. A TLC output with physical exit 255
can stay unclassified even when the dive word validates, because division and
overflow share that exit. The assertion classifier also requires the expected
message types and severities.

```sh
dive check crates/dive-api/tests/fixtures/checker/errors/specs/ErActConstrDivZero/ErActConstrDivZero.tla --compat=tlc --workers 1 --seed 0 --fp 0 --output json
```

This reports `evaluation-error`, deciding code 2179, TLC exit 255 and a
version-4 word. The default profile has the same division behavior; integer
overflow exists only in TLC integer mode.

A non-Boolean value returned by a constraint or action constraint uses the same
ending once the Boolean-role check raises `TLC_EXPECTED_VALUE`. A non-Boolean
value returned by a named constraint, action or property role is not proved by
this ending. An error raised inside expression evaluation, such as `[A]_e`
rejecting a non-Boolean inner action, is a different operation that replay can
prove; the validator decides from what runs, never from a fixture name or a rendered
message.

### VIEW errors

A pure VIEW error while expanding an admitted source is a version-6 word. Its
`view-source-terminal` ending keeps the original source span and instance, the
underlying `ec` and the reported and deciding codes. The last complete row is
the source; the failing candidate is not in the word. For `ErViewChoose` the
word is `(0,0), (1,0)`, and VIEW fails on the generated `(2,0)`. The validator
needs the run's fingerprint settings (keep the `--seed` and `--fp` values) and
reconstructs one exhausted Init, the earlier source expansions, the key
insertions and the error and its diagnostic.

```sh
dive check crates/dive-api/tests/fixtures/checker/errors/specs/ErViewChoose/ErViewChoose.tla --compat=tlc --workers 1 --seed 0 --fp 0 --output json
```

This reports the original error, deciding code 2103, physical exit 75 and two
source rows; validate the whole `counterexample.trace`. The same ending covers
proved assertion, division, `ENABLED` and TLC-overflow failures: `ErViewDivZero`
reports deciding 2179 and TLC status 255.

VIEW proofs support scalar states and scalar or tuple VIEW keys (64 key nodes,
tuple depth eight), at most 64 admitted keys and 256 emissions. More VIEW Init
emissions, mutable effects, symmetry, ALIAS, temporal roles and unproved
diagnostic histories refuse. A compound state is refused before a VIEW that
ignores part of it can certify a failure.

### Errors in temporal rows

A temporal `evaluation-error` at `error_site: "temporal-state-row"` keeps the
actual partial statistics and `capture_completed: false`. Its finite trace may
end in an independently replayed error, with no violated property or lasso.
Human output prints the same certified states as JSON. A diagnostic prefix that
is not certified says nothing about the model's behavior.

An error at `error_site: "temporal-source-row"` reports GENERAL (1000) and TLC
exit 255 after the source row runs, the diagnostic is suppressed and the
validator replays it. It also has partial statistics,
`capture_completed: false`, a one-state word and no property or lasso. This
one-worker path writes a version-5 word; a different source order or ending
does not inherit the class.

## Diagnostics and `dive explain`

`dive explain DIVE-L0020` (or with `--output json`) gives a cause and a next
action. The catalog covers every assigned parser `DIVE-S` code, the assigned
temporal codes `DIVE-L0020` to `DIVE-L0038`, temporal API codes `DIVE-A0030` to
`DIVE-A0034`, watchdog start-up `DIVE-R0001`, memory observation `DIVE-R0002`
and command adapter codes `DIVE-X0001` to `DIVE-X0004`. It does not cover every
semantic-analysis, evaluation or storage diagnostic. An unknown code exits 2,
and JSON reports `known: false` and `explanation: null`. Explanations carry
`command: "explain"` and the common report schema version, and contain no model
verdict.

`dive parse` adds a code-specific `help` action when the parser has no more
specific suggestion; messages, severity, source spans and acceptance are
unchanged. `dive explain DIVE-S0012`, for example, explains how to resolve
overlapping operator precedence with parentheses. Suggestions never edit the
source.

`dive analyze` adds the same kind of help for argument shapes and unresolved
references when the analyzer supplied none:

- `DIVE-E0107` (operator call count), `DIVE-E0110` (function index count),
  `DIVE-E0112` (an operator argument is required), `DIVE-E0113`
  (operator-argument arity) and `DIVE-E0114` (LAMBDA arity). Function brackets
  and operator-call parentheses mean different things, and a tuple-pattern
  parameter counts as one function index.
- `DIVE-E0100` (an unknown name or qualified operator) and `DIVE-E0101` (an
  undefined operator symbol). Check the spelling, the declaration or
  definition, the module import and the INSTANCE qualification. Importing a
  standard module resolves its names; it does not fix a typo or a wrong arity.

A more specific suggestion from the analyzer always wins. Messages, SANY codes,
spans and acceptance stay as they were, and the raw Rust `Analysis` keeps its
original diagnostics.

Temporal failure diagnostics keep `code` and `message` and add the original
`cause`, a `help` action and the cause's `span` and `location` when known. A
span's `start` and `end` are UTF-8 byte offsets with an exclusive end, and its
`file` is the IR source file id; `location` names the module and source
position. Front-end diagnostics keep their own span shape with line and column
fields. A formula construction bound keeps its recorded predicate context,
which need not be the exact child where the bound was reached. Failures in the
finite callback source walk keep the innermost valid expression through
aliases, lazy actuals and substituted scopes; projection and opaque-storage
failures may have only their enclosing context. Stage-wide failures have
`span: null` and `location: null`. A report's own cause takes precedence over
the catalog explanation. Budget exhaustion uses its resource-specific
`stopped.hint`; raising a time or memory budget does not lift a named count
limit or add a missing feature.

## Memory observations

Check reports carry `resources.memory`. The configured ceiling is in
`options.budget.max_memory_bytes`, with its source recorded separately from the
observations. `metric` is `rss+swap` on Linux and `rss` on macOS. `samples` and
`observed_max_bytes` describe successful observations, with a `null` maximum
when none succeeded; they are not a true process peak. `initial_sampled`,
`final_sampled` and `unavailable_at` describe the boundaries reached.
`status` is sampled, not-started, disabled or unavailable; disabled means an
explicit unlimited setting, never an unknown measurement replaced by zero.

A known excess at start-up uses the `stopped` outcome, exit 5, phase
`check-startup`, no statistics and `resources.checking_started: false`; binding
and graph capture have not started. Safety reports omit `execution`; temporal
reports add `execution.started: false` and `capture_completed: false`. Start-up work after binding begins includes constant definitions, constant
folding while dive compiles its plan, and the ASSUMEs. A time or memory budget
that runs out there also stops in phase `check-startup`, before `Init`, with no
statistics and whatever start-up output was printed. Refusals that hold at any budget are reported
first. `resources.checking_started` is then omitted, since binding started, and
temporal reports give `execution.stage: check-startup` and
`capture_completed: false` without `started`.

A failed observation, or a failed RAM lookup for the default or `auto` memory
limit, reports `DIVE-R0002` as an internal error with no verdict and exit 4,
with no invented `stopped.observed` value or TLC class. An excess at the end
uses phase `finalization` and keeps the actual capture completion and progress.
Stops and measurement failures mark the retained statistics
`exhaustive: false`; a completed capture alone does not prove the checked
claim. `DIVE-R0001` is the separate watchdog start-up failure. An earlier error or validated witness takes precedence, and a
later sampling failure is reported as unavailable rather than as an enforced
ceiling.

## Optional fields

Check reports may add `options.symmetry_liveness`, its source and a nullable
`symmetry_liveness` summary (see
[symmetry with liveness](../tlc-differences.md#d-005-symmetry-with-liveness-uses-the-full-state-space)).
Earlier reports may omit these fields; do not infer an unreduced mode
from their absence. The requested mode is one of `unreduced`, `refuse` and
`tlc`; unknown or contradictory values fail closed. The summary names the
declared symmetry and whether it applies to the bound temporal properties. It describes the configuration, not what ran.

Simulation and DFID reports include `engine`, `engine_requested` and a nullable
`engine_fallback`. dive is the only engine, so `engine_fallback` is always
`null`. Check reports do not carry `engine_requested` or `engine_fallback`.

## Report compatibility

Reports and counterexample documents are versioned contracts.

| Document | Version | What it contains |
| --- | --- | --- |
| Command report | `schema_version: 4` | Every command. Temporal limits are reported as `cell_limits.graph_labels`, `cell_limits.attribution_rows` and `cell_limits.word_truth`. |
| `dive-trace` | 1 | Ordinary endings: invariant, action-property and liveness violations, deadlock, and the error endings such as `next-ordered-terminal`. |
| `dive-trace` | 2 | The `invariant-error` ending: an error while evaluating a named invariant, with its configured name and underlying cause. |
| `dive-trace` | 3 | A partial terminal candidate: a Next step that leaves variables unassigned. |
| `dive-trace` | 4 | The `candidate-ordered-terminal` error ending on a generated candidate. |
| `dive-trace` | 5 | The `temporal-source-row` error ending. |
| `dive-trace` | 6 | The `view-source-terminal` error ending. |

The trace version is independent of the report version: `validate-trace`
reports schema 4 and reads trace versions 1 to 6. Each trace version is
reserved for its form, and a lower version cannot carry it; a version-1
document with an `invariant-error` ending is rejected. The
[trace guide](traces.md#trace-format-versions) describes each form.

Within a version, existing field names, types, required presence, null
meanings, units and result meanings stay stable. This includes `dive-trace`
fields, tagged values, property endings and the zero-based `ending.back_to`
loop target; the report's `counterexample.back_to` summary is one-based. New
optional fields and diagnostic codes may be added without a version change.
A new closed outcome needs a new version. Human display text,
timings and JSON member order are not stable. Different valid lassos are
allowed, and each still needs independent validation.

Readers select the operation and a supported version before reading results.
They may ignore unknown optional fields and keep unfamiliar diagnostic codes,
resource names or phase descriptions as data. They must reject unknown
versions, check kinds, verdicts, result dispositions, trace tags and endings;
none of these supplies a successful result.

An incompatible report or trace change requires a new version and a
`CHANGELOG.md` entry that describes the migration. Update readers before moving
to a new producer version.

Schema 3 replaced schema 2's single `options.temporal_bounds.atom_cells` limit
with the three `cell_limits` fields. A graph policy of `{"kind":"run-budgets"}`
means no logical graph-cell ceiling, with the time and memory settings in
`options.budget`. `{"kind":"cells","cells":n}` is an explicit finite ceiling,
including zero. The attribution and word-truth defaults are 8,000,000 cells.
`acceptance_policy` declares either the legacy cell limits or byte and work
buffers; the byte and work fields are null in legacy mode and integers in
buffer mode. These are configured settings, not proof that a run reached them.

Schema 4 replaced schema 3 by removing the construction-history metrics,
which were always 0 because dive keeps no construction history:
`core.metrics.store.memory.construction_history_view_bytes` and, under
`core.metrics.candidate_storage`, `origin_index_bytes`,
`peak_origin_index_bytes` and `origin_index_bytes_limit_per_worker`. A schema-3
reader moves to schema 4 by no longer requiring those four fields; it can keep
accepting schema 3 for stored reports.

Schema-4 `simulate` reports add to each trace entry an `alias` array (the
`ALIAS` value of each state, or null when it could not be rendered), and
their trace documents label each step with its action instead of null. A
reader that ignores unknown fields needs no change.

Schema-4 `simulate` reports also keep only some traces: by default the
deciding trace and the 10 lowest-numbered others (`options.keep_traces`), so
`traces` no longer lists every trace started. `statistics.traces_kept` and
`statistics.traces_dropped` count what the report holds and what
`--max-report-bytes` left out; a trace entry may carry a `summary` instead of
its `document`; `options.traces` is null for `--traces unlimited`; and
`options.workers_used` is the worker count. A reader that walked `traces` to
count the run should read `statistics.traces_started` instead.

With a temporal `PROPERTY`, schema-4 `simulate` reports add
`statistics.trace_classes` (`closed`, `terminal`, `truncated`,
`extended_closed`, `extended_terminal`, `violated`, `cancelled`) and
`statistics.properties`, one entry per configured property with `name`,
`part`, `refuted_by_trace`, `checked_traces`, `unchecked_traces`,
`unchecked_reasons` and `behaviors_checked`; `options` adds
`liveness_extend`, `liveness_budget` and `bad_prefix`. A temporal violation
has `observed_violation.kind: "temporal"` and `tlc_code` 2116, and its trace
document ends in `lasso`, `stuttering` or `bad-prefix`. A reader that ignores
unknown fields needs no change.

The JSON wire format is versioned this way; the Rust library API has no such
stability promise.

## Exit codes in comparisons

The exit status is part of the result. Native exit 10 against TLC's 12 is a
real difference unless it is a
[documented exit difference](../tlc-differences.md#d-023-default-process-exits-are-distinct-conformance-observables),
and a matching exit does not make up for a missing, invalid or refused
counterexample.
