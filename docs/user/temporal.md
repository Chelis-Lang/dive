# Temporal checking

The checker exposes bounded temporal analysis. Eligible ordinary dive-profile
requests check at deterministic completed-level boundaries; ordinary TLC-profile
requests check only after exhaustion. Explicit final/short selection is available
in both profiles:

```sh
dive check docs/user/examples/Counter.tla --config docs/user/examples/CounterTemporal.cfg --lncheck final --lasso short --seed 7 --output json
```

The example reports a real liveness violation. Although `Next` always increments
before four, TLA+ temporal behaviors can stutter at zero; there is no fairness
condition in this model requiring the increment action to occur.

This mode explores the supported graph, checks once at the end, chooses a
deterministic short periodic witness and independently replays it in memory and
as a lossless document. It checks the complete configured property claims,
including the closing edge. This explicit mode does not use periodic
scheduling or claim all evaluation-error timing. Explicit requests retain their
existing `tlc+overrides` label when combined with TLC value semantics.

At one worker, a bounded temporal source-row error can now retain TLC's
GENERAL1000 and TLC exit255 in either execution engine. The checker must reach
the first actual source boundary after exactly one generated and accepted Init
state, prove that its concrete destinations are the source itself, execute
ordered temporal demand and its diagnostic replay, and then
independently validate the finite original word on a fresh runtime. This covers
both a Next failure before any complete emission and a successful self successor
followed by a fairness error. A complete self emission before a later Next error
is covered too. The report has no violated property or lasso, and capture remains
incomplete. Native trace version5 carries the source-only endpoint (ADR 0132).
An unproved or quiet first row disables this optional lane; it cannot skip that
row or its opaque residual and later certify another source as first.

The original `MinChooseFair` and `MinChooseFairSafeNext` controls now agree with
the pinned oracle, as do action-before-state demand and prior-self-emission
controls. `WF`/`SF` syntax alone grants no classification. A quiet earlier action
bin may reach a later state error; an earlier failing action bin takes priority.
State-only residual tableau work is an opaque boundary: reaching it supplies no
holding, completed-row or residual-truth certificate.

Unknown source order, non-self targets, effects, VIEW/symmetry, cached/history
questions and unsupported parallel source events remain explicit refusals.
Other retained-primary or candidate-ending lifecycles also remain unproved.
In particular, an action-candidate error with pending temporal obligations may
not borrow the ordinary2103/75 classification or the new source ending. These
refusals retain actual counters, output and diagnostic context. Successful
fairness checks and the separate2179/2182 temporal-state-row meaning remain
unchanged. Issue #502 remains open for the uncovered boundaries.

The native optional lane performs no source binding or reference row evaluation
for compound roots. It uses a callback-free eligibility check, waits for an
actual scalar source event before binding, and reuses one hidden owned row
runtime. Existing native constructed-action execution remains available.

Property attribution is relative to the actual independently validated word.
The report's `properties` is the complete configured false-property list on that
word; `property` and the human headline format the same list. A different valid
lasso can violate a different set of configured properties. Compatibility does
not require copying TLC's set onto a word where some named property holds.
The harness accepts this difference only after fresh independent whole-word and
complete-attribution validation; all required verdict/count/depth/class/exit
comparisons remain in force (ADR 0126).

## Native graph capture with the core engine

Ordinary Core temporal checks retain persistent original state rows
and compact CSR edges. The existing temporal analyzer reads values on demand;
a bounded cache reuses successful closed tuples, records, functions and fully
materialized finite sets; other values use controlled conversion. It retains no second complete evaluator-value
graph. This changes storage, not the supported temporal language, fairness,
property attribution or completion requirements. Core is the default engine.

Native action-predicate evaluation avoids repeated successor scans while
retaining the first original successor values, implicit stuttering and the
same demanded value-conversion failures.

```sh
dive check docs/user/examples/Counter.tla --config docs/user/examples/CounterTemporal.cfg --workers 1 --seed 7 --output json
```

This checked example returns `liveness-violation` for `EventuallyFour`, exit 13:
5 distinct states, 6 generated states and depth 5. Its one-state word stutters
at `x = 0` and has `counterexample.validation: "valid"`. Capture is exhausted;
no prefix check is due for this small graph. Adding `--compat=tlc` keeps those
observables and selects final-only analysis. The default profile retains its
deterministic completed-level schedule. Explicit final/short requests retain
their existing separate route.

Pure native temporal capture can use all requested workers under `--compat=tlc`
when its real startup name table is fixed. For example:

```sh
dive check crates/dive-api/tests/fixtures/core-liveness/FixedNames/TupleSubSeq.tla --compat=tlc --workers 8 --seed 0 --fp 0 --output json
```

This tuple `SubSeq` cycle returns success with 3 distinct states, 4 generated
states and depth 3, as pinned TLC does at one worker. The same default-profile
request is supported. No model, operator-name or function-name whitelist is used.
The native producer refuses any name absent from its sealed startup table;
reference temporal evaluation must also leave its exact private name domain
unchanged throughout planning, capture, final/prefix analysis and reporting.
Actual new names in temporal atoms therefore yield explicit unsupported before
a semantic result escapes. Independent replay has its own fresh-domain guard.
Source/configured/captured effects, foreign scopes and unproved value shapes
remain refused, and reference-pool scheduling is unchanged. This is no new
runtime-string ordering capability or full-model performance qualification.

JSON schema 3 preserves the actual completion/counters and existing trace
format. `core.metrics.liveness.storage: "native-csr"` identifies native capture;
`convert_ms`, `converted_nodes`, `memo_entries` and `graph_states` remain,
with additive `values` counters for admission, on-demand reads, cache hits/misses,
entries, requested bytes, replacements and bypasses. `retained_rows` reports the
physical native row count. `convert_ms_scope` makes explicit that conversion timing
excludes one-variable cache hits. Cache allowance is the
smaller of 128 MiB and 1/64 of the run memory budget (128 MiB when unlimited).
It charges actual table capacity and known eligible payload; shared run-owned
name payload and opaque Arc/allocator overhead are outside that metric. Collector `memory` fields cover native
buffers and action-label capacities; growth overlap is an upper bound,
`accounting_complete` is false, and sampled process memory is not a true peak
or an allocation-time hard cap.

Every accepted original is admitted in producer order through exact Graph
conversion before retention, including when a prior read has cached that handle.
When every liveness atom runs natively, that admission is deferred until a row
is first read (a reference fallback or a report), and then done in the same
order; `values.admission` reports `"deferred"` or `"eager"`.
Successful Bool/Int/run-owned-string leaves and recursive tuple/record/function
conversions can be reused during reads; general function keys must be scalar.
Sets, BigInt/model values and unproved shapes continue through conversion.
Inspection exceeding its work/depth bound bypasses caching. Exact persistent
handle identity and the fixed run/store/profile isolate entries. One-variable models reuse the cached shared row on a hit; multi-variable
models assemble rows from cached components. In-range retained row IDs select
one-variable cache slots directly, with exact handle equality still required;
other reads use the bounded handle lookup. The cache charges the shared row
array storage as well as its table and eligible value payload, including
possible later scalar-domain ordering storage. Budget checks
precede hits. The table initializes after 1,024 read
lookups; unproved, oversize or unavailable cache storage bypasses reuse. Unknown set normalization history is refused;
function versus tuple/record literal classes remain distinct. A selected word
outlives capture, but the native graph, converter, store and plan owners are
released before a freshly bound model independently replays both the typed word
and serialized trace. Invalid or interrupted validation publishes no violation.

