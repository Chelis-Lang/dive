# Temporal checking from Rust

Eligible ordinary `dive_api::check_file` requests use deterministic completed-level
checks in the dive profile and final analysis in the TLC profile.
`TemporalMode::Ordinary` remains the request default. The actual bound role split
retains an ordinary safety result or performs temporal analysis, with one
startup binding and separate independent validation. Both use report schema 3;
`check_kind` distinguishes the safety/temporal report family without claiming
that checking ran or that capture completed.
A deciding prefix returns `Outcome::NativePrefixViolation` with actual
incomplete capture evidence; only exhaustion can return `Outcome::NativeHolds`. Both typed and
serialized words must pass fresh independent validation.
`CheckRequest::periodic` (`dive_api::PeriodicSchedule`) selects the completed
levels to check; the default checks once the graph has at least 2^16 states and
has doubled since the previous check.
See [ordinary dive-profile checking](user/temporal.md#ordinary-checking-in-the-dive-profile)
and [ordinary TLC-profile checking](user/temporal.md#ordinary-checking-in-the-tlc-profile)
for options, physical exits, reports and limits. Core implements no bounded
final-only mode, so `TemporalMode` has only `Ordinary`; the CLI's
`--lncheck final --lasso short` refuses temporal properties by name.

Temporal checks from Rust run on Core through `dive_api::check_file` with a
`CheckRequest`. Under `--compat=tlc` (`compat_tlc: true`) the supported pure
temporal subset is checked once the safety graph is completely explored. `CheckReport::temporal_result` returns the temporal report: either a
holding result over the exhausted graph, an independently validated periodic
counterexample, or the safety engine's earlier stop. `CheckReport::result`
returns a safety result, and `CheckReport::temporal_failure` the typed failure
of a temporal check that stopped without a verdict, such as a resource stop.
Planning and validation failures whose report Core does not build are named
refusals (`unsupported`, exit 3).
If the configuration has no `PROPERTY` entries, or its properties bind only to
safety roles, the ordinary safety checker runs and its complete result is
returned. Binding and symmetry-preparation model errors retain the reported
`binding` stage; engine model errors and other safety stops retain `capture`.
`CheckResult::binding_model_error` records this actual origin without inferring
it from an error class or statistics. Behavior-specification fairness alone does
not require temporal property checking. Genuine implied temporal properties
always continue to the temporal analyzer.

The reference engine's in-memory `dive_api::temporal::check_final*` functions,
which took a lowered module and caller-set analysis limits and policies, were
removed with that engine (#791).

```rust
use dive_api::{CheckRequest, temporal::Outcome};

let request = CheckRequest {
    file: "M.tla".into(),
    compat_tlc: true,
    seed: Some(17),
    ..CheckRequest::default()
};
let report = dive_api::check_file(&request)?;
match report.temporal_result().map(|t| t.outcome()) {
    Some(Outcome::NativeViolation(v)) => {
        println!("{}", v.document().to_json());
        assert_eq!(v.validation().validation, dive_trace::Validation::Valid);
    }
    Some(Outcome::NativeHolds(evidence)) => {
        println!("{} distinct states", evidence.stats().distinct);
    }
    _ => println!("{}", report.to_json()),
}
# Ok::<(), dive_api::ApiError>(())
```

Each run binds a fresh model for checking and a separate model for trace
validation. Returned counterexamples name **all** configured temporal
properties they falsify. Their trace documents preserve exact values and
include the closing edge. Both the in-memory word and its serialized form must
pass validation before the API returns a violation.

The two native validation passes register the same armed run budget on their
fresh evaluator. Interruption is handled outside the validator's verdict type
and is classified only by the matching owning budget: exhaustion becomes a
stopped error at `Stage::Validation` / `trace-validation`, and unavailable memory
sampling remains `DIVE-R0002`. The failure retains actual capture statistics,
its completion flag, and optional `Failure::validation_progress()`; it contains
no uncertified word. Unknown state/check positions remain absent. A previously
fully certified witness retains the existing terminal-sampling priority.
Native document shape errors retain their independent validation class.
The same capability is installed during evaluator-backed fresh rebinding, after
structural configuration checks and evaluator construction. It returns the
original typed interruption instead of swallowing a constant-definition error or
reclassifying it as a binding refusal. A stop in this stage retains the real
original capture, has zero completed validation checks and no supplied state
position, and cannot certify a candidate. `Failure::binding_output()` preserves
that binding invocation's output; validation rebinding output is separate from
the original capture's execution output.

A complete supported startup conversion can return
`AnalysisError::StartupTautology` before assumptions or exploration. It means
TLC's actual combined fairness and bound temporal-role formula normalized to
literal `FALSE`, not that a general tautology prover accepted the property.
`Failure::binding_output()` retains earlier output; there is no capture, state
row, word or `Holds`. Ordinary reports use evaluation error 2253 and TLC status
77 with null exploration counts. Effectful/unknown conversions remain refused.
Original configured claims remain the legacy continuation's separate inputs;
no expression or scope-cycle equality authorizes substituting actual roles for
those claims. This entails extra startup preparation on continuing runs and
requires ordinary-runtime overhead qualification; it adds no per-state hook.

This does not control structural binding/evaluator construction, `ASSUME`,
post-binding symmetry preparation, encoding/decoding, quiet value units, or
deferred providers owned by a different evaluator. General TLC session/scoring
and standalone trace-validation budgets remain follow-ups. The additive
`dive_model::Recipe::bind_controlled` supports callers which reconstruct their
own fresh startup runtime; legacy recipe/session calls retain their behavior.

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
product construction. They do not bound the preceding binding stage. Fingerprint mode retains the
safety engine's collision qualifications; exact mode stores full states.

The coordinator stores base acceptance in private flat `u64` bit columns for
promises and recurrence truth, with optional shared edge offsets. Those
recurrence columns are computed once from existing labels; search reads their
bits. Extracted Boolean persistence (`<>[]B`) and recurrence (`[]<>B`) conditions
share the base product and evaluate Boolean truth over the original state/action
labels during lasso search. They do not re-evaluate specification predicates.
Public owned `Product`, `Labels`, `Acceptance` and lasso-search representations
remain unchanged.
The coordinator's product adjacency uses flat destination and original-edge-position
arrays retaining their discovery allocations. Canonical source rows use ranges
into those arrays, and destination IDs are remapped in place. Pair lookup remains
a `BTreeMap`, and search
still constructs its recurrent-edge graph using the existing algorithm. All
vertex, edge, inspection and acceptance limits retain their units and values.
The layout change does not by itself establish a process-memory saving; flat
buffer spare capacity and simultaneous discovery offsets/final row ranges also
consume memory.
A [four-case synthetic capacity study](internal/survey/data/product-adjacency-capacity-2026-10-22.json)
records the buffers at six construction checkpoints. Nonempty target and
original-edge arrays retain their allocation, length and capacity throughout
canonicalization. These observations exclude the lookup index, acceptance,
search and allocator internals; they establish neither CoffeeCan capacity nor
a whole-process memory reduction.
A separate [normal-input instruction screen](internal/survey/data/product-adjacency-overhead-2026-10-22.json)
compares the original base and CSR candidate on four reduced models in both
profiles. Every matched cell stays within 1% overhead, with identical verdicts,
counts and execution facts. The geometric mean is 0.0104% more instructions.
These historical source pins establish the normal-input admission screen; they
do not measure current-main throughput or qualify a product-limit removal.

File-based ordinary eligible TLC requests and explicit final/short requests use
**8,000,000 acceptance-buffer bytes** and **32,000,000 materialization work units**,
in both semantic profiles. The byte default preserves the former nominal Boolean
payload allowance. It replaces only the logical acceptance-cell check; vertex,
edge and transition inspections retain their limits and ordering. JSON reports
`acceptance_buffer_bytes`, `acceptance_materialization_work` and
`total_memory_budget: false` under `options.temporal_bounds`.

Covered allocations are the promise columns' outer header buffer and packed
words, and the maximum simultaneously live Boolean scratch of one cycle-condition
alternative. Layout arithmetic is checked before fallible exact reservations;
actual Vec capacities are charged and no covered buffer subsequently grows.
Allocator-provided excess capacity can exist transiently before refusal.
Allocator metadata/rounding, runtime/stack, input labels/tableaux, graph capture,
product construction/renumbering, search and witness allocations are excluded.
**This is not a process-memory ceiling.** Materialization work counts promise
assignments, initialized headers, words and scratch entries, and inspected cycle
conditions. It does not count search work. Eager atom order remains unchanged.
Header/scratch costs can refuse even a tiny product; exhaustion never means the
temporal language is empty or a property holds. TLC has no equivalent fixed
acceptance-buffer bound.

The product stores only promise columns, which give constant-time truth lookup.
Extracted `[]<>` and `<>[]` conditions are evaluated during the search on the
original behavior source and edge position, so parallel edge positions and
properties stay separate. They reuse maximum-sized Boolean scratch; its live
bytes share the acceptance byte ceiling. Condition inspections and scratch
initialization consume the remaining materialization work cumulatively across
sequential alternatives. Boolean evaluation during search remains outside that
work bound. No specification predicate is recomputed.
All state/action labels and their selected graph admission/error checks are
computed before the promise columns are built.

Graph labels have a separate `analyze::GraphLabelPolicy`. Existing final,
periodic and acceptance-policy APIs retain `Cells(limits.atom_cells)`, even when
the caller supplies exactly 8,000,000. New `*_with_policies` variants accept
`Cells(n)` or `NoCellLimit` explicitly. The latter removes only obsolete logical
`(states + retained edges) * atoms` admission; checked packed layout, shape,
fallible allocation and configured run budgets remain. Attribution rows and
word-truth workspace still use the supplied `Limits::atom_cells` unchanged.
File final and periodic checks select `NoCellLimit` in both profiles. Returned
API reports and failures expose their actual admitted `Configuration`; it does
not claim a bound was exercised. Major-3 wire reports serialize these owner
settings instead of the former ambiguous scalar. See
[ADR 0111](decisions/0111-graph-label-policy-and-report-v3.md).

Rust callers of `analyze::periodic::external::GraphCheck::prepare` pass
`product::AcceptancePolicy::Buffers { bytes, work }` or `compact_default()`;
no sentinel switches behavior. See [ADR 0098](decisions/0098-compact-acceptance-buffer-bounds.md)
for default rationale and exclusions. The CLI example below exercises the file
default.

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

The same backend runs from the command line in the TLC profile:

```sh
dive check Model.tla --compat=tlc --seed 17 --output json
```

Eligible ordinary requests select deterministic level checks in the dive
profile and bounded final analysis in the TLC profile; ordinary safety checks
also keep their existing command. `--lncheck final --lasso short` refuses
temporal properties by name (exit 3) and leaves a model without them unchanged.
Supported checking returns success or independently validated periodic
counterexamples; unsupported formulas, missing independent covers and limits
return no verdict.
Safety violations found during exploration retain their full original report.

All operation reports use JSON schema 3. Temporal runs use the `temporal`
report family; ordinary safety reports use the `safety` family and carry no
temporal capture claim. A temporal
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

`dive_runtime::capture::Limits::successors_per_unit` is a separate scratch
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


### Whole-run unreduced symmetry policy

`CheckOptions.symmetry_liveness` and `BindOptions.symmetry_liveness` transport
D-005 explicitly; the dive constructors select `Unreduced`, and TLC constructors
select `Tlc` (its quotient temporal path still refuses). Existing struct-literal
callers must supply the new typed field or use a constructor/update expression;
these Rust APIs are not yet the stable published library interface.
`Model::symmetry_liveness` exposes an immutable post-role-binding resolution,
retaining the original declaration. Only a genuine temporal obligation can
select whole-run unreduced identities. All safety-only symmetry behavior is
unchanged. Capture verifies the requested/bound policy and refuses any prepared
reduction or VIEW; no quotient graph can enter the unreduced path.

Independent trace binding shares only this model-level typed policy. The legacy
`Recipe::new` conservatively retains periodic-symmetry refusal; select
`Recipe::with_symmetry_liveness` explicitly to reconstruct unreduced validation
inputs. `TraceOptions` and the file API do this independently from their own
resolved request. Literal closing and full fairness remain mandatory; neither
model metadata nor an external report provides a validation certificate.
The optional report-3 summary and requested fields are additive. Old producers
without them remain readable, but cannot authorize the exact D-005 count tuple.


### Reachable product vertex ceilings

`product::Limits::vertices` remains a finite numeric maximum of reachable
behavior-state/tableau-node pairs. All existing APIs enforce the actual supplied
value, including zero, exact boundaries and the explicit default 1,000,000.
The public `Limits::default()` value remains unchanged. There is no sentinel or
comparison against a former default that disables a caller's guard.

The private file configuration selects `usize::MAX` in both profiles, for final
and deterministic completed-level analysis. `Builder::visit` still performs
checked cardinality addition and compares it with this actual scalar before
pair admission. This removes the arbitrary file-request threshold while keeping
machine representability, layout checks and all other owner bounds. Report
major 3 still emits the actual numeric `product_vertices`: the owner, units,
type and meaning are unchanged. A literal unbounded/null policy would require a
separate report-contract decision; this number is not reported as infinity.

The range-row representation retains one target and one original-edge buffer;
it does not introduce a Cartesian allocation. Existing infallible pair/index,
canonicalization and recurrent/search allocations remain possible host failure
points. Sampled process memory is not an allocation-time hard cap, and no new
resident-size projection or universal interruption guarantee is claimed.
