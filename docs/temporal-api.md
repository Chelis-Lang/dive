# Temporal checking from Rust

Eligible ordinary `dive_api::check_file` requests use deterministic completed-level
checks in the dive profile and final analysis in the TLC profile.
`TemporalMode::Ordinary` remains the request default. The actual bound role split
retains an ordinary safety result on schema 1 or performs temporal analysis on
schema 2, with one startup binding and separate independent validation.
A deciding prefix returns `Outcome::PrefixViolation` with actual incomplete
capture evidence; only exhaustion can return `Outcome::Holds`. Both typed and
serialized words must pass fresh independent validation.
`CheckRequest::periodic` (`dive_api::PeriodicSchedule`) selects the completed
levels to check; the default checks once the graph has at least 2^16 states and
has doubled since the previous check.
See [ordinary dive-profile checking](user/temporal.md#ordinary-checking-in-the-dive-profile)
and [ordinary TLC-profile checking](user/temporal.md#ordinary-checking-in-the-tlc-profile)
for options, physical exits, reports and limits. Explicit `FinalShort` behavior
and its mixed-profile label retain the final-only schedule.

`dive_api::temporal::check_final` checks the supported pure temporal subset
once the safety graph is completely explored. It returns either a holding
result, an independently validated periodic counterexample, or the safety
engine's earlier stop. Planning and validation refusals return an error.
If the configuration has no `PROPERTY` entries, it runs the ordinary safety
checker directly and returns `Outcome::Safety`, including a successful exhausted
safety result. Binding and symmetry-preparation model errors retain the reported
`binding` stage; engine model errors and other safety stops retain `capture`.
`CheckResult::binding_model_error` records this actual origin without inferring it
from an error class or statistics. It does not bind twice or issue a temporal holding/capture proof;
temporal limits are unused on this route. All ordinary checker settings remain
in force.

Configured properties also use the ordinary engine when their actual bound roles
contain no implied temporal obligations and the request uses the narrow default
resident-memory route. This route binds once under the checker's startup host;
it retains all configured initial predicates, invariants and implied actions.
Behavior-specification fairness alone does not require temporal property checking.
The source/configuration eligibility inspection evaluates no expressions and
grants no purity or model-truth claim. The bound role split decides the route.

This initial scope requires profile-default evaluator and fingerprint settings,
fingerprint polynomial 0, resident fingerprint storage, no storage budgets or
directory overrides, no deadlock/sequential override and no chaos flag. Seed and
requested workers are retained. Randomness under VIEW or SYMMETRY stays on the
existing temporal route, as does every excluded option. Those routes retain
their original binding/evaluation error priority and named refusals. Genuine
implied temporal properties always continue to the temporal analyzer unchanged.
The independent temporal validator and the analyzer's no-temporal-properties
guard remain intact. The same role dispatch is used for eligible ordinary TLC-profile requests.

```rust
use std::sync::Arc;
use dive_api::temporal::{check_final, Outcome};
use dive_check::CheckOptions;
use dive_liveness::analyze::Limits;

let source = "---- MODULE M ----\nVARIABLE x\nInit == x = TRUE\nNext == x' = ~x\nSpec == Init /\\ [][Next]_x /\\ WF_x(Next)\nP == <>[]x\n====\n";
let analysis = dive_sema::analyze_source("M.tla", source, &dive_sema::Options::default());
let ir = Arc::new(dive_ir::lower(&analysis)?);
let config = dive_config::parse("SPECIFICATION Spec\nPROPERTY P\n")
    .into_config().ok_or("invalid config")?;
let report = check_final(&ir, &config, &CheckOptions::dive(17), Limits::default())?;

match report.outcome() {
    Outcome::Violation(v) => {
        println!("{}", v.document().to_json());
        assert_eq!(v.validation().validation, dive_trace::Validation::Valid);
    }
    Outcome::PrefixViolation(v) => {
        println!("{}", v.document().to_json());
        assert_eq!(v.validation().validation, dive_trace::Validation::Valid);
    }
    Outcome::Holds(evidence) => println!("{} distinct states", evidence.stats.distinct),
    Outcome::Safety(result) => println!("earlier safety result: {:?}", result.verdict),
}
```

The function binds a fresh model for each run and a separate model for trace
validation. Returned counterexamples name **all** configured temporal
properties they falsify. Their trace documents preserve exact values and
include the closing edge. Both the in-memory word and its serialized form must
pass validation before the API returns a violation.

The supported language includes pure boolean combinations, `[]`, `<>`, `~>`,
zero-arity definitions, pure constant/state-argument operators, finite constant-domain quantifiers with explicit sets
or intervals (including tuple bindings), direct `ENABLED`, and weak/strong fairness over supported
pure actions. Direct configured first-order top-level user operators retain
lazy scoped value/action arguments inside those pure atoms; their actual bodies
are checked independently by planning and trace replay. Configured native,
recursive, higher-order, temporal, value-overridden, substituted and captured
operator targets remain refused. Unused actuals are inspected for effects and
scope without forcing their values or binder domains. Other action/temporal
argument forwarding, higher-order calls other than the [SelectSeq subset](user/temporal.md), nonconstant/unbounded quantification, dynamic scopes, effects and
checker-register reads remain explicit refusals. The independent validator
needs a finite successor cover to prove some ENABLED/fairness cases; missing
covers return a validation refusal even if analysis finds a candidate.

Capture uses the ordered parallel safety engine and reports the actual pool
width and the requested width. Explicit sequential mode retains its reason.
Each fixed wave expands at most eight source states; fewer sources can use
fewer concurrent units than the pool width.
The limits bound formula entries, graph states/edges/generated work, labels and
product construction. Existing public `analyze_final*` and `check_final*` APIs
retain their entry/work counts and custom logical `acceptance_cells` limits.
They do not bound the preceding binding stage. Fingerprint mode retains the
safety engine's collision qualifications; exact mode stores full states.

The coordinator stores acceptance in private flat `u64` bit columns for promises
and recurrence truth, with optional shared edge offsets. Recurrence truth is
computed once from existing labels; search reads bits. Public owned `Product`,
`Labels`, `Acceptance` and lasso-search representations remain unchanged.

File-based ordinary eligible TLC requests and explicit final/short requests use
**8,000,000 acceptance-buffer bytes** and **32,000,000 materialization work units**,
in both semantic profiles. The byte default preserves the former nominal Boolean
payload allowance. It replaces only the logical acceptance-cell check; vertex,
edge and transition inspections retain their limits and ordering. JSON reports
`acceptance_buffer_bytes`, `acceptance_materialization_work` and
`total_memory_budget: false` under `options.temporal_bounds`.

Covered allocations are outer column headers, packed words, optional edge offsets
and maximum simultaneously live Boolean recurrence scratch. Layout arithmetic is
checked before fallible exact reservations; actual Vec capacities are charged
and no covered buffer subsequently grows. Allocator-provided excess capacity can
exist transiently before refusal. Allocator metadata/rounding, runtime/stack,
input labels/tableaux, graph capture, product construction/renumbering, search
and witness allocations are excluded. **This is not a process-memory ceiling.**
Materialization work counts metadata-node/column inspections, initialized
headers/words/scratch, promise assignments, offsets, action rows, edge projections
and bit writes, executed recurrence nodes and actually inspected child references. It does not count search work. Eager atom
order remains unchanged. Header/scratch costs can refuse even a tiny product;
exhaustion never means the temporal language is empty or a property holds.
TLC has no equivalent fixed acceptance-buffer bound.

Byte mode can store recurrence truth once per original behavior-source/edge
position and condition, then project through existing product maps. Parallel edge
positions and properties stay separate. It chooses the behavior domain only from
checked layout and work metadata before evaluation; a product-domain fallback
avoids expanding a sparse reachable product to a large behavior graph. It does
not evaluate a candidate mode and retry another. Word/offset storage for the
selected domain and all temporary Boolean scratch remain inside the same byte
allowance. Search performs constant-time indexing without predicate recomputation.
All state/action labels and their original admission/error checks are still
computed before this representation choice.

Rust callers opt into this policy through `temporal::check_final_with_policy`
or `analyze::analyze_final_with_policy`, passing
`product::AcceptancePolicy::Buffers { bytes, work }` or `compact_default()`.
All existing public entrypoints retain legacy cells even with default-valued
limits; no sentinel switches behavior. See [ADR 0098](decisions/0098-compact-acceptance-buffer-bounds.md)
for default rationale and exclusions. The additive API example is compiled by
rustdoc; the existing final/short CLI example below exercises the file default.

This API uses final checking and deterministic short-lasso selection. Passing
`CheckOptions::tlc` selects TLC's value/arithmetic/ordering semantics, but does
not select TLC's exact lasso, periodic schedule or evaluation-error timing.
Eligible ordinary `dive check` uses the profile schedules described above.
Excluded options and unsupported combinations retain named refusals.
TLC's exact lasso is not reproduced in any mode (ADR 0082); `--lasso tlc` is a
usage error. Under `--compat=tlc`, `dive_liveness::tlc_plan::classify`
reproduces TLC's liveness formula refusals (errors 2213 and 2214) for a later
stage to wire in.


Original configured PROPERTY formulas are retained for attribution. A universal
property may split into several checking roles, but its counterexample claim is
still the original configuration name; all roles are checked without guessing
which repeated internal name means which binding.

Pure `IF` formulas use their guard's truth at each position, including temporal
guards and nested conditionals. Action-level guards and temporal `CASE` remain
explicit refusals: the pinned TLC also rejects them during liveness planning.
A formula with an effectful branch is refused before planning executes it.
The shared `dive_liveness::plan::lower` planner also retains TLC's evaluation
error for a numeric temporal operand, such as `<>1`, in both profiles. It does
not turn this malformed formula into a feature refusal. The check report names
`evaluation-error`, retains code 2215 (`TLC_EXPECTED_VALUE`), and exits 255 under
`--compat=tlc` or 13 in the default profile, before state exploration.

## Command-line access

The same backend is available explicitly:

```sh
dive check Model.tla --lncheck final --lasso short --store exact --seed 17 --output json
```

Both flags are required for explicit selection. Eligible ordinary requests
select deterministic level checks in the dive profile and bounded final analysis
in the TLC profile without these flags; ordinary safety checks also retain their
existing command. With these flags, an empty property list or the bounded
configured-safety-role route above returns the ordinary safety result. Supported final
checking returns success or independently validated periodic counterexamples;
unsupported formulas, missing independent covers and limits return no verdict.
Safety violations found during exploration retain their full original report.

Explicit final requests and actual ordinary temporal analysis use JSON schema 2.
Ordinary safety results retain schema 1. A temporal
violation includes `properties`, an independently admitted tagged `counterexample.trace`, a 1-based
`counterexample.back_to`, and `counterexample.validation: "valid"`. The trace
document itself retains its own schema and 0-based back edge. Save the trace
field and replay it with `dive validate-trace` against the same spec/config.

`--compat=tlc` with these explicit overrides is labeled `tlc+overrides`. It
selects TLC's values and reports temporal violations with exit 13, but does
not claim TLC-exact lasso selection, finite-prefix stopping, temporal error
classes or periodic scheduling. The `tlc` class field stays null for temporal
holds/violations for explicit requests. With TLC exit conventions, temporal
violations exit 13 in either evaluator profile; the native exit convention uses
10. Actual ordinary TLC-profile final holds/violations carry verified TLC
success/temporal classes and complete capture counters through the ordinary
conformance adapter. Unsupported combinations retain named refusals.

Capture honors the requested worker pool for supported pure models, while
fixed eight-source waves bound scratch retention. Bounds are counts of retained entries,
not a hard byte-memory limit; they exclude preceding model binding.

Constant arguments are inspected for purity before evaluation and become value
bindings in each call's retained scope. For example, `Visit(i) == []<>(x=i)`
can be called as `Visit(2)` or from a constant-domain temporal quantifier.
Repeated calls with different values keep separate contexts. The planner and
independent word validator each check their own scoped arguments and work/depth
bounds. Hidden checker-register reads are refused;
parameterized actions whose ENABLED proof lacks an independent finite cover
remain validation refusals.

Independent ENABLED/fairness proofs now follow pure constant-argument action
calls and value-bound locals. Covers retain each call's scalar/tuple context,
and every witness is tested against the original relation. Missing primed
variable covers and arbitrary state/action arguments in the independent
finite-cover search remain refused. The bounded SelectSeq route also follows
ordinary unary pure <=State helpers through lazy source arguments, retaining
sequence and captured callback state dependencies in the required covers.
Explicit primed/action callback sources and higher-order calls outside the [SelectSeq subset](user/temporal.md) still refuse.
The `SfDisabled` probe specifically requires false ENABLED on its stuttering
cycle; a recurring action cannot stand in for this proof.

Pure state arguments now work too: `Live(p) == []<>p` can take `Live(x)`,
including nested forwarding, tuple arguments and state guards. State bindings
remain lazy and retain their caller context; priming inside the callee reads
the successor value. Planning checks instantiated levels before classifying
an atom or folding a constant. The independent word evaluator resolves those
contexts separately. Pure first-order temporal expression actuals also retain
that lazy formula route, including `Identity([](ENABLED A))`; direct queries
still require independent enabledness proofs. Query-bearing operator captures,
unsupported action contexts and higher-order operators outside the
[SelectSeq subset](user/temporal.md) remain named refusals.

Finite constant-domain quantifiers inside state/action predicates now retain
all instantiated body scopes during planning and independent word validation.
This includes quantified actions in WF/SF. Independent necessary covers combine
existential alternatives and universal constraints, then check the entire original
relation for each assignment. Nested domains may read already bound outer values;
state-dependent domains remain refused by temporal preflight, while independent
ENABLED can decide a finite domain using its concrete current state. Primed and
infinite domains, effects, missing covers and explicit budgets remain refusals.

Pure `LET` expressions retain their enclosing scope in both planning and
independent validation. Zero-arity values stay lazy, including captured state
and priming; local operators keep their parent frame. Independent covers follow
used LET values and prove action-call arguments are state-independent before
binding them as constants. Used effects/checker reads, missing covers and work
budgets still refuse. Six pinned `Lt*` models match final truth/counts in both
profiles/stores, with every returned periodic claim independently validated.

Pure INSTANCE value/variable bindings retain captured caller scopes in temporal
predicates, including nested substitutions, quantifiers and priming. General operator
substitutions retain named refusals. Independent finite-cover decisions follow
actual variable/constant substitutions and can prove disabledness when every
required successor name has a complete finite cover. Effects and checker reads are rejected before evaluation.

Pure finite constant-domain set maps, filters and function constructors retain
local bindings and captured state in temporal predicates. State-dependent
domains outside the bounded state-dependent subset retain explicit refusals.
The earlier blanket recursive-constructor refusal is superseded by the finite
local recursive-function subset below. See the `Bc*`
probes in `crates/dive-api/tests/fixtures/temporal-constructors/`.

Independent ENABLED covers also preserve finite set-map, filter and nonrecursive
function-constructor scopes. Concrete current-state domains are supported there,
while successor-dependent primed domains and missing finite covers remain refused.
This does not broaden the planner to state-dependent domains.

## Bounded parallel graph retention

Capture preserves every accepted successor occurrence, including already seen
states and duplicates in one wave. Original action labels and global successor
ordinals are retained; discovery IDs agree with the safety trace log. Roots
keep initial discovery order and each source ends with an implicit stutter,
including when generated transitions are filtered by constraints. The graph is
collected while expanding the safety search, without re-enumeration.

`dive_check::capture::Limits::successors_per_unit` is a separate scratch
admission limit, default **65,536 emitted successors per expanded source**.
Existing retained-state (1,000,000), retained-edge (8,000,000), and logical
generated-state (32,000,000) defaults are unchanged. Every source's actions
remain one unit, so the scratch boundary is independent of workers and chaos.
At most eight units are buffered: at the defaults, no more than 524,288
accepted edge records and 524,288 candidate payloads can await one commit.
Each unit stops before checking its first over-budget successor. The first
logical generated-budget violation takes precedence if the two coincide.
The bound counts callback admissions, not arbitrary evaluation operations;
source expansion can do expensive work before emitting a successor. Exact
keys, values, and output can have different sizes, so this is not a byte-memory
limit. Binding, atom evaluation and product construction have separate limits.

The library can explicitly raise the scratch cap for a high-fanout source;
there is no automatic increase. Its TLC counterpart is unbounded admission.
A scratch refusal never counts as a complete TLC-equivalent search. A bound
or earlier safety event retains only the ordered prefix's counters and evidence,
never an incomplete graph labeled complete. The cap applies to explicit
sequential capture too, making admission consistent at 1, 4 and 8 workers.
Eight-source waves are a conservative scaling boundary, not a claim of
64-way expansion scalability.

Controls compare complete graph order and bounded stopping against the inline
engine in both profiles/stores at 1/4/8 workers and chaos schedules. Thirteen
clean pinned liveness probes retain TLC final counts and produce identical,
independently validated short-word documents across those widths. Pure safety
error/constraint/invariant/implied-action/deadlock stopping and ordered output
also agree across waves. VIEW, symmetry, random operators and stateful checker
reads remain named graph-capture refusals; this increment does not establish
phase-L timing, TLC-exact lassos or strict integrated liveness conformance.

Runtime name constructors under TLC ordering use an explicit inline fallback
(issue #122), even for a requested pool of one with chaos enabled. Actual width
and reason are reported. Canonical default ordering remains parallel. This
admission exception does not allow random operators, live counters or I/O.
JSON `temporal_bounds` reports `successors_per_unit` and `max_expansion_units`
separately from logical graph limits and the physical worker pool.

## Finite recursive function contexts

Pure local RecFcn state expressions retain their actual self, evaluated domain,
formal bindings and lexical/state captures through bounded evaluator operations.
Analysis binds self/formals to non-runtime state-level unknowns and inspects the
whole body. Runtime scopes contain the actual owned LazyFcn and concrete formal
values; a plain context bundle retains source scope even for empty domains.
That bundle establishes no purity, termination, successor existence or truth.
Foreign/stale function, state and invocation contexts refuse.

The producer and independent temporal/EXISTS traversals inspect these separately.
EXISTS scans domains and captures before concrete enumeration and retains the
original necessary-cover rules. Primed/action/temporal captures, configured or
higher-order recursive forwarding and opaque/native effects remain refused.
Existing domain/depth/work caps and recursion errors remain in force. Body
inspection avoids repeated recursion by actual object identity, but ownership
checks and bounded body-context construction currently repeat at self references;
construction and projection also repeat bounded domain enumeration. This is
conservative extra work, not a one-materialization or performance claim.
See [the user guide](user/temporal.md#finite-local-recursive-functions) for the
checked tiny command and current qualification limits.

## Nested Boolean enabledness

The bounded subset includes nested `ENABLED` in ordered Boolean guards of
ordinary first-order definitions, lexical LET bodies and finite quantifiers.
Native atoms keep the original checker search. Independent temporal atoms,
nonstuttering action bodies and periodic `Next`/closing edges use a private
Boolean-spine coordinator with separate existential successor namespaces.
It derives complete inner finite covers, rechecks original correlations, and
shares expression/assignment/depth counters and the original pure invocation
across outer candidates and inner decisions. Inner primes never supply outer
covers. The action subscript and body use one invocation, with the existing
subscript-first stutter shortcircuit. Non-stuttering atoms evaluate the actual
subscript in both source states with the core evaluator; planning refuses
queries in the subscript.

Only a query-free state prefix can decide false before candidate construction;
later query guards remain neutral during cover derivation. Missing/infinite
covers, exhausted bounds and empty nested covers requiring source-order evidence
refuse rather than certifying Disabled. Data/provider/higher-order, configured
and primed query placements retain their narrower support boundaries. Actual
INSTANCE/lazy scopes are traversed by the independent name-addressed search. No planner certificate,
checker enumerator dependency or new public result type is involved. The existing
trace caller-stack contract and all numeric caps remain unchanged. Two primary
references and their native/primary words are covered by the tests under
`fixtures/nested-enabled`; original Elevator sufficiency remains unmeasured.

State-level Boolean mixtures of direct queries retain their established temporal
lowering routes, including native query atoms under constant INSTANCE bindings.
The private Boolean coordinator retains whole action-level guards. Independent
queries retain hidden successor names and the original lazy/cutoff scope; a
root witness projection is not an ordinary Next edge. Sixteen pinned direct and
instantiated WF/SF cases cover continuous/intermittent starvation, disabled
stutter and recurrent closing occurrences, with genuine typed/serialized
violations and forged initial-state controls. This does not establish general
callable INSTANCE or planner integration.

Every independent core fallback is checked in its actual scope with the existing
finite data-position guard. Raw effects do not authorize a hidden query. The periodic Word interpreter
still conservatively refuses queries hidden behind state-valued helper formals, such
as `Guard(p) == p /\ TRUE; Guard(ENABLED A)`, before core evaluation. Native
exhaustive checking can still establish Holds for these formulas. Supported
action-valued lazy formal guards retain their independent Boolean paths. Periodic optional labels
use the same check; refusal skips only that metadata evaluation and adds its
existing note. Original Next and property checks remain required.


### State-level recursive operator leaves

Temporal planning and independent periodic-word validation share the core
`Evaluator::recursive_leaf` proof for pure top-level `RECURSIVE` definitions
whose bodies and reached definitions are at most state level. The proof checks
levels, fixed-point effects and binding scope, rather than excluding binder or
value-constructor syntax. Calls retain their caller's argument context and are
computed by the ordinary evaluator. Recursive action/temporal bodies,
`ENABLED`, effects, operator arguments and dynamic/configured operator bindings
retain explicit refusals. Value-bound constants are permitted. Admission does
not prove termination or suppress ordinary evaluation errors/resource limits.
See [the temporal user guide](user/temporal.md#state-level-recursive-operators)
for the checked CLI fixture and both-profile success/validated-violation tests.
