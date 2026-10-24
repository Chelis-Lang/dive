# Safety checking

```sh
dive check docs/user/examples/Counter.tla --seed 7 --workers auto --output json
dive check docs/user/examples/Counter.tla --compat=tlc --seed 7 --fp 0 --workers 8 --store exact --output json
```

Invariant expressions that use `ENABLED` through an `INSTANCE` retain the
query context inside native callbacks such as `SelectSeq`. A callback that reads
a primed variable observes the query's candidate assignment, including when an
inner variable shares a name with an outer variable. This applies in both
semantic profiles.

Bounded temporal callback analysis can admit native sequence concatenation
(`\o`) when an owned source or stored value proves that at least one operand
cannot successfully produce a string. The proof follows lexical aliases and
requires every result arm of an `IF` or `CASE` to have that property. It does
not evaluate an operand, force a cached thunk or inspect state invariants.
Strings and unknown result kinds retain the runtime-name refusal.

This capability removes only Concat's own string-name creation concern. Both
operands and all retained captures, including unused actuals, still undergo the
existing effect, provider, context ownership and resource checks. Configured or
redirected native origins and higher-order native operator actuals retain their
existing restrictions. Ordinary evaluation preserves eager argument order and
typed errors. The three Concat temporal-atom error controls still produce
`DIVE-L0033` refusals; issue #451 tracks their ordinary evaluation-error
outcomes. VIEW graph capture
still requires quotient-edge and closing-state evidence (issue #428), so
EWD998ChanID remains unsupported. Other temporal restrictions also remain.

## TLC checking entrypoints

The checking-only adapter accepts either `dive tlc [OPTIONS] SPEC[.tla]` or the
installed `tlc [OPTIONS] SPEC[.tla]`. An executable whose basename is `tlc` or
`tlc.exe`, including a renamed binary or symlink to `dive`, selects the same
adapter. `cargo run -p dive-cli` still selects `dive`; both binaries share one
command implementation.

```sh
dive tlc docs/user/examples/Counter -seed 7 -fp 0 --output json
tlc docs/user/examples/Counter.tla -seed 7 -fp 0 -workers 1
```

The adapter selects TLC evaluation and TLC verdict statuses without requiring
`--compat=tlc`, and requests one worker unless `-workers N|auto` changes it.
`DIVE_COMPAT` and the native hidden chaos environment option do not change these
defaults. It accepts `-modelcheck`, `-config FILE[.cfg]`, `-seed N`, `-fp N`,
`-workers N|auto`, `-deadlock`, and `-h`/`-help`. Double-dash checking aliases and
`--output human|json` also work, including inline double-dash values.

Missing `.tla` and `.cfg` suffixes are appended. With an absolute module path,
an explicit relative `-config` is resolved from that module's directory. With a
relative module path, an explicit relative `-config` is resolved from the current
directory. The default configuration is always the module's sibling `.cfg`.
Absolute configuration paths remain absolute. There is no existence-based
fallback between these directories. Native file APIs keep their explicit path
semantics. Configurations embedded in `.tla` files retain their named refusal.

`-lncheck final` directly selects the existing bounded final checker and its
deterministic short lasso. It does not require a second policy flag. Reports
retain `tlc+overrides`, actual bounds, startup and planning errors, incomplete
capture and independent periodic-word validation. This selection need not match
TLC's arbitrary lasso choice. `-lncheck default` retains ordinary checking.
With profile-default resident-memory settings and `-fp 0`, eligible pure temporal
obligations use the same bounded final analyzer, reported as profile `tlc` with
actual final/short metadata and independent word validation. Other TLC-profile
temporal combinations retain named refusals; see [ordinary TLC-profile temporal
checking](temporal.md#ordinary-checking-in-the-tlc-profile) for exact eligibility.
Other strategies refuse by name. Native
`dive check --lncheck final --lasso short` keeps its existing explicit contract.

Every counterexample emitted by this adapter is independently replayed from
fresh source/configuration inputs. JSON records `counterexample.validation` as
`valid` only after replay succeeds. Unsupported replay returns status 3 and
invalid replay returns 4; both suppress the word and its violation claim.
Startup output and observed statistics may remain in the JSON failure report,
and are not proof of a completed check. Existing validator limits remain active.

Human output and versioned JSON use the existing check report. `-tool` message
framing is not implemented here. TLC simulation/generation, DFID, checkpoint,
storage, debugger, coverage and dump flags refuse by name rather than selecting
native modes with different contracts. Unknown or malformed arguments exit 1;
recognized unsupported options exit 3 before input access. Input I/O errors go
to stderr and use 150 for the module and 151 for the configuration. The adapter
reads and parses the configuration first, as TLC does: a missing or malformed
configuration exits 151 even when the module is also missing.
`dive_api::check_file_tlc_inputs` provides the same input order for Rust callers;
filenames must already include their extensions.
Native commands retain their existing statuses and input ordering. These
entrypoints implement a checking subset; full TLC toolchain parity remains open.

## Time and memory budgets

Every `dive check` runs under two budgets (ADR 0107):

- `--time-limit DURATION|none` bounds wall-clock time for the whole check.
  Durations are seconds by default, or take a unit: `500ms`, `90s`, `10m`,
  `2h`. The default profile defaults to 30 minutes, a provisional value until
  release; `--compat=tlc` has no time limit, as TLC has none.
- `--max-memory BYTES|auto|none` guards sampled memory of the whole process. Sizes take `-Xmx` units: `512m`, `8g`. `auto`, the default in both
  profiles, is half of physical memory.

The clock starts after the specification and configuration are loaded.
dive polls the budget for each initial state, each expanded state, and during
graph capture and temporal analysis. After startup, nested quantifier, CHOOSE
and filter predicate loops also poll at amortized work checkpoints, including
when no complete successor is emitted (issue #374). Successor domain candidates
poll before binding, so a rejecting continuation cannot bypass control. Action
composition also polls while collecting those candidates and before each
intermediate right step in next-state generation, action-predicate evaluation
and `ENABLED` (issue #393). These checks retain the original mode-specific
enumeration and counts. An interrupted private intermediate collection does
not become reported generated states; progress describes the externally
published prefix.

For example, this rejects candidates without producing successors and stops
with exit 5 under the explicit time budget:

```sh
dive check docs/user/examples/RejectedCandidates.tla --seed 0 --fp 0 --workers 4 --time-limit 100ms --max-memory 1g --output json
```

Fresh model binding installs the same run control after structural configuration
validation and real evaluator construction. Existing evaluator loop checkpoints
then cover constant startup evaluation, evaluator-backed role binding and action
expansion. An interrupted startup definition is returned as typed control flow;
it is never swallowed as a TLC evaluation error or relabelled `unsupported`.
A binding stop reports `model-binding`, retains output already emitted, and
reports no search statistics or used workers. The additive Rust
`dive_model::bind_controlled`, `bind_with_names_controlled` and
`Recipe::bind_controlled` entry points expose this capability separately from
`BindOptions`; their uncontrolled counterparts retain their original behavior.

Producer `ASSUME` evaluation in safety checking and graph capture uses the
armed startup control at the same amortized evaluator-loop checkpoints.
Interruption reports `stopped` in `check-startup`, retaining preceding output
without search statistics, a behavior graph or a model verdict. Genuine false
assumptions and evaluation errors retain their existing classifications.

Structural binding and evaluator construction,
post-binding symmetry preparation, value-only allocation/sorting/native
operations, normalization in existing quiet equality/ordering/hash units, and
diagnostic error/count and predecessor-path reconstruction after an already
deciding event remain outside this control (#349, #379). The diagnostic exclusion
affects only the reconstructing thread; other workers retain control. It does
not suspend ordinary checking.

Fresh independent replay of final and prefix native temporal candidates inherits
the same armed control, including evaluator-backed rebinding. Both supplied and
serialized native words must validate before a violation is certified.
Interruption there reports `stopped` at `trace-validation`, with real capture
evidence and optional validation progress, without an uncertified word. A stop
while rebinding has no passed validation checks or supplied state position.
Rebinding output is retained as validation evidence and is not printed again as
original execution output. General TLC/session interpretation and standalone
`validate-trace` budgets remain follow-ups. Independent validator `ASSUME`,
serialization, and supplied deferred providers retained by another evaluator
remain outside this control. Diagnostic suppression never pauses candidate
certification. No universal wall-clock deadline is promised.
Unavailable memory observations interrupt these loops with typed `DIVE-R0002`,
preserving that failure rather than inventing an exhausted resource.
Quiet value units defer calling the host control until they return: an
infallible value operation cannot substitute a transient interruption for truth
or cache it as a structural hash. Ordinary fallible enumeration and comparison
retain typed interruption, including function-domain lookup with 32 or more keys.
A genuine incomparable argument still receives its existing evaluation error or
named normalization refusal. These quiet value units remain part of #349.
Memory is the whole process's: resident
plus swapped pages on Linux, resident pages on macOS. Where it cannot be
measured, checking fails with diagnostic `DIVE-R0002`; it never substitutes
zero memory or silently disables the requested ceiling. An unavailable physical
RAM lookup also fails when resolving `auto`. `--max-memory none` explicitly
disables memory enforcement. `dive tlc` accepts the JVM spelling `-Xmx8g`, or
`--max-memory`.

A finite memory budget takes a native observation before model binding and a
terminal observation before publishing a holding result, plus periodic samples.
An initial known excess returns `stopped` at `check-startup`, with no search
statistics or capture claim. Source/configuration loading errors remain earlier;
future binding effects, assumptions and initial predicates are not evaluated
after that cut. A terminal resource excess reports phase `finalization`,
retaining completed capture and progress without publishing a holding result.
The owner checks its elapsed clock at that boundary even if the watchdog was
delayed. macOS uses a native RSS query instead of launching `ps`.

`resources.memory` reports the platform metric (`rss+swap` on Linux, `rss` on
macOS), successful sample count, observed maximum, initial/final sample flags,
and unavailable status. The observed maximum is **not** the true process peak.
Sampling is not an allocation-time hard cap: excursions between observations
remain possible. macOS RSS does not claim swapped/compressed-memory or physical
footprint accounting. Earlier genuine errors and independently validated
counterexamples retain precedence if a later terminal observation fails.
See [ADR 0109](../decisions/0109-native-memory-budget-sampling.md).

When either runs out, dive stops and reports `verdict: "stopped"` with a
`stopped` block (resource, limit, observed value, elapsed time, unit, phase and
a hint),
the statistics reached so far, and exit status 5. A stop proves nothing about
the model: it is never a success, a violation or `unsupported`. Rust callers
set `CheckRequest::time_limit` and `CheckRequest::max_memory`
(`dive_api::BudgetSetting`). `options.budget` reports the resolved limits and
whether each came from the default, an explicit setting, or none. See
[D-026](../tlc-differences.md#d-026-run-budgets-stop-a-run-with-a-structured-result).

```sh
dive check docs/user/examples/Counter.tla --time-limit 2m --max-memory 4g --output json
```

## State and depth budgets

`--max-states N|none` bounds discovered in-model state identities, using the
same VIEW, symmetry and fingerprint/exact identity as the reported distinct
count. `--max-depth N|none` bounds BFS discovery depth; initial states have
depth one. Both are unset by default, which is their TLC-equivalent setting.
Zero is a valid explicit budget. TLC's simulation `-depth` option does not set
an exhaustive BFS discovery bound.

```sh
dive check docs/user/examples/Counter.tla --max-states 3 --output json
dive tlc --max-depth 3 --output json docs/user/examples/Counter
```

Both examples stop with exit 5 before discovering the fourth state; neither
returns a counterexample. Raise the ceiling to five to finish this finite
counter normally.

The first new identity that would exceed either ceiling is left unadmitted.
Duplicate identities do not consume the state budget. These limits bound
admitted identity discovery. If constraints exclude every initial candidate,
the empty retained graph can complete even with both ceilings zero; its
reported depth still follows TLC's existing depth-one print convention.
States at the depth boundary can still be expanded to already discovered identities, so a finite
graph exactly at both limits may finish normally. A truncated search reports
`verdict: "stopped"`, exit 5, resource `states` or `depth`, the attempted count
or depth in `observed`, actual admitted progress and a continuation hint. It
has no counterexample and makes no successful or temporal-completion claim;
its statistics carry `exhaustive: false`. The resolved limits appear in
`options.budget.max_states` and `max_depth`. An explicit finite ceiling labels
the TLC profile `tlc+overrides`; `none` restores TLC's unbounded setting.

Parallel workers retain their requested pool. Their generated candidates are
published in reference order up to the admission boundary, retaining earlier
errors and violations. Speculative candidates do not increase the admitted
distinct count, trace log or retained graph past the limit. Memory and elapsed
time remain separately governed by their run budgets. Recovery first checks
that its whole saved graph fits the requested ceilings before admitting it.

A prior genuine error or violation remains the deciding result. Diagnostic
error replay also stops admitting identities at the ceiling, and does not
check the invariants of an unadmitted state. Single evaluations and domains
materialized before the first emitted state still need their time or memory
budgets; discovery limits do not interrupt those operations. Rust callers set
`CheckRequest::max_states` and `CheckRequest::max_depth` (`Option<u64>`).

## Process statuses

`dive check` uses TLC process statuses by default, with native semantic choices
still active unless `--compat=tlc` selects TLC evaluation. An invariant violation
exits 12 and a rejected specification 150. Configuration rejection exits 151;
initial/action properties and supported cyclic liveness use 13. Deciding error
classes determine other verdict statuses. Success is 0; usage 2, unsupported or
resource limit 3, internal error 4 and a stop by an explicit or default run budget 5 mean their existing distinct outcomes.
These statuses do not prove an unverified error class or validate a missing trace.

```sh
dive check docs/user/examples/Counter.tla --seed 7 --workers 1 --output json
dive check docs/user/examples/Counter.tla --seed 7 --workers 1 --exit-codes native --output json
```

`--exit-codes native` keeps the historical CLI contract: invariant/property 10,
rejected input 1, deadlock 11, assumption 12 and evaluation error 13. Both inline
`--exit-codes=native` and separate arguments are supported; the last explicit
exit selection wins independently of `--compat` or `DIVE_COMPAT`. Native exits
with TLC evaluation are labeled `tlc+overrides`.

JSON adds the actual selected `exit_code`, `options.exit_codes` and
`options.semantic_profile`; `options.profile` includes compatibility overrides.
Human output displays the selected convention and process status. The existing
`tlc` object describes the deciding error class, independently of the selected
exit convention; `verified: false` stays unverified.

Rust callers keep `CheckReport::exit_code()`, `to_json()` and `render_human()`
behavior. To select explicitly, use `ExitConvention::Tlc` or `Native` with
`exit_code_for`, `to_json_with_exit_convention` and
`render_human_with_exit_convention`. Changing presentation does not rerun the
checker or mutate its evaluator or counterexample.

This applies only to `check`; other commands keep their documented statuses.
The named finite temporal 2116/exit 12 runtime boundary remains unsupported;
selecting TLC statuses does not implement that missing stream shape.

The explicit BFS checker loads a configuration, binds constants and roles,
evaluates assumptions, then explores supported initial and successor states.
It checks invariants, supported action properties, constraints and deadlocks.
`-deadlock` disables deadlock checking, matching TLC's spelling and meaning.
Use the sibling `.cfg` or select one with `--config FILE.cfg`.

A pure `SYMMETRY` declaration is prepared before assumptions and initial states.
Division by zero in that declaration and incompatible values encountered while
normalizing its set report `model-error`, TLC `GENERAL` (1000), with no search
statistics or counterexample. Both semantic profiles exit 255 by default; `--exit-codes native`
uses the historical model-error exit 1. The default comparison policy may produce
a different explanatory message for a mixed set, while retaining this class.
For example, this fixture fails before exploring any state:

```sh
dive check crates/dive-model/tests/fixtures/symmetry-group/specs/SyDivZero/SyDivZero.tla --compat=tlc --seed 0 --fp 0 --workers 1 --output json
```

Other unimplemented startup exception shapes remain named refusals. Effectful,
state-dependent and parameterized declarations still refuse; these diagnostics
do not expand supported symmetry modes or resource bounds.

Fingerprint storage is the normal mode. `--store exact` keeps canonical concrete
keys for supported values; fingerprint collision qualifications do not become
an exact-state proof. Inspect `statistics`, `collision`, `counterexample`,
`warnings`, `notes` and resolved `options` in the JSON report.

Supported pure parallel searches commit results in deterministic reference
order. Read `options.workers.requested`, `used` and `reason`: checker-register
reads and some modes can require an inline run. A requested pool width is not
a claim that every step used that many active threads. `--sequential` explicitly
selects inline checking. Simulation and checkpoint widths have separate limits.

Pure safety symmetry retains concrete counterexamples while canonicalizing
visited keys. Unsupported representative/startup/value interactions refuse.
Ordinary `RandomElement` checking supports a closed pure VIEW and pure startup/Init.
Expansion draws use the VIEW value's TLC fingerprint, while counterexamples retain
concrete variable values. Direct calls and proved configured native replacements
use the same authority. The runnable paired example
shows the reported seed, polynomial, requested width and actual inline width.
Unproved VIEW/startup contexts still return `unsupported` before binding, output
or search. The conservative source walk includes unused definitions and native
aliases, so importing unused Randomization bodies can also refuse this slice.
Concrete values and the VIEW value must fit 16,384 stored-value visits and depth
64: atoms, literal tuples/records, general functions with homogeneous atomic
domains, and explicit homogeneous atomic sets. Compound/mixed sets, symbolic or
deferred sets and nonliteral tuple/record representations remain named refusals;
these extra shape limits do not widen the unresolved mixed-word certificate
boundary in #190. No provider callback or comparison is used to grant admission.
Pure VIEW and direct source-call random checking without VIEW remain available.
VIEW normalizes all concrete variables before computing a visited key, even
variables the projection ignores. If a value cannot be normalized on the
proved bounded path, the result is `unsupported`; an unprintable state never
becomes a validated counterexample. See the measured boundary.
Random checking reports its actual inline width separately from requested workers.
Ordinary safety checking admits the configured `RandomElement` route, for example
`CONSTANT C <- RandomElement`, using the actual bound native operator for startup
and predecessor-stream selection. Native definition redirects and admitted typed
context aliases follow the same authority. These runs execute inline and disclose
requested and used worker counts separately. Pure user replacements still use the
parallel engine where admitted.

The committed configured-operator example is runnable directly:

```sh
dive check docs/user/examples/Stream.tla --compat=tlc --seed 7 --fp 0 --workers 8 --output json
```

It reports an invariant violation with generated/distinct/depth `4/4/4` and
concrete values `0,80,94,100`, matching clean pinned TLC. Although eight workers
were requested, the report discloses the actual inline width of one.

A random violation on the first successor of an initial state
currently returns `unsupported` with its observed stopping-prefix statistics.
TLC's initial-predecessor printing path can keep an already-consumed stream and
omit the final state; dive has not proved that replay contract. The report contains
no counterexample certificate. The conservative source-wide random detector can
also refuse this boundary when an unused definition contains a random call.
This boundary applies in both profiles;
a new full-witness override has not been adopted. Startup draws before random Init can likewise
prevent reconstruction; a seed alone never creates an independent certificate.
The standalone validator also refuses unproved draw positions for multiple draws
or conditionally discarded random branches. A matching concrete value does not
establish where the generator consumed its stream. A checker violation whose
witness is missing or independently `unsupported` never counts as full agreement.
Configured `Randomization` replacements, SYMMETRY with randomness, graph
capture, temporal random checking and seeded native recovery remain explicitly
refused. The callback-free configured-target scan remains conservative across
matching modules and aliases. For graph capture and the search's random stream,
a specification counts as random when a definition in a non-standard module,
used or not, reaches a random operator through the definitions it references.
Extending `Randomization` or `TLC` without calling a random operator is not
randomness: the standard modules' own helpers, such as
`TestRandomSetOfSubsets`, count only when something outside them calls them.
See the original wrong-answer repair
and bound-stream controls and remaining replay limits.
The corrected standalone campaign independently validates 144 of 180 original
witness controls, refuses 36 unproved draw positions, and rejects all 180
changed-Init controls. The remaining refusals cover two draws, a discarded random
branch and conditional random Init. These counts belong to the corrected
producer; the original failed campaign's 48 refusals remain historical evidence.

Eligible ordinary dive-profile requests check temporal properties at deterministic
completed-level boundaries; ordinary TLC-profile requests use the bounded pure
final analyzer. Unsupported combinations retain named refusals. See
[temporal checking](temporal.md) for exact eligibility, actual schemas and limits. The explicit final/short mode
remains available in either evaluator profile. Refused library
operators, unknown finite deadlock covers, resource stops and internal failures
must never be treated as `success`.

See [compatibility](compatibility.md), [storage](storage-recovery.md),
[reports](reports-and-errors.md) and the [complete coverage ledger](../scoreboard.md).

A lazy filter ignored by VIEW does not require evaluating its predicate. The
checker normalizes its stored bound set only. `x = {i \in Nat : Print(i,
TRUE)}`, `Next == UNCHANGED x`, and `VIEW 0` finish with no Print output.
Spilling or archiving that callback-bearing value still refuses explicitly;
this support is limited to memory checking. Other unproved lazy representations
remain unsupported.

The current D-009 comparison improvements do not make every mixed structure
normalizable. A reversed heterogeneous set may refuse even in the default
profile. A forward heterogeneous set can produce a checker report whose
canonical structured trace then fails decoder shape admission in both profiles.
Treat that report as an unvalidated witness; checking counts or a displayed
trace do not confer independent validation. See the measured selection and
refusals and
[follow-up #190](https://github.com/Chelis-Lang/dive/issues/190).


For automation, an invariant violation uses TLC's physical status (normally 12)
in both semantic profiles. `--exit-codes native` selects the historical status 10.
`--compat=tlc` also selects TLC evaluation. Conformance
compares these numeric statuses exactly. Known default differences count as
ExpectedDivergence only for declared exact pairs and valid independent witnesses,
never as Agree. See [reports and errors](reports-and-errors.md).

The conformance harness requires whole counterexamples for safety, action-property,
liveness and deadlock verdicts at every width. Missing or metadata-only words
receive no agreement credit; present words must validate independently. This
also holds when a worker-specific comparison omits witness shape.

If the host cannot start a requested budget watchdog, checking does not begin.
The CLI prints `DIVE-R0001` and exits 4; this is a runtime failure, not a verdict,
a refusal or exhausted time/memory budget. It is an admission error on stderr,
like an input-loading failure, rather than a completed JSON check report. Free
host resources and retry, or explicitly choose `--time-limit none --max-memory none`.
Managed `dive_api::check_file` callers receive `ApiError::BudgetStartup`;
`dive_check::check` returns an internal-error result, and liveness analysis returns
`AnalysisError::BudgetStartup`. Lower-level `Budget::start` and `budget::arm`
return `Result` so this failure cannot look like an unlimited budget. Already
armed or unlimited budgets still need no new thread.

Low-level `capture::*` functions expect the caller to arm a budget and retain
its watchdog through capture and analysis. Use the returned armed options from
`budget::arm`, or clone `watchdog.budget()` into the options: `Budget::start`
does not mutate the original budget. An unarmed budget has no running clock and
its polls return no exhaustion. These low-level primitives do not resolve CLI
defaults or arm a watchdog themselves.

The primary `interval-streaming` corpus records TLC's ascending, incremental
`IntervalValue` enumeration: `x # x` fails on the first initial state even for
a billion-element domain. A constant `FALSE` invariant instead fails configuration
validation before enumeration. The corresponding initial and successor witnesses
and a complete five-state interval are recorded separately from the established
conformance populations; see [the corpus guide](../conformance-corpora.md#interval-enumeration-resource-controls).

Unassigned `x \in lo..hi` initial predicates and `x' \in lo..hi` successor
assignments stream ascending integers without first retaining the whole domain.
This lets existing checker guards run when a complete assignment reaches the
sink. `ENABLED` likewise streams unassigned interval-membership candidates,
stopping at its first witness. Its membership-candidate loop reaches the
existing amortized evaluator control even when every candidate is rejected.
Already assigned variables still use the ordinary membership check.
Bounded evaluator contexts also stream primitive interval columns for
quantifiers and their shared binder traversal. Every domain is still evaluated
before the first body, including domains after an empty interval; the first
variable varies fastest and tuple validation keeps its original priority.
Existing loop checkpoints can therefore run without first retaining the entire
interval. Existing cardinality admission errors still apply before the first
body in both profiles. APIs requesting all bindings still collect the full
result. Other constructors can still materialize a domain before candidate binding, and long
individual evaluations remain potentially uninterruptible (issue #349). No fixed
domain-size count is introduced. Parallel checking polls the armed run budget during
successor emission (issue #362). A stop retains its actual ordered prefix and
leaves the unfinished source queued; a later worker's violation cannot bypass an
earlier incomplete expansion. A predicate such as
`x' \in 0..50000000 /\ FALSE` emits no complete successors, so the emission
guard alone cannot interrupt it. The additional amortized candidate checkpoint
now observes the budget before binding while this rejecting search proceeds
(issue #393), including inside private action-composition collection. Quiet
normalization/equality/hash units, domain materialization before the candidate
loop, and single long evaluations retain their limitations. For a bounded
first-state violation with an independently
validated witness, run:

```sh
dive check conformance/corpus/interval-streaming/specifications/WideIntervalInit/WideIntervalInit.tla --compat=tlc --time-limit 1s --max-memory 1g --output json
```

The invariant fails on `x = 0` (TLC class `TLC_INVARIANT_VIOLATED_INITIAL`, exit
12); this is not a successful proof over the rest of the interval.

Compound assignment domains with explicit scalar or interval components
also yield candidates incrementally: powersets, Cartesian products, function
sets and record sets reach the initial/successor sink and `ENABLED` continuation
without retaining every output. Required component normalization still precedes
the first candidate. Candidate checkpoints also run when a continuation rejects
every value, allowing the configured run budget to stop that search.

This changes output storage, without imposing a new cardinality limit. Ordinary
normalized bulk operations, deferred component providers and values requiring a
different exploration order use their existing paths. A compound component may
still require materialization; this does not make every domain construction
interruptible. The four pinned first-state controls and their independent trace
validation are documented in the [corpus guide](../conformance-corpora.md#compound-domain-first-state-controls).

Temporal analysis borrows the captured graph's edge targets and roots while
constructing labels and products. It retains every captured state and original
concrete successor occurrence. This avoids a second behavior adjacency copy; it
does not change capture, graph-label, product or acceptance limits, and does not
provide disk-backed temporal capture.

The [source-bound qualification](../survey/data/borrowed-topology-qualification-2026-10-23.json)
preserves all 312 small-tier model runs and 458 SANY agreements. Its eight normal
instruction pairs stay within 1% overhead (maximum 0.0301%). Four ordinary-release
CoffeeCan observations are descriptive; they establish no quantitative memory
or throughput gain. All measurements run dive, including its TLC compatibility
profile; this adds no capacity or parity claim.

The `liveness-atom-error-timing` validation microcorpus contains four pending
primary controls for temporal atom evaluation: a delayed source error with a
pending sibling, competing action/state errors, combined-claim simplification,
and candidate residual-tableau initial consistency. Their manifests record
unknown results and runtimes. No oracle has run for these inputs and no goldens
are supplied; neither the error site nor a selected class, exit, count, queue,
depth or word is established. They do not change the established small/full
corpus selections or qualify temporal evaluation errors currently refused under
issue #451.