Ordinary core file requests use the actual `--time-limit` and `--max-memory`
budgets across capture, analysis and validation, rather than artificial state,
edge, generated-state, product-work, acceptance or attribution/word count ceilings.
Checked machine layouts, 32-bit native row/vertex ids and fallible growth remain;
parallel capture still expands at most eight sources per wave. Budget stops
retain actual statistics, stage and capture-completion truth and never trigger
an automatic retry on the reference engine. Explicit Rust limits, reference
routes, final/short selection and formula/tableau/property planning bounds retain
their existing policies. Existing core admission refusals remain.
VIEW/symmetry quotient semantics, effects and temporal error-demand order are
separate limitations. A core refusal is explicit and never falls back to the
reference route. This implementation establishes no larger TLC-relative capacity
or throughput improvement. Complete
memory accounting and original-workload qualification remain open.

Temporal product construction uses exact pair lookup and restores deterministic
search order before choosing a witness. The optimization applies in both
profiles under the existing budgets; it changes no option or supported formula.

## Ordinary checking in the dive profile

With profile-default evaluation, resident fingerprint storage and no storage,
deadlock, sequential or chaos overrides, ordinary `dive check` uses
`options.lncheck: deterministic`. Binding and safety-role dispatch retain the
original startup host and error priority. After each fully committed BFS
expansion level with an unexhausted frontier, the graph is checked when it has
at least 65,536 (2^16) distinct states and at least twice as many as at the
previous check (ADR 0018 §7). The doubling bounds the cost of all prefix checks
together by about one final check, and the schedule depends only on the graph,
so results are the same at every worker count. Workers finish and join their
level work before checking. Unexpanded frontier rows remain empty. Exhausted
levels skip the prefix check; the complete graph receives one final check. A
model with fewer than 65,536 states is therefore checked exactly once, at the
end, with the same verdict and counts as `--lncheck final`.

A prefix can produce only an independently validated violation or a named
failure. It never proves that a property holds. A deciding prefix stops at that
actual boundary, skips postconditions and reports its actual generated/distinct/
depth/queue counters, `capture_completed: false` and `exhausted: false`. A fresh
model validates both the typed periodic word and its serialized trace, including
original `Next`, configured property names and the closing edge. A refused or
invalid word publishes no violation or counterexample. Only real exhaustion can
produce `Holds`. TLC timing, queue order and final-oracle counts are not copied.
A prefix check searches only the first configured property: a violation of it
on a prefix is also a violation on the complete graph, where the final check
would name it first. Violations of later properties wait for the final check,
so the reported property is the one `--lncheck final` reports.

Schema 3 reports the schedule (`min_states`, `growth_factor`), prefix check
count, last checked level/size, exhaustion and final-check truth in `periodic`.
Native capture interrupted by its run budget retains these actual facts,
including zero checks when no level was due. Such a default-profile run keeps
`options.lncheck: deterministic`; a final-only run keeps `final` and omits
`periodic`. Interruption supplies no holding result or counterexample.
Rust callers can choose another schedule with `CheckRequest::periodic`
(`dive_api::PeriodicSchedule`); the CLI always uses the default. Capture bounds are
cumulative across the search. Planning happens once; label/product/formula
bounds retain their actual per-property and per-analysis-check scopes. Repeated
checks add computation: these bounds promise neither a total-run work/byte cap
nor a speedup. Explicit `--lncheck final --lasso short` retains the final schedule.

The conformance adapter reads these actual default prefix counters and schedule
facts alongside ordinary TLC final reports. It independently replays words; a
producer's validation marker is insufficient. Required result comparisons stay
strict, so prefix counts are not replaced with final-oracle counts or waived.
Goldens use `-lncheck final`; a model whose violation the default profile finds
on a prefix stops with fewer states than the golden, which is a D-014 count
divergence. Like TLC's own periodic checking, a prefix violation can also
preempt an invariant violation or evaluation error beyond the checked prefix
that a final-only run reports instead. The
TLC-equivalent setting is `--compat=tlc` or `--lncheck final --lasso short`.

## Ordinary checking in the TLC profile

```sh
dive check docs/user/examples/Counter.tla --config docs/user/examples/CounterTemporal.cfg --compat=tlc --fp 0 --seed 7 --output json
```

This ordinary request uses the same bounded pure final analyzer and independent
word validation. No lasso-policy flag is needed. It finishes graph exploration
before checking temporal properties and chooses a deterministic short valid word;
TLC's particular SCC, lasso, timing and queue order are not reproduced.
The report records the actual `lncheck: final`, `lasso: short` and temporal
bounds, with profile `tlc`. This is partial temporal support, not general TLC
parity or an implementation of periodic checking.

The route requires profile-default evaluation and no storage
budget/directory or chaos overrides. Any `--store`, `--fp` polynomial,
`-deadlock` and `--sequential` setting is accepted: temporal capture retains
concrete states, so neither the store nor the polynomial affects its result,
and the report carries no collision bound. A request excluded by one of these
options is refused with a message naming the option. Seed and
worker requests are retained. Checkpoint/recovery uses its existing separate
safety path. Randomness combined with VIEW or SYMMETRY, excluded options and
requests with excluded dive-profile options retain their existing routes and named
refusals. Real unsupported language features and resource limits remain explicit;
a refusal, bound or external timeout is never a holding result.

Reference file-based temporal checking and explicit final/short selection use
8,000,000 covered acceptance-buffer bytes and
32,000,000 independent acceptance materialization work units, in both profiles.
Packed words, column headers, optional edge offsets and Boolean scratch count;
capture, labels, graph construction, search, runtime and allocator overhead do
not. This is not a total memory or search-work limit. Other evaluator,
retained-edge and transition limits remain unchanged. JSON names the byte/work
bounds under
`options.temporal_bounds`; exhaustion is an explicit resource refusal. Existing
Rust custom-limit APIs keep legacy logical acceptance cells; the additive
`check_final_with_policy` API selects byte/work admission explicitly. See
[covered allocations and exclusions](../temporal-api.md) and ADR 0098.

### Product vertex capacity

File-based temporal requests in both profiles use `usize::MAX` as the numeric
ceiling on reachable behavior-state/tableau-node pairs. This is the machine's
finite index capacity, not an infinity marker or a memory allowance. The report
keeps the exact configured number in
`options.temporal_bounds.product_vertices`. Ordinary completed-level checks and
final checks use the same setting; there is no model-specific admission rule.

Rust callers retain their supplied ceilings. `product::Limits::default()` stays
at 1,000,000 vertices, and every existing final/periodic entrypoint enforces
explicit zero, exact boundaries and an explicitly supplied former default.
Callers can already supply `usize::MAX` when they want machine-index capacity.
No numeric value is interpreted as permission to ignore the admission check.

Outside the ordinary core file route described above, product edges, transition
work, acceptance buffers, captured graph, tableau, attribution and periodic-word
bounds retain their independent settings. Checked
size/layout arithmetic and existing fallible reservations remain in place.
Whole-process time and memory budgets retain their documented cooperative and
sampling limits: individual allocations and uninterruptible units can still
outlast a poll or fail at the host. A refusal or stop never proves a property.

The [CoffeeCan feasibility record](../survey/data/coffeecan-product-vertex-experiment-2026-10-22.json)
is a separate instrumented custom-ceiling experiment. Both native profiles
completed the pinned model and two groups each retained 1,003,000 vertices.
Its original offline classifier HOLD remains preserved alongside a reviewed
zero-buffer interpretation correction. It is not production-default, current
corpus, causal speedup or true peak-memory qualification.

The separate [production qualification](../survey/data/product-vertices-qualification-2026-10-23.json)
at source `db8d4ad5` passed the full gate and both complete small-tier baseline
comparisons. Both ordinary CoffeeCan1000Beans profiles completed all configured
properties with 501,500 distinct states, 2,000,002 generated and depth 1 under
an explicit 564,520 ms budget and 8 GiB ceiling. All eight normal instruction
pairs stayed within 1% overhead; this is capacity evidence, not a throughput claim.
From the pinned Examples CoffeeCan directory, use:

```sh
dive check CoffeeCan.tla --config CoffeeCan1000Beans.cfg --workers 1 --seed 0 --fp 0 --time-limit 564520ms --max-memory 8g --output json
dive check CoffeeCan.tla --config CoffeeCan1000Beans.cfg --workers 1 --seed 0 --fp 0 --time-limit 564520ms --max-memory 8g --output json --compat=tlc
```

Binding happens once under the ordinary startup host. If the configured roles
are all safety obligations, their complete ordinary result uses schema 3 with
`check_kind: "safety"`,
including startup Print output, randomness and error ordering. Genuine temporal
analysis uses schema 3. A completed holding result or violation reports actual
generated/distinct/queue/depth counters and `execution.exhausted: true`, even
though the legacy `reported_by_tlc` print marker is false. A violation names its
property and carries a lossless periodic word independently replayed against a
fresh model. TLC exit conventions give success 0, temporal violation 13,
named unsupported/resource refusal 3, and classified evaluation errors their
reported TLC status; inspect the verdict and execution stage as well as the exit.

Core analysis may evaluate certified closed-input predicates and bounded fairness
tests directly from retained original states. Proved ordinary action atoms may
construct functions over integer intervals, use bounded interval CHOOSE and make
single-key scalar EXCEPT/@ updates. Scoped non-stuttering tests compare the
before and after projections first; hidden ENABLED queries retain their existing
reference boundary. Unproved scopes, values, updates and evaluation errors use
the existing reference evaluator. The additive
`core.metrics.liveness.atoms` counters show prepared eligibility, successful
native evaluations by kind, and fallback; they do not establish throughput or
whole-model completion. State admission and independent word validation retain
their existing checks. See [the native evaluation boundary](../developer/core/11-liveness.md#41-optional-stable-native-predicate-evaluation).

The ordinary conformance adapter retains those actual complete counters and
requires coherent schema, class, physical status and word metadata. It still
replays the word independently: a producer's `validation: valid` field is not
proof. Periodic liveness words may differ in length or closing target while
Distinct counts, depth, property, error class and physical exit remain
strict; generated-count equality is required at one worker. Queue population is retained as raw data and as an internal final
exhaustion consistency check, rather than compared with TLC's queue population. Finite
counterexample checks retain their existing comparison rules.

When the configuration has no `PROPERTY` entries, these flags leave ordinary
safety checking in charge: invariants, initialization/successor errors,
assumptions and other safety settings retain their usual behavior. No temporal
claim or capture proof is issued.

With default resident-memory settings, configured properties whose actual bound
roles are entirely initial predicates, invariants and implied actions also use
that ordinary engine. Each obligation retains its scope, order and name; the
checker binds once and retains its startup output and random stream. This initial
route requires profile-default evaluation/fingerprints, polynomial 0, fingerprint
storage, no storage budget/directory options, deadlock or sequential override, or
chaos flag. The TLC profile also admits `--action-composition=tlc` with every
other setting unchanged, for example:

```sh
cargo run --locked --offline -p dive-cli --bin dive -- check \
  crates/dive-api/tests/fixtures/ordinary-cdot/CompositionFinal.tla \
  --config crates/dive-api/tests/fixtures/ordinary-cdot/hold.cfg \
  --compat tlc --action-composition tlc --fp 0 --seed 0 --workers 1 --output json
```

This checked Hold example reports success, with 5 generated states, 2 distinct
states and depth 2.

The Rust counterpart is `CheckRequest { compat_tlc: true,
action_composition: Some(ActionComposition::Tlc), fp: Some(0), ..request }`.
Composition intermediates retain their original generated-state charges and
sequential execution reason. This option does not add independent composition
replay: a candidate requiring composed original Next or a composed closing edge
still refuses without a published word. Holding on a complete captured graph
requires no candidate replay; genuine violations still require it. Composed
ENABLED/fairness atoms are decided by the evaluator's port of TLC's composed
`ENABLED`; a violation that needs them still refuses at replay. TLC's
`off` default and the default profile are unchanged; `tla` and other evaluator,
storage or explicit sequential overrides do not gain ordinary dispatch support.
Seed and worker requests are retained. Other options and randomness
under VIEW or SYMMETRY keep the existing final-mode route and error priority.
Real temporal property roles still require temporal analysis and independent
periodic-word validation. A state-only `PROPERTY P` is checked in the initial state by the ordinary engine;
`[]P` becomes an invariant. An evaluation resource limit in those safety roles
stays a resource limit, including recursive calls. A genuinely temporal formula
such as `[]<>P` still goes through temporal planning; recursive calls outside
the state-level rule below retain their named refusal. These are different requested obligations.


Pure supported boolean/temporal operators, finite quantification and WF/SF
predicates have scoped limits. Unknown ENABLED/fairness covers, runtime effects,
dynamic/exported scopes and other unproved constructs refuse. Eligible ordinary
requests use the profile schedules described above; excluded options retain
their named refusals. A diagnostic planning/tableau/product API that completes
has not issued a temporal verdict.

Direct configured operator constants can call nonrecursive top-level user
operators with ordinary value parameters inside pure state/action predicates.
The original call and its lazy arguments are retained, including primed
arguments in fairness actions. For example:

```sh
dive check conformance/corpus/configured-temporal-operators/specifications/ConfiguredReplyHolds/ConfiguredReplyHolds.tla --compat=tlc --fp 0 --seed 0 --output json
```

This model configures `Reply <- MCReply`, uses `Reply(0,1,x,x')` as its fair
action, and checks that `x` eventually becomes 1. The paired
`ConfiguredReplyFails` model checks `[]<>(x=0)` and has a temporal violation.
Both value profiles use the same scoped structural checks and existing limits.
Configured native, recursive, higher-order, value-overridden and temporal
targets, captured configured operators and substituted operator calls retain
named refusals. Within configured calls, effects, temporal operands and
ENABLED inside a pure atom, including unused actual arguments, remain refused. Independent trace replay
checks the original configured calls separately.

Temporal preflight distinguishes an absent INSTANCE substitution from an
explicitly unset binding. An absent reference can use its original variable
declaration or configured constant instead of being rejected as opaque. Pure
zero-argument constant redirects are inspected in their actual scopes; value
bindings and lazy substitutions keep their existing meanings. An unused
INSTANCE import therefore does not hide a configured constant from checking.
For example:

```sh
dive check crates/dive-trace/tests/fixtures/instance-constants/constant_absent_violation/Constant.tla \
  --config crates/dive-trace/tests/fixtures/instance-constants/constant_absent_violation/Constant.cfg \
  --compat=tlc --fp 0 --seed 0 --workers 1 --output json
```

This model reports property `Bad` violated, code 2116 and exit 13, with three
generated states, two distinct states and depth two. Its periodic word passes
independent validation. The paired configured-redirect fixture has the same
result; the original holding fixtures report success. Explicit final checking
also supports these cases with default value semantics: omit `--compat=tlc`
and add `--lncheck final --lasso short`. Ordinary default-profile temporal
dispatch keeps its existing support boundary. Explicitly unset or opaque
bindings, effectful/recursive/nonconstant constant redirects and missing finite
ENABLED covers retain their named refusals.

Pure state predicates can call authoritative `Sequences!SelectSeq` with a unary
user definition or `LAMBDA`, including a known forwarded operator and lexical
captures. For example, `SelectSeq(<<0,1,0>>, LAMBDA y: y=x)` returns `<<0,0>>`
when `x=0` and `<<1>>` when `x=1`; duplicate positions and order are preserved.
The planner and independent trace evaluator resolve the captured scopes
separately, so a forwarded state callback is not folded as a constant.

Pure bounded callback binders and captured function, filter and map sources are
inspected without enumerating their domains. Nested authoritative `SelectSeq`
calls and finite local recursive-function captures retain their original
scopes and share the same admission work budget. For example,
`SelectSeq(<<0,1,0>>, LAMBDA y: \E z \in {y}: z=0)` returns `<<0,0>>`.
The reference corpus `selectseq-captures` pairs holding and violating models
with independent replay controls. The shared admission allowance counts source
visits; it is not a ceiling on lexical-frame copying or allocation.

This subset excludes referenced definition value overrides, opaque stored
functions/providers, action or temporal
callbacks, recursive user-operator callbacks, native/configured predicate
targets and retained dynamic callback contexts. Unused captured bodies and
stored values are checked too; even an empty domain cannot hide effects or a
primed source. These combinations report a named
`unsupported` result. Other higher-order standard operators remain refused.
This includes direct and negated `ENABLED` and WF/SF enabledness atoms when the
independent validator can derive complete finite successor covers. Ordinary
unary pure state helpers enter through their original lazy lexical arguments;
callback captures and sequence dependencies remain part of the cover checks.
Missing covers, explicit primed/action callback sources, generic higher-order
action forwarding and action composition retain named refusals. Independent
replay can also prove fairness from an action recurring on the repeated cycle
without querying enabledness.
Atoms are evaluated by the core evaluator, as TLC evaluates them: the native
loop invokes the original callback on each element and stops at the first
error, empty sequences invoke no callback, only the branches an evaluation
reaches are evaluated, and definition and lazy argument results are cached as
for invariants. An enabledness atom runs the evaluator's `ENABLED` search
(ADR 0103). Planning still inspects callback sources without evaluating them.
Both value profiles use this same subset.

Schema 3 reports explicit final requests and actual ordinary temporal analysis;
ordinary safety results also use schema 3 with `check_kind: "safety"`.
`properties` names complete claims, and periodic counterexamples carry validation
and a closing edge. Read [the temporal API contract](../temporal-api.md) for the
Rust `temporal::check` example, supported forms, limits, source-qualified errors
and indexing conventions.

## Final checking in the TLC profile

`dive check --compat=tlc --lncheck final --lasso short` checks temporal
properties with TLC's value semantics and reports a deterministic short lasso
that passes independent replay. The profile is labeled `tlc+overrides`, because
TLC's own lasso choice is not reproduced: it depends on hash-table and SCC
visitation order, which are not parity obligations (ADR 0082, ADR 0086).
`--lasso tlc` is a usage error. Explicit final requests check only after
exhaustion; the ordinary dive profile uses the deterministic level schedule
described above.

Malformed numeric temporal operands, such as `Bad == <>1`, report
`evaluation-error` before exploration, with TLC code 2215 (`TLC_EXPECTED_VALUE`).
The process exits 255 in both semantic profiles; `--exit-codes native` selects 13. Effects
and unsupported scopes still refuse before planning executes them.

Action predicates use the first generated concrete successor for each
destination identity at a source. Later constructor aliases and the implicit
stutter do not replace that selected target; exploration still counts every
generated successor. This matters when TLC distinguishes a function constructor
from a record during comparison. A single direct `<>[]A` or `[]<>A` action
operand preserves an actual evaluator `GENERAL` error as `evaluation-error`,
TLC code 1000 and process exit 255 (`--exit-codes native`: 13). The report names
the `temporal-action` error site and retains completed capture counts.

This error routing does not establish general temporal error ordering. Synthetic
nonboolean results and uncertain compound or multiple-property action errors
remain unsupported. State predicates continue to use canonical captured states;
some residual-tableau constructor errors still refuse during independent
validation rather than reproduce TLC's model error. No counterexample is
published after that refusal. Action-only constant simplification excludes
eliminated temporal operands only at graph labeling. Existing eager candidate
attribution and independent validation can still refuse eliminated-action
candidates; those refusals emit no model error or counterexample. The additional
action-membership pass summarizes the original formula DAG and its negative
polarity without retaining copied normalization trees. It preserves original
action occurrence IDs, plain Boolean constant simplification and exact direct
atomic GF/FG classification. Repeated conjunction/disjunction operands remain
multiple operands even when their action IDs coincide. Input-term, depth and
actual-work limits still apply; all references are validated before constant
pruning. This pass neither classifies TLC formulas nor changes tableau rules,
limit defaults or predicate-local lazy Boolean guards. A model can reach a
separate existing graph/product bound after this temporary-copy refusal is
removed; new corpus agreement is claimed only after bounded qualification.

The [source-specific Allocator observations](../survey/data/active-action-membership-2026-10-21.json)
preserve the prior `dd2f0907` measurements separately from fresh integrated
`85592819` results. Both profiles at that historical source use the unchanged
pinned configuration, one worker, seed/fingerprint zero, and explicit
361.620-second / 4 GiB budgets. They complete capture with 17,701 distinct
states, 64,414 generated states and depth 16, then report `DIVE-L0034`,
`atom cells`, at stage `atoms`. The report does not identify a graph-label,
attribution or word-cell sub-bound or give its dimensions. These results remove
the temporary-copy refusal but prove no property, improve no agreement count
and justify no further count removal. Early refusal timing is not compared
with TLC's completed check; sampled process memory is neither a true peak nor
a projected allocation lower bound.

That historical source passed the full gate: 2,178 tests with no
skips, 458 SANY modules, and the small corpus at 147/156 in the TLC profile and
142/156 plus five expected divergences in the default profile. Both profiles
retain nine unsupported models, with no baseline changes, disagreements,
timeouts or pending trace validation. The compact evidence keeps the original
before-run identities, producer archive and report hashes. A documentation-only
delivery preserves that qualified runtime, test and harness source; it does
not rename either native observation's measured pin.

Final checking also preserves binding errors: a missing `_PERIODIC` definition
reports TLC code 2229, and a parameterized definition reports 2228. Both exit
151 in both semantic profiles; `--exit-codes native` selects 1. Binding and
symmetry-preparation model errors report the `binding` execution stage, including
when an empty property list takes the ordinary safety route. Engine model errors
retain the `capture` stage. A valid `_PERIODIC` hook
still refuses by name until hook execution is implemented.

## Rust APIs

Final analysis admits every retained bound-model predicate and its evaluator,
IR and name ownership before planning evaluates constants or calls native hooks.
Malformed public model metadata returns a planning refusal without graph capture.
Its diagnostic does not resolve an unadmitted location through the evaluator.
Valid configured wrappers keep their original native callbacks and typed errors.

Constant-domain predicate binders are inspected once, without enumerating their
values during temporal admission. This includes quantifiers, set maps and
filters, and nonrecursive function constructors. There is no predicate-binding
count limit in either value profile: for example, `[]<>(\A i \in 1..5000 :
i >= 1 /\ x)` no longer refuses solely because the domain has over 4,096
values. The predicate still evaluates its concrete domains and bodies in each
state; static analysis never supplies a sample value or proves a verdict.
Nested dependent domains and pure helper calls retain their lexical bindings.
Existing refusals for effects, unsupported action/temporal captures and opaque
operators remain, including behind empty domains. Actual temporal quantifiers, such as `\A i
\in S : []P(i)`, still expand concrete formulas and retain formula/storage
limits. Recursive functions retain their separate admission rules.

Finite predicate binders support structurally bounded deferred filters. Their
global configured-redirect preflight can resolve scalar constants supplied with
`=`, such as a bounded `LimitedSeq` helper reading `Max=1`, without calling the
helper or forcing unused arguments. Each inspected definition, redirect lookup,
formal/actual/binder metadata item and expression consumes the existing domain work budget; depth and materialized
component/output limits still apply, even with warm evaluator caches. This is
metadata inspection, not predicate evaluation or a liveness verdict.

Predicate and captured-context effect checks retain their existing refusals.
Definitely forbidden global redirect effects refuse before charged source
inspection, including at zero work, independently of redirect iteration order.
Constants supplied with `<-`, operator constants, deferred/non-scalar values,
recursive or higher-order redirects and unsupported lexical contexts do not
qualify for this extension. Two tiny filtered-WF models exercise success and a
genuine independently replayed violation. Coverage of original Quicksort and
other corpus models is not established by these controls.

Core-native Rust reports add `temporal::Outcome::NativeHolds`,
`NativeViolation` and `NativePrefixViolation`. The holding variant retains sealed
complete producer facts. Validated violation wrappers expose `word()` (selected
states, edges, properties and loop position), `evidence()`, `document()` and
`validation()`. Complete evidence and unfinished-prefix evidence stay distinct;
a prefix cannot prove Holds. The wrappers' constructors are private and are
published only after fresh typed and serialized validation. Existing reference
variants remain available. See [the Rust guide](rust-api.md) for the low-level
CSR/read interfaces.

`dive_api::temporal` is the Rust entry point (see
[the temporal API contract](../temporal-api.md)). `dive-liveness` exposes the
kernel it uses: planning (`plan`), the tableau (`formula`), the product and
lasso search (`product`, `search`), the coordinator (`analyze`), TLC's liveness
formula classifier for `--compat=tlc` (`tlc_plan::classify`, TLC errors 2213
and 2214), and the ordinary deterministic coordinator and its schedule
(`analyze::periodic`, `analyze::periodic::Schedule`).

## State-level recursive operators

Temporal properties and fairness actions can call pure top-level `RECURSIVE`
operators whose bodies and reached definitions are at most state level. For
example, `Sum(s) == IF s = <<>> THEN 0 ELSE Head(s) + Sum(Tail(s))`
can occur in `<>(Sum(s) = 0)` or a fair action's finite binder domain.
State-level quantifiers, `CHOOSE`, set/function constructors, recursive function
values and `LET` retain ordinary evaluator semantics. Planning does not unfold
the recursive call or enumerate its internal domains. The call's actual
arguments are checked in the caller's scope, including primed edge reads in
action arguments. Evaluation keeps its normal
resource limits and errors; admission does not prove termination.

Both profiles apply the same rule, matching TLC's evaluation-leaf treatment.
The independent trace validator evaluates the recursive atoms too: a reported
violation still requires a validated counterexample. This checked fixture
succeeds with 3 distinct and 7 generated states in both profiles:

```sh
dive check crates/dive-api/tests/fixtures/recursive-temporal/Rec.tla --config crates/dive-api/tests/fixtures/recursive-temporal/Rec.cfg --compat=tlc --fp 0 --seed 0 --workers 1 --output json
```

Action/temporal bodies, `ENABLED`, runtime effects, operator arguments,
configured operator bindings/overrides, dynamic INSTANCE substitution contexts
and local recursive operators keep explicit refusals. Value-bound constants
are supported. The proof checks semantic levels, effects and binding scope,
including reached and unused `LET` definitions; syntax and small actual
arguments cannot hide an unproved dependency. Evaluation errors inside an atom
retain the existing named temporal-atom refusal. The API controls in
`crates/dive-api/tests/recursive_temporal.rs` cover success, violations and
independent replay in both profiles.

## Finite local recursive functions

Pure state predicates and fairness actions can use a finite local recursive
function, such as allocator-style `PermSeqs(S)`. The recursive self, finite
domain, formal values and enclosing state/LET captures retain their actual
interpreter scopes. Planning inspects these without evaluating state-dependent
domains; execution obtains the concrete bounded contexts in each invocation.

```sh
dive check crates/dive-liveness/tests/fixtures/recursive-function/PermSeqsScope.tla --config crates/dive-liveness/tests/fixtures/recursive-function/PermSeqsScope-2-Good.cfg --compat=tlc --fp 0 --seed 0 --workers 1 --lncheck final --lasso short --output json
```

This tiny model requires a Schedule action under weak fairness and checks
`<>done`. Its two initial elements generate two permutation alternatives,
including duplicate successor work. Changing the configuration to
`PermSeqsScope-2-Bad.cfg` checks `[]<>(~done)` and returns a named temporal
violation with an independently validated periodic word.

The supported recursive body and its domains must be pure state expressions.
Primed/action/temporal captures, configured recursive actuals, higher-order or
opaque function forwarding, unsupported native effects, infinite/opaque domains
and exhausted bounds remain named refusals. Finite domains do not prove
termination: the interpreter retains its existing recursion errors and limits.
Unused LET definitions stay lazy.

Recursive source analysis conservatively treats self and formals as state-level
unknowns, even for a closed constant call. A recursive result therefore cannot
yet supply a temporal quantifier domain that planning must establish constant.
This is an analysis limitation, not evidence of an actual state dependency.

Independent fairness enabledness still needs complete finite successor covers
and checks the original relation. A recursive function supplies no cover by
itself. Empty domains cannot hide captured or outer primed reads, and missing
covers cannot become a Disabled result. No planner approval is passed to the
validator. Both value profiles share this bounded subset; existing defaults,
options, schemas and resource caps are unchanged.

Seven tiny pinned primary cases cover empty/one/two-element domains, holds,
fails and a direct control. Original allocator observations retain their earlier
producer identities and do not qualify this transferred source. No corpus gain
or roadmap completion is claimed. Fresh review, current-main qualification and
the full gate remain pending.

A configured first-order target whose body constructs a local recursive function
also refuses, including through ordinary helpers or lazy bindings. The same
restriction applies to used or unused recursive actual arguments and actual
recursive-function slots. Each temporal/finite/independent enabledness walk
inspects that configured source before evaluating domains or using cached values;
an empty recursive domain cannot hide a captured primed read or effect. Pure
unconfigured local recursive functions retain the scoped support described above.

Sequential graph capture distinguishes stateful checking from ordering needs.
Runtime string/name constructors and TLC action composition may appear together:
`--compat=tlc --action-composition=tlc --lncheck=final --lasso=short` retains their
single-worker encounter order and intermediate-state counts. A larger worker
request still reports the sequential fallback. Register/live-counter access,
randomness, I/O and cached lazy effects retain their existing refusals, and
speculative parallel capture remains excluded. Independent validation can still
refuse a counterexample that requires composition or newly created runtime names;
complete graph capture alone does not establish a validated violation. This
change does not extend checkpoint or DFID admission.

## Nested enabledness in Boolean guards

Bounded final analysis accepts pure `ENABLED` queries in ordered Boolean
`~`, `/\` and `\/` guards, including ordinary first-order definition bodies,
local LET expressions and bounded Boolean quantifiers. Queries can appear in
`Next`, temporal action predicates, and actions used by weak or strong fairness.
Each query quantifies a fresh successor from its actual current state; its inner
primed variables do not refer to the outer transition's target.

Fairness on a lasso is validated with the same `ENABLED`: `WF_v(A)` holds when
`<<A>>_v` occurs on the cycle or is disabled at some cycle state, and `SF_v(A)`
when it occurs or is disabled at every cycle state. An action whose guard reads
a primed variable, such as `ack' = IF x' = 1 THEN ~ack ELSE ack`, is decided
like any other (issue #321; corpus `fairness-replay`).

```sh
dive check crates/dive-liveness/tests/fixtures/nested-enabled/NestedEnabled.tla --config crates/dive-liveness/tests/fixtures/nested-enabled/hold.cfg --compat=tlc --workers 1 --fp 0 --seed 0 --lncheck final --lasso short --output json
```

The tiny model alternates `x=0` and `x=1`. `hold.cfg` returns success;
`fail.cfg` reports property `Fail`, a liveness violation with a validated periodic
word. Both explore 3 generated states, 2 distinct states and depth 2. The failure
word closes from `x=1` to `x=0` even though a reached inner query needs `x'=1`.
These two pinned primary references qualify this tiny subset, not the original
Elevator model or a corpus gain.

The trace validator checks the original `Next` and closing edge on the word and
decides each query with the core evaluator's `ENABLED`
([ADR 0103](../decisions/0103-validator-enabled-via-core-evaluator.md)). A
reached query error is reported even when a later guard is false; ordinary
Boolean short circuit leaves unreached errors unreached. Use the dedicated
`RECOMMENDED_STACK` thread contract when calling trace validation directly from
Rust.

First-order temporal expression arguments retain their existing formula route:
`Identity(p) == p; Identity([](ENABLED A))` forwards a temporal expression,
including its caller scope, rather than capturing an operator. Direct queries
still undergo the usual planning and independent enabledness checks. This does
not admit query-bearing operator captures, native/provider arguments or hidden
state-helper query words.

State-level mixtures of direct queries keep their existing temporal conversion
and native INSTANCE support. Replayed queries use the actual INSTANCE and lazy
scopes, including enabled and disabled correlated assignments. Direct and instantiated WF/SF words cover continuous
and intermittent starvation, disabled stutter and recurrent closing occurrences.
General callable and planner integration remains unfinished.

Periodic word validation evaluates queries hidden behind helper formals, such
as `Guard(p) == p /\ TRUE; Guard(ENABLED A)`, with the core evaluator. A
query-bearing action label is checked like any other label; the original Next
edge, closing edge and property claims are still required independently.

## INSTANCE query namespace controls

The separate `instance-temporal` corpus records ten tiny Boolean controls from
the pinned primary oracle before changing INSTANCE execution. These expose a
lexical distinction: `ENABLED I!A` applies an instance inside the query, whereas
an exported `I!Can` whose definition is `Can == ENABLED A` enters the query after
the instance bindings. They need not give the same answer.

The conflict action requires `y' = FALSE` and `z' = TRUE`, with both variables
instantiated by one caller variable. The direct and outside-query controls make
that conflict unsatisfiable. The exported-query controls instead violate `P`
in the initial state (TLC code 2107, exit 12), including when the caller has no
variables named `y` or `z`. These are initial invariant failures, not liveness
failures. A nested instance sends both substitutions through `q`; its query
returns false because the successor choices share that name. Flattening every
instance to the caller's root variable would lose these observed distinctions.

The guarded controls also separate an actual current-state value from a caller
variable with the same spelling as the imported declaration. Fresh substitutions
inside the query differ from captured bindings established before it. See
[the corpus guide](../conformance-corpora.md#instance-query-namespace-controls)
for the exact selection and generated oracle route.

The evaluator's ENABLED search represents its partial successor by canonical
variable name, including hidden names. It preserves the captured contexts and
branch cutoffs above. Ordinary graph states, fingerprints and trace variable
lists still contain only the root variables. Both semantic profiles use the
same query-name rules; no new flag or default divergence is introduced.

Sequential assignments and `UNCHANGED` checks reuse an unshared partial query
state. Retained alternatives, lazy computations and caller states keep their
own snapshots; query evaluation still follows the same branch and error order.

Rust callers can construct an actual partial query assignment with
`Evaluator::named_state`, using checked IR variable declaration IDs and values.
`StateRef::Named` retains it for scoped reads; `St::enabled_candidate`,
`enabled_current` and `primed_successor` preserve the complete state. An absent
name is unassigned and does not fall back to the old current state. Public
evaluation and context operations check both current and successor named-state
ownership before effects or retained captures. Named assignments from another
evaluator are refused. A root
projection of a query candidate is not an ordinary Next transition. See the
constructor's runnable rustdoc example and [ADR 0099](../decisions/0099-enabled-variable-name-states.md).

This is evaluator/checker query execution, not complete file-based INSTANCE
support. Independent INSTANCE query-word validation, general instantiated
temporal actions and refinement, callable substitution, and fairness word replay
remain required scope. Existing high-level planning/data guards and independent
replay refusals remain; the ten oracle cases do not themselves add native corpus
agreements. Composition with assigned hidden query names explicitly refuses
until its intermediate-state generator can retain them; existing root-only
composition keeps its original behavior. No new native agreement or independent
full-word validation is claimed.

## Lazy action sources in enabledness callbacks

The isolated `lazy-enabled-capture` oracle corpus records four controls for
[issue #312](https://github.com/Chelis-Lang/dive/issues/312). An exported
INSTANCE query assigns `a' = 1`, captures `LET Cut == a'`, and filters
`<<1, 2, 3>>` with `SelectSeq(..., LAMBDA e : e /= Cut)`. Requiring the result
to remain `<<1, 2, 3>>` makes that query false; requiring `<<2, 3>>` makes it
true. The outer invariant exposes the false query as an initial violation
of `Inv` (TLC 2107, exit 12). The true query completes with one distinct state,
two generated states and depth one (2193, exit 0). Constant and current-state
capture controls distinguish the creation context from the successor lookup.

Native checking matches all four in both profiles. Since the validator decides
`ENABLED` with the core evaluator (ADR 0103), replay accepts the genuine
initial-violation word of the primed disabled case and rejects the same word
for the primed enabled case, where `Inv` holds, through both the document and
the typed API, on the corpus's own `LET Cut == a'` specs
(`crates/dive-api/tests/callback_enabled.rs`). The corpus is
reported separately from the established 156-model population, and its
generated goldens retain their own source identity.

## A false prefix inside ENABLED

`ENABLED A` evaluates `A`'s conjuncts in order, as TLC does, and stops at a
false one. In the `enabled-false-prefix` corpus
([issue #320](https://github.com/Chelis-Lang/dive/issues/320)),
`A == FALSE /\ x' = 0 /\ D = D` compares a startup set filter
`D == {k \in SUBSET {0} : 1 \div 0 = 0}` that would divide by zero. The false
prefix leaves the equality unevaluated, so `[]<>ENABLED A` is violated
(2116, exit 13; 2 generated, 1 distinct, depth 1) with no division error, in
both profiles, and the reported lasso validates. Two controls with a `TRUE`
filter predicate, comparing `D` with itself and with an alias of it, hold.
`crates/dive-api/tests/enabled_false_prefix.rs` also rejects forged words (an
unreachable state, and the violation claimed for a holding control) and
checks that without the `FALSE` prefix the TLC profile reaches the division by
zero, which TLC reports with exit 255.

## Hidden-query lazy argument caching

The separate `lazy-enabled-cache` oracle corpus records six minimal controls for
[issue #313](https://github.com/Chelis-Lang/dive/issues/313). An exported INSTANCE
query assigns `b' = 42`, enumerates hidden `a' \in {1, 2}`, and calls
`Step(a')`. Its callback filters `<<1, 2, 3>>` using the lazy `Cut` argument.
Pinned default TLC reports disabledness when the final conjunct requires
`<<1, 3>>`, exposed as an initial `Inv` violation (2107, exit12). Requiring
`<<2, 3>>`, using only candidate2, inlining `a'` in the callback, or moving the
query to the root module each yields success (2193, exit0; one distinct, two
generated, depth one). Requiring the unchanged sequence remains disabled.

This result depends on observable primary caching behavior. Pinned
`LazyValue.getCachedValue` calls `TLCState.isSubset`, which compares only root
variable names; hidden `a` and `c` do not invalidate the retained lazy value.
The root collision `b` stays42 for both candidates. A paired research run with
`-Dtlc2.value.impl.LazyValue.off=true` changes only the remove2 control to
success. That JVM setting is diagnostic and supplies no oracle golden.
LazyValue captures its expression and lexical context; forcing supplies the
states and control flags, so adding creation-state snapshots is not a solution.

Native named-query lazy-cache checks compare the assigned root names, retaining
pinned TLC's observable behavior across hidden membership candidates. The six
controls check actual invariant evaluation in both profiles. Independent replay
compatibility remains open. Replay must evaluate original
source with its own bounded invocation state; checker caches or witnesses cannot
establish validity. The six inputs and generated goldens remain separate from
the established156-model population and do not increase its agreement count.

### Lazy query evaluation in replay

Replay evaluates lazy query sources with the core evaluator and its ordinary
caches, so a LET reached before a hidden membership choice keeps one binding
across the choices and one reached after it gets a binding per choice, as in
TLC. The isolated lazy-capture and lazy-cache controls do not change the
established 156-model population.

## Lazy query binding lifetime

The separate `lazy-query-lifetimes` oracle corpus records 24 primary controls
for the lexical lifetime issue investigated in [issue #313](https://github.com/Chelis-Lang/dive/issues/313).
An exported INSTANCE query assigns `b' = 42` and then enters a LET binding
`Cut == a'` before enumerating hidden `a' \in {1, 2}`. A SelectSeq callback
uses Cut to remove an element from `<<1, 2, 3>>`. Pinned TLC retains the same
lazy binding across both membership candidates: requiring `<<1, 3>>` leaves
the query disabled. Moving that LET after membership creates a binding for
each candidate and makes the same query enabled. Leading helper arguments,
delayed helper arguments, nested queries, sibling Boolean expressions and
false leading guards supply controls for this distinction.

The paired inverted invariants make the independent replay obligation explicit:
a genuine invariant-ending word must validate, and a fabricated ending must
fail. Recreating all LET contexts per complete candidate, or hoisting every
LET before all candidates, violates one of these controls. Source inspection
cannot supply the runtime context. The implementation must preserve each
reached lexical binding's lifetime across precisely the branches it encloses.

These primary-first fixtures and generated goldens establish the required
behavior before its implementation. They remain separate from the established
156-model population; no native or independent replay support is implied.


## Fairness with shared temporal memory

Both profiles separate pure Boolean recurrence (`[]<>B`) and persistence
(`<>[]B`) obligations from the temporal memory needed by a property. Multiple
strong-fairness alternatives share that memory. A finite prefix may violate a
persistence condition; its recurrent cycle must satisfy it. Recurrence may be
fulfilled on the closing transition, including an actual stuttering edge.
This reduces memory for models with many fairness conditions without changing
verdicts, state counts or counterexample validation.

The core engine may also reuse the completely evaluated native truth of each
behavior fairness condition (every `WF` and `SF` of the specification) shared
by configured properties. Each property keeps its required input
reads and current resource checks. This optional storage is bounded by the
smaller of 16 MiB and one 256th of the requested memory limit; unavailable storage
uses ordinary evaluation. It resets for each graph analysis and changes no
fairness or property semantics in either profile. Additive `fairness_reuse`
metrics distinguish reused truth from actual atom evaluations; they do not prove
completion, time saved or a hard whole-process memory ceiling.

The existing formula, graph, atom and product bounds still apply. An exhausted
bound remains an explicit non-verdict result; this change introduces no new
flag or profile divergence. Shared Boolean scratch consumes covered bytes;
condition inspections and scratch initialization share the materialization
work budget with the base buffers. Scratch bytes are reused across sequential
alternatives; setup work accumulates. Boolean evaluation during lasso search
remains outside the materialization work bound.
Run budgets interrupt decomposition, graph search
and SCC traversal through an explicit stopped path; acceptance truth is never
changed to signal budget exhaustion. Completed-level prefix analysis receives
the same armed budget on its capture thread. Polling occurs between bounded work
units; nested evaluator quantifier, CHOOSE and filter loops also observe the
same armed run budget, including atom evaluation and property attribution.
Value-only allocation, sorting and native operations may finish before the next
poll, so the time limit is not a hard real-time deadline. Structural binding,
evaluator construction, assumptions, post-binding symmetry preparation and
diagnostic reconstruction of an already deciding error remain outside this
interruption scope (#349, #379). Evaluator-backed fresh binding observes the
same control after structural construction. Fresh independent replay of native
temporal candidates inherits the same armed run control. Both the supplied word and its
native serialized round trip must validate before a violation is certified.
Interruption returns `stopped` at `trace-validation`, with the actual capture
completion flag and optional `resources.validation` progress; it publishes no
uncertified counterexample. Validation progress counts completed checks and
records a zero-based supplied state and check only when known.

Fresh validation rebinding observes this control during evaluator-backed work;
a stop retains the original capture, zero completed validation checks and no
supplied state position. Its structural construction and `ASSUME`, native
document encoding/decoding, and quiet value units remain uninterruptible (#349).
A supplied deferred value can retain a different evaluator that this fresh
control does not govern; the
serialized native pass reconstructs its values independently. General TLC
JSON interpretation/session reconstruction and standalone `validate-trace`
budgeting are separate follow-ups. The diagnostic exclusion never pauses
independent candidate certification. The checked
strong-fairness fixture in
`crates/dive-liveness/tests/fixtures/solutions/Solutions.tla` has 8 states and
25 generated states for three independent toggles. Its holding and failing
properties, including multiple-property attribution, are exercised in both
profiles by:

```sh
cargo test -p dive-liveness --test solutions
```

All operation reports use schema 3. A check's top-level `check_kind: "temporal"`
identifies its request/report family; it is not capture, completion or proof
evidence. Final/short requests that encounter a safety failure keep this family
and the actual incomplete-capture facts. Read the required `execution`, property,
options, physical status and independently validated witness fields together.
Ordinary safety-family reports have no temporal capture claims. See the
[report and trace versioning policy](reports-and-errors.md).

Liveness atom truth is stored as bit columns in the atom's actual state or
action domain, with one shared index of original outgoing edges. Unused false
rows for the other domain are not allocated. Atom identities and predicate
evaluation order are unchanged, including first source errors, parallel
transitions and closing edges. File checking uses run budgets rather than an
arbitrary graph-label cell ceiling. Reports distinguish that policy from the
unchanged attribution-row and word-truth cell ceilings under
`options.temporal_bounds.cell_limits`; they never present a shared 8,000,000
number as a graph limit that is no longer enforced. Packed layout arithmetic,
shape checks and fallible allocation remain. This is not an allocation-time
process-memory or universal wall-clock hard cap; individual bulk/quiet/native
units retain the interruption limitations described in the checking guide.

Existing Rust analysis APIs retain `GraphLabelPolicy::Cells(limits.atom_cells)`,
including caller-supplied zero and default-valued ceilings. New final and
periodic `*_with_policies` variants select graph admission explicitly, separate
from acceptance-buffer policy. `NoCellLimit` affects only graph labels;
attribution rows and word truth retain their original bounds and failure order.
See [ADR 0111](../decisions/0111-graph-label-policy-and-report-v3.md).

The `866c00ba` qualification retains the unchanged 156-model small tier:
149 agreements in the TLC profile, and 143 agreements plus six registered
expected divergences in the default profile. Both profiles retain seven named
refusals and have zero disagreements or timeouts. MCCRDT and
AllocatorImplementation are newly admitted; default-profile MCCRDT retains
D-022's generated-count difference and earns no agreement credit.

Six ordinary affected-model checks completed in both profiles under explicit
8 GiB ceilings and TLC-derived native time budgets: 165.8 seconds for MCCRDT,
230.82 seconds for Huang and 361.62 seconds for AllocatorImplementation, each
with 30 seconds of external cleanup grace. Every check exhausted capture and
completed temporal analysis. Their counts are:

| Model | Distinct | Generated (TLC / default) | Depth |
|---|---:|---:|---:|
| MCCRDT | 25,000 | 1,350,001 / 900,001 | 16 |
| Huang | 81,256 | 1,165,101 / 1,165,101 | 21 |
| AllocatorImplementation | 17,701 | 64,414 / 64,414 | 16 |

All eight normal instruction cells changed by less than 1%; geometric overhead
was 0.0113%, with identical verdicts, counts, depth, initial counts and final
queues. The [compact qualification](../survey/data/graph-label-policy-2026-10-22.json)
records actual source, input, binary and producer identities. These are bounded
capacity and instruction observations; historical full-tier and four-worker
measurements retain their own sources and counts.

A separate [historical CLI interoperability check](../survey/data/native31-cli-evidence-2026-10-22.json)
used source `eeae5dd1` and its own frozen binary on 31 exported Oyster fixtures.
Its 126 native commands cover 62 checks, 32 genuine trace validations and 32
forged-trace rejections. These observations retain that source's identity;
they do not qualify an Oyster report reader, typed consumer or new pilot pin.


## Symmetry with genuine temporal properties

`--symmetry-liveness=unreduced` is the dive profile's default (D-005). If the
actual configured roles contain a genuine temporal property and `SYMMETRY`,
the entire check uses full concrete state identities, including its initial and
safety checks. Counts are the actual unreduced counts. `SYMMETRY` remains in the
configuration and report; its reduction is not applied. The choice is made after
one ordinary binding and role split, without repeating startup output or effects.
A `PROPERTY` that binds entirely to initial/invariant/implied-action roles does
not trigger this rule, nor does fairness alone; ordinary safety symmetry and
its declaration/error ordering remain unchanged.
Core and Reference apply the same policy and report the same counts.

```sh
dive check conformance/corpus/probes/specifications/P4SymLive/P4SymLive.tla --workers 1 --seed 0 --fp 0 --output json
```

`--symmetry-liveness=refuse` explicitly refuses the temporal combination.
`--compat=tlc` selects `--symmetry-liveness=tlc`, the TLC-equivalent requested
setting. TLC's quotient liveness implementation is still unsupported, so it
refuses rather than silently using the unreduced mode. To combine TLC value
semantics with sound unreduced checking, select both explicitly:

```sh
dive check conformance/corpus/probes/specifications/P4SymLive/P4SymLive.tla --compat=tlc --symmetry-liveness=unreduced --workers 1 --seed 0 --fp 0 --output json
```

That named result-changing mode reports `tlc+overrides`. The flag wins over the
profile regardless of argument order. `dive tlc` accepts the same double-dash
extension. No new TLC `.cfg` keyword is introduced. The original configured
constant/role errors still precede checking; an ignored temporal symmetry
operator is not newly evaluated or prepared solely to validate its declaration.
A malformed declaration still receives the existing ordinary safety treatment
when safety reduction actually applies.

`options.symmetry_liveness` records the resolved request and
`options.symmetry_liveness_source` records `explicit` or `profile-default`.
The optional `symmetry_liveness` object is null before successful binding; a
bound unreduced run records its original `declared` name, `genuine_temporal`,
`effective: "unreduced"` and `scope: "whole-run"`. This explanatory metadata
proves neither exploration nor completion. Existing execution, counts, stopped
outcomes and actual requested/used worker fields remain authoritative. An
incomplete run cannot report success. Worker routing and all other capture
limits/refusals retain their existing behavior.

Periodic words still close by literal equality and independently check the
original transition relation, complete property set and full behavior fairness.
Fresh standalone validation receives the same explicit policy independently.
Use your saved native counterexample with the matching specification:

```text
dive validate-trace conformance/corpus/probes/specifications/P4SymLive/P4SymLive.tla --trace YOUR_NATIVE_TRACE.json --compat=tlc --symmetry-liveness=unreduced
```

Native `dive-trace` stays version 1. No quotient/permutation closing annotation
is accepted. VIEW remains unsupported, including the order-sensitive primary
controls in [the corpus guide](../conformance-corpora.md#liveness-quotient-primary-controls)
and issue #428. Strict TLC parity does not count an unreduced default result as
agreement with a reduced golden; only the exact registered D-005 difference can
be an expected divergence. The explicit no-SYMMETRY primary references remain a
separate validation population.
