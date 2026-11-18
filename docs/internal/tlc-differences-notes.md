# TLC differences: maintainer notes

Development notes, progress records and evidence moved out of the user-facing
[TLC differences page](../tlc-differences.md). This is maintainer working
material, not user documentation.

### Historical component observations

The minimal `LvPeriodic` probe stopped at 418,795, 167,402 and 162,021 distinct
states in three pinned-TLC runs; all found the same violation with different
time-dependent stopping counts (ADR 0017 §6).

### Zero-duration ratio edge case (controlled component evidence)

The former controller's 296
controlled observations and the 196-row runtime-ratio selection with three
assertion negatives are component evidence, not evidence for this implementation.
Its zero-duration ratio sample recorded NaN bits `7ff8000000000000`. A historical
native diagnostic retained 12,337 requests and reached 3,929,089 generated
occurrences before ResourceLimit. That failed experiment remains uncredited;
no controller, ratio policy, proof or historical performance result is imported
into the completed-level implementation.

## Maintaining this page

This page is the narrative companion to the terse registry in [`docs/divergences.md`](../divergences.md). The rules for agents and contributors are in `AGENTS.md`. In short:

- Every registry row has a `D-NNN` entry here, added or updated in the same PR.
- IDs are stable and never reused. An entry that is withdrawn stays, marked *withdrawn*, with the reason.
- Every entry includes:
  - a minimal example;
  - what TLC does, measured on the pinned oracle, with numbers;
  - why we think it is wrong or suboptimal;
  - dive's default;
  - the switch that restores TLC's behaviour;
  - the status and the evidence.
- When an entry's status changes (planned → implemented → default), update it in the same PR as the code.
- Write for users who know TLA+ and TLC but not dive's internals. Link to ADRs for the design detail rather than repeating it.

### D-013 candidate: more temporal constructors

TLC accepts the syntax of the following properties, then refuses them during
liveness planning:

```tla
VARIABLE x
Init == x = FALSE
Next == x' = ~x
P == [](IF x' # x THEN <>x ELSE <>~x)
Q == [](CASE TRUE -> <>x [] x -> <>~x [] OTHER -> FALSE)
```

On the pinned oracle, P stops with 2214 (wrong formula format), Q with 2213
(cannot handle formula), both exit 255 before exploration. Pure state guards
and even a temporal guard such as `IF []<>x THEN <>[]x ELSE <>[]~x` do work.
The captures live in `crates/dive-api/tests/fixtures/temporal-conditionals/`.

Dive implements the working IF forms in its bounded final Rust API and
independent validator. It keeps the other forms as named planning refusals in
both profiles. Supporting more constructors could remove an implementation
restriction, but is only a recorded candidate. No new default or compatibility
claim is adopted here.

**D-013 implementation update:** `dive check --lncheck final --lasso short`
now exposes the bounded final backend. Its periodic counterexamples pass both
in-memory and lossless-document validation before return. Schema-2 reports
include the complete false-property set and actual worker width. With
`--compat=tlc`, the report says `tlc+overrides`; it does not claim TLC-exact
lasso choice, temporal error classes, finite-prefix stopping or periodic checks.
Ordinary strict checking remains a named temporal refusal. See
[command-line access](../temporal-api.md#command-line-access).


D-013 acceptance resource policy: file-based eligible ordinary/final checking
uses compact acceptance-buffer bytes (8,000,000) and independent materialization
work (32,000,000) in both profiles. Existing public custom-limit APIs retain
legacy cells; additive APIs select the new policy explicitly. TLC has no fixed
counterpart, so exhaustion is a resource refusal, never a verdict. Covered
allocations and excluded owners are listed in [the temporal guide](../temporal-api.md).
This does not bound process memory or change vertex/edge/domain/transition limits.


**D-013 supported-language update:** pure constant-argument operators now work
in final/short checking, including nested calls, tuple arguments and calls from
finite temporal quantifiers. The five `temporal-calls` probes match the pinned
oracle's final verdict truth and state counts in both profiles/stores. Each
returned lasso validates independently. Broader lazy/higher-order arguments,
ordinary scheduled timing and strict CLI integration remain pending; bounded final-only pinned selection is available in the separate Rust facade. No new default or
compatibility claim is introduced.

**D-013 validation update:** independent finite ENABLED covers now preserve
constant scalar/tuple arguments through action calls. A pinned 36-state probe
checks three ENABLED identities, and a fair stuttering counterexample requires
disabledness rather than a recurring action certificate. Three additional
`scoped-fairness` probes match final truth/counts in both profiles/stores.
Missing covers remain validation refusals; this expands the validated subset
without claiming TLC-exact temporal selection or error timing.

**D-013 scoped-language update:** final/short checking now accepts pure state
arguments such as `Live(x)` for `Live(p) == []<>p`. Caller scopes remain lazy
through nested calls, guards, tuple values and priming. Seven pinned `Sp*` probes
match final truth/counts in both profiles/stores and every returned periodic
claim validates independently. SANY's rejection of bare eventual actions is
also retained as a negative control. Action/temporal and higher-order arguments
still refuse; this adds supported semantics without a new default or exact
TLC temporal-conformance claim.

**D-013 quantified-action update:** final/short checking now accepts finite
constant-domain predicates such as `\E p \in Processes : Move(p)` inside
fairness. Six pinned probes match final truth/counts in both profiles/stores,
and every returned periodic claim validates independently. A 36-state probe
checks 108 independent ENABLED decisions, including contradictory universal
constraints and an empty existential domain. Missing covers and exhausted budgets
still refuse. This expands supported semantics without a new default or exact
TLC temporal-conformance claim.

**D-013 LET update:** final/short checking now accepts local values and
operators in temporal predicates. Captured state stays lazy, including nested
calls and priming. Six pinned `Lt*` models match final truth/counts in both
profiles/stores, with every returned periodic claim independently validated.
A fair stuttering ending requires independent disabledness. Effects, checker
reads, missing covers and explicit budgets remain named refusals; strict TLC
selection and temporal stopping remain pending.

**D-013 INSTANCE update:** final/short checking follows pure value and variable
substitutions through nested modules without freezing the current state. Six
pinned `Is*` models match final truth/counts in both profiles/stores, with every
periodic claim independently validated. Substituting a state expression for a
constant remains rejected as in SANY. Effectful/checker-dependent bindings,
operator substitutions and unproven substituted disabledness remain named
refusals. This is supported-language coverage, not TLC-exact temporal selection
or stopping. Evidence: `crates/dive-api/tests/fixtures/temporal-substitutions/`.

**D-013 finite-data update:** temporal predicates can use set maps, set filters
and function constructors over finite constant domains. For example:

```tla
P == []<>({IF b THEN x ELSE FALSE : b \in BOOLEAN} = BOOLEAN)
```

With a Boolean variable initially TRUE and a toggling Next action, TLC reports
a violation: infinite stuttering at FALSE fails P. It generates 3 states,
retains 2 distinct states and reaches depth 2. Both dive profiles/stores match
this result and independently validate the returned periodic witness. Local
bindings and captured state remain separate through nested constructors and
function arguments. Empty and bounded state-dependent predicate domains are
supported; effects and explicit resource limits remain named refusals. Evidence:
`crates/dive-api/tests/fixtures/temporal-constructors/`, seven supported pinned
truth/count controls and exhaustive short Boolean words. Exact TLC temporal
selection, stopping and parallel capture remain pending.

### Symbolic function-set enumeration

The pinned oracle prints `[[Nat -> {}] -> {1}]` as `{<<>>}` and succeeds
(`EmptySymbolicFunction`, two generated states, one distinct state, depth 1).
The value layer now recognizes empty function ranges before enumerating their
domains, and recognizes the singleton empty function without enumerating its
range. Size, equality, hashing, fingerprints and printing agree with the finite
representation. A singleton range over a non-enumerable domain still refuses
enumeration; diagnostic printing retains a symbolic fallback instead of
recursively retrying the failed expansion.
Evidence: `crates/dive-value/tests/fixtures/printing/`.

### Independent disabledness with primed domains

A prime outside a quantifier also primes its domain. For example, with
`S = {}` and `t = FALSE`, the action `(\E b \in S : t=b)'` has a concrete
successor `S = {TRUE}`, `t = TRUE`. Enumerating the current empty domain
would miss that successor and could incorrectly validate a deadlock. The
independent validator now refuses successor-dependent primed domains with
`DIVE-T0020`; constant primed domains still require complete finite covers
for all body reads. Ordinary current-state empty domains remain decidable.
This repairs dive's own soundness defect (#94), rather than introducing a
different TLC default. Evidence: `crates/dive-api/tests/primed_covers.rs`,
including concrete witnesses, nested quantifiers and an end-to-end deadlock
claim; the primed-domain regression fails against the old domain scan.

**D-013 independent-constructor update:** finite set maps, filters and function
constructors retain lexical scopes during independent ENABLED checks. Three
pinned fair-stuttering violations and one success match final truth/counts in
both profiles/stores. The eight-state `EnabledConstructorCover` pins 48
independent decisions. Current-state domains are read at that state; primed
domains and missing finite successor covers refuse rather than certify a false
deadlock. Complete TLC temporal stopping, planning/selection and parallel
graph capture remain open.

**D-008 frontend evidence update:** a malformed PlusCal comment can make SANY
reject or crash after TLA+ grammar parsing succeeds. For example, the existing
`PcalEndWithoutBegin.tla` case exits 255 with an array index of -1, while the
pinned root grammar accepts it. `dive analyze` reports `DIVE-E0009` for the
translation-validation problem; `dive parse` measures the TLA+ grammar only.
Deep campaigns now retain both facts separately, with no acceptance inferred
from a generic crash. See `docs/internal/frontend-campaign.md` and issue #98.
**D-013 state-domain update:** final/short predicates may quantify or construct
values over a finite domain that changes with the current state. For example:

```tla
P == []<>({b : b \in IF x THEN {TRUE} ELSE {}} = {TRUE})
```

Starting at FALSE with a toggling Next, pinned TLC reports a liveness violation
(exit 13): infinite stuttering at FALSE leaves the domain empty. It generates
3 states, retains 2 and reaches depth 2. Both dive profiles/stores match truth
and counts; a fresh independent validator certifies the periodic witness.
Planning never picks a sample element or freezes the domain. Concrete evaluation
checks finite-entry bounds at every state and closing edge. Evidence is in
`crates/dive-api/tests/fixtures/temporal-state-domains/` (eight pinned models)
and exhaustive short-word tests. State-dependent expansion of temporal
quantifiers, infinite/oversized domains and effects still refuse by name.
Ordinary strict temporal checking and exact TLC selection/timing remain pending.

**D-013 finite-choice update:** temporal predicates can use bounded CHOOSE,
including definitions such as:

```tla
Min(S) == CHOOSE b \in S : \A c \in S : b <= c
P == []<>(Min(IF x THEN {1,2} ELSE {3,4}) = 1)
```

With x initially FALSE and a toggling Next, pinned TLC reports a liveness
violation (exit 13), generating 3 states, retaining 2 and reaching depth 2.
Both dive profiles/stores match final truth/counts and independently validate
the periodic witness. Each choice here is unique; ambiguous choice keeps the
existing canonical default and TLC restoration switch from D-006. Finite
domains retain actual state context even when a body does not read its binder.
Empty or unsatisfied choices raise errors rather than inventing a value;
infinite/oversized domains, effects and checker reads remain named refusals.
Evidence: `crates/dive-api/tests/fixtures/temporal-choose/`, five pinned models
and exhaustive short-word/context tests. This does not establish strict TLC
temporal selection, scheduling or parallel graph capture.

**D-013 action-helper update:** a pure value helper may inspect the actual
current and next state, including through nested and captured arguments:

```tla
Less(a,b) == a < b
P == []<><<Less(x,x')>>_x
```

For x initially 0 and Next alternating 0 and 1, pinned TLC reports a liveness
violation (exit 13): a stuttering behavior never has a Less step. It generates
3 states, retains 2 and reaches depth 2. Both dive profiles/stores match final
truth/counts and independently validate the periodic claim. Sequence-prefix
helpers such as `IsStrictPrefix(log, log')` use the same retained caller edge.
Evidence: `crates/dive-api/tests/fixtures/temporal-action-args/`, four pinned
models and exhaustive short-word/closing-edge controls. Temporal helpers still
refuse action-level arguments; temporal-valued/higher-order forwarding and
effects remain unsupported. Full TLC temporal selection and timing are pending.

**D-013 fairness storage update:** final/short checking handles five independently
fair processes without materializing every combination of recurring choices:

```tla
Init == x = [p \in 1..5 |-> FALSE]
Flip(p) == x' = [x EXCEPT ![p] = ~@]
Next == \E p \in 1..5 : Flip(p)
Spec == Init /\ [][Next]_x /\ (\A p \in 1..5 : WF_x(Flip(p)))
P == \A p \in 1..5 : []<>(x[p])
```

Pinned TLC finds no violation: 161 generated states, 32 distinct states, depth 6.
Removing the fifth process's fairness violates P (exit 13) at the same counts.
Dive's explicit final/short mode matches both results in both profiles/stores;
the weak-fairness probes fit a 64-node tableau bound that unfactored construction
exceeds. Every reported violation is independently validated. A witness mutated
to stutter forever is rejected specifically for violating fairness.

Recurring conditions are checked on the infinite cycle, including its closing
edge. They do not constrain the finite prefix. Reachable products preserve state,
parallel-edge and witness tie-break order; resource defaults are unchanged.
Evidence: `crates/dive-api/tests/fixtures/fairness-factoring/`, independent
short-word/path-closure controls and the generated LM1 final/short study.
DiningPhilosophers now matches TLC's final truth/counts; MCDistributedReplicatedLog
remains refused for VIEW capture. Actual capture is still one worker. Full TLC
temporal planning, selection and timing remain pending; `--compat=tlc` alone
retains the strict temporal refusal until those are proven.


### D-013 implementation progress: concrete liveness successors

ADR 0029 adds real concrete successor collection with internally derived TLC
fingerprints, first representatives, grow-before-duplicate slot order and one
logical multiplier history. Null-tableau OOS action bins precede state bins;
initial tableau consistency is separately projected. Actual diagnostic execution
is inline (one worker); wider requests are refused honestly. Every receipt says
`graph_insertion=false`, and no receipt is proof evidence.

The complete `conformance/probes/tlc-phase-l/` selections retain 55 structural
controls, 35 actual synthetic LiveCheck controls, 51 bound model rows and 20 clean
TLC runs. Completed field/error projections, invalid syntax, effects/introspection/
state-IF refusals and tableau graph-pending rows are accounted separately. The
prior planning IF mismatch (#147) and body-first nonstuttering mismatch (#149)
remain available; conservative diagnostic refusal is not a match. Independent
trace controls reject forged Next/closing edges and claims. Previous corpus
reports are immutable and receive no new verdict credit.

F-020/F-021/F-022 remain partial/audit-required and F-023 missing. #140 stays
open: graph/product insertion, actual parallel commits, additional consistency/
error ordering, SCC/lasso selection and scheduling must still be implemented.

### D-013 implementation progress: actual graph commits

The bounded graph stage now keeps original generated successor values even when
an equal state was already stored, then writes concrete graph nodes with TLC's
slot order, bin labels and one logical capacity history. Source expansion really
uses 1, 3, 4 or 8 workers in the tested runs; ordered predicate/graph commits
report their own single-worker execution. Twelve completed pinned graph
projections match their defined fields. All fifteen selected models, errors,
tableau refusal, clean TLC outcomes and original failed adapter attempts remain
available in `conformance/probes/tlc-phase-l-graph/`.

This does not enable strict liveness yet. Complete safety exploration currently
precedes temporal evaluation, so its counts are not TLC's temporal stopping
prefix. Multiple OOS, dynamic tableau revisitation and done-state re-enumeration,
periodic checking and exact SCC/lasso selection are still required. Opaque
captured providers are refused before execution; per-source speculative quotas
can reject a skewed level conservatively. Independent validation still decides
a concrete periodic word and its claims, rather than trusting graph metadata.
No graph result or diagnostic receipt claims a model proof.

D-013 planning/error-order update (#147): `[]<>(IF x = 1 THEN 1 \div 0 = 0
ELSE x = 2)` raises TLC code2179 before any exploration, even if a reachable
state would not select the erroneous branch. The TLC-shaped native planner now
preserves that error and the reference's distinct user-call/boolean fallbacks.
No alternate default is adopted: independent formula evaluation keeps its
existing semantics, and strict integrated liveness remains unsupported. Complete
24 current controls, original18 and the native false completion are retained in
`conformance/probes/state-if-planning/`. That capture predates the bounded
pure state-IF admission in explicit final TLC mode (#168);
ordinary periodic scheduling and upstream-refused temporal-argument IF forwarding remain refused.
This is a native limitation repair, not family certification.

The [bounded process-fairness increment](../pluscal-translation.md) adds weak/strong
declarations, label guards and explicit pinned fairness options. Translation
artifacts remain nonproof results. Unproved/overlapping process domains, procedure fairness, renaming
and unproved domain normalization are still named refusals; full-family
certification remains pending.

**D-008 PlusCal fairness formatter observation:** the pinned translator prints
an **87-column** fairness line with its default **78-column** setting for:

```tla
--algorithm A {
  variables g=0;
  fair process (P \in{1,2}) variables x=0;
  { L: + x:=1; E: - g:=x; }
}
```

`ProcessFairness.singleLineWidth` omits the sole quantifier-prefix line and
conjunction separator. The retained `FairPositive_C_set_weak_both` primary output
shows the 87-column line. Current native translation preserves its measured
junction grouping in both profiles; full width/layout improvement is pending.
No alternate formatting default is silently adopted. Long-label controls also
compare full operator CST, since moving TLA junction indentation can change
expression structure. See [the fairness evidence](../pluscal-translation.md).

**D-008 public backend admission update (#157):** malformed public Rust IR
metadata now refuses before ordinary checking or checkpoint request handling.
Graph capture likewise checks arena/name and predicate/action ownership before
model effects. Error reports omit an invalid variable mapping instead of
panicking while reporting another refusal. Original panic source/hash evidence
and native controls are retained in `conformance/probes/public-ir-admission/`.
This is a native API safety repair, not new TLC semantic certification.

The [multiple finite-process increment](../pluscal-translation.md) now binds
each admitted block to its own storage, labels and fairness context. Overlapping
IDs, name rewrites and cross-process local references remain explicit refusals;
translation artifacts still make no model-proof claim.

The [bounded native macro increment](../pluscal-translation.md) reproduces lexical
substitution rather than inventing capture avoidance. The pinned translator can
return zero while producing TLA+ that SANY rejects (including `Set(b, 3)` whose
actual `b` is undeclared). Native raw generation retains those comparisons only
as diagnostic evidence; public translation rejects the invalid enclosing module
in both profiles and makes no proof claim (D-008). General binder/procedure/name
rewrites remain named refusals. Source PlusCal strings deliberately preserve the
pinned printable-ASCII boundary; no wider Unicode-string default is adopted.



The bounded sequential PlusCal procedure path now generates saved parameter/local
frames and call/return continuations. Its 84-row selection keeps four primary
SANY-invalid modules uncredited. Thirty generated invariant witnesses reproduce
TLC exploration but strict independent replay refuses runtime string encounter
order (#124); default/canonical replay is separately qualified evidence, not
TLC-profile certification. See [the procedure guide](../pluscal-translation.md).

The earlier nonrecursive PlusCal graph study admitted multiple disjoint procedures
and nonrecursive nested calls. Procedure fallthrough still generates `pc="Error"`
and may deadlock, matching the reference. That study refused cycles (including unused ones), name
rewriting, process-owned stacks and unproved procedure-label/macro interactions.
The current recursive path is described below; its original study is unchanged. Strict stack witness replay remains pending under

The bounded sequential PlusCal graph path now admits multiple disjoint procedures
and nonrecursive nested calls. Procedure fallthrough still generates `pc="Error"`
and may deadlock, matching the reference. Cycles (including unused ones), name
rewriting, process-owned stacks and unproved procedure-label/macro interactions
remain honest named refusals. Strict stack witness replay remains pending under
#124; separately qualified canonical replay confers no TLC-profile certification.
See [procedure graph use and limits](../pluscal-translation.md).

### Recursive zero-parameter tail call: invalid output (D-008 candidate)

The pinned translator reports success for this PlusCal source:

```tla
--algorithm A {
  variable x = 0;
  procedure First() { F: call First(); return; };
  { L: call First(); E: skip; }
}
```

Its generated `F` action contains a blank conjunction between `pc="F"` and
`pc'="F"`. `pcal.trans` exits 0 and writes the module; SANY rejects the next
`/\` with a parse error. The retained raw source and both stages are
`crates/dive-pluscal/recursive-procedure-acceptance/fixtures/Recursive_C_self_tail_no_params.*`.
The reference's same-procedure call/return path adds an empty assignment when
there are no parameters; this is not a valid translated model or a termination
proof. The native recursive increment retains this as diagnostic evidence
and public admission rejects invalid modules in both profiles. A repaired-output
option is not adopted silently. Full translation parity remains pending.

The disjoint sequential procedure path now admits ordinary self/mutual recursion
and reference same-procedure tail-call normalization without unrolling calls.
See [recursive procedure use and limits](../pluscal-translation.md).
Strict stack-witness namespace proof remains pending; bounded unbounded-model
stops never claim exhaustive search or termination. Process/scoped name handling
and full translator parity remain pending.

### D-013 progress: ordered temporal execution

The bounded Rust execution path now commits real multiple-OOS/tableau graphs in
source order, with initial consistency before initial invariants and immediate
done-state re-enumeration. Fifteen completed pinned graph-field cases match at
1/3/4/8 workers; all 26 cases and eight clean TLC stopping controls are retained
in `conformance/probes/tlc-phase-execution/`. Logical prefix counters, speculative
batch emissions and recursive enumeration work remain separate. Independent
validation rejects altered words, closing edges and incomplete property claims.

This remains intermediate: accepting-sink witnesses, exact TLC SCC/lasso choice,
initial error replay and periodic checking are not certified. Ordinary strict
liveness stays refused; no defaults or fixed budgets increase. F-020/F-021/F-022
remain partial/audit-required, F-023 missing and #140 open. Previous complete
studies keep their original outcomes and receive no new corpus verdict credit.

### D-013 progress: pinned final-only SCC and witness selection

The bounded `dive_api::tlc_temporal` Rust mode now constructs its own ordered
graph and reproduces TLC's deciding component, greedy postfix and fingerprint-
bucket closure. It retains configured claims separately from derived role names.
At 1/3/4/8 workers, 19 completed pinned controls match their SCC/witness fields;
all 30 controls, including 11 uncredited failures, remain available in
`conformance/probes/tlc-final-check/`. Fresh independent validation accepts the
supported original words and rejects changed Init, Next/closing edges, loop
targets and incomplete claims. A 4,096-graph independent census checks acceptance
and selected path labels.

This is an explicit final-only TLC/fingerprint library mode, not ordinary strict
CLI parity or periodic-schedule support. Exact-store temporal proof is refused:
a fingerprint graph must not silently merge concrete states while claiming exact
identity. Accepting-sink reconstruction, initial error replay, symmetry/VIEW/ALIAS,
periodic hooks, unsupported scopes/effects and fixed resource bounds remain named
limitations. Holds results retain collision qualifications; backend candidates
never carry their own validation certificate. F-019 through F-022 remain partial/
audit-required, F-023 missing and #140 open. Old broad studies retain their original
outcomes; no default or ordinary strict conformance credit changes.

See final-only Rust API guide for examples, resolved schedule,
actual evidence and refusal boundaries. The API admits startup sources before
binding creates a host; effectful ASSUMEs, unused top-level constants, config
replacement candidates and hooks return named refusal with zero used workers.
Standard-module classification does not bypass this admission. Explicit sequential
requests/fallback execution remain refused until their observer path is certified.


**Explicit final/TLC integration update (withdrawn 2026-10-12: the `--lasso tlc` mode, the `dive_api::tlc_temporal` API and the `dive-final-tlc` harness mode were removed with the rest of the TLC-internal liveness emulation):** `dive check --compat=tlc --lncheck final
--lasso tlc` now exposes the independently certified bounded final-only path.
Actual counts/worker/stage metadata and original property identities are retained,
including TLC's stuttering marker. This does not enable periodic checking or lift
exact-store, accepting-sink, recovery, effect or resource refusals. The separate
`dive-final-tlc` harness mode requires a final-schedule golden and fresh independent
replay; ordinary schema-1 results and earlier studies are unchanged. Configuration
alias targets are admitted through actual module contexts before any startup
binding effect. See the user guide.

### D-013 progress: explicit final-mode safety-only routing

A freshly bound model with no implied temporal obligations now routes through
bounded ordinary safety checking in explicit final/TLC mode. This checks real
invariants, initial/action properties, constraints and supported deadlocks;
absence of a temporal obligation is not a fabricated NoOOS/product result.
Finite witnesses require fresh independent in-memory and JSON replay. Reports
name `bounded-safety-only`, preserve actual stopping counts and worker facts,
and state `temporal_certificate: false` and `product_constructed: false`.

The same entry/work ceilings stay active. They bound retained graph data, not
aggregate memory: a graph-state limit of three on `x'=x+1` refuses while the
engine truthfully records the deciding fourth inserted state. It never reports
success or completed capture. Constraint/property combinations whose stopping
order the independent validator has not proven remain explicitly refused before
pool work. No ordinary schedule, liveness-family or broad corpus credit follows.
Original all-refusal studies and the constant `Next == FALSE` binding rejection
are preserved. See the safety-only user contract
and `conformance/probes/tlc-final-safety` for clean paired evidence.

D-013 conditional-admission progress (#168): pure state IF atoms are admitted
through the corrected shared planner and independent final-word replay. This
removes the older duplicate syntax refusal without changing conversion/error
order, resource defaults or ordinary periodic scheduling. All original refusal
and failed-capture histories remain available. Effects, opaque bindings and
unverified errors remain explicit; no whole-family certification is claimed.
See conditional user guide, ADR 0036 and
`conformance/probes/tlc-state-if/`.

### Temporal IF identity repair (dive limitation, #170)

Pure state conditionals are available in the bounded final-only path. Temporal
conditionals still refuse there: the source-level shared-guard tag repair now
matches selected exact normalized/tableau fields, but execution and witness
admission remain staged. See the maintained repair note.
No new result-changing default or ordinary periodic parity is claimed.

The proven direct temporal-IF subset now runs in explicit final/TLC mode after
shared-object tag repair, exact particle/edge checks and fresh independent
in-memory/JSON witness replay. The unchanged full selected controls retain
upstream 2213/2214 errors and source-prefix 2116/exit-12 cases as named refusals,
without matching-error credit. Compound guards and INSTANCE claim/role identity
are separately checked. Existing arena/work ceilings and whole-arena forwarding
admission remain; no aggregate memory bound or periodic-family claim is added.
See the user guide and
`conformance/probes/tlc-temporal-if-final/`.


**PlusCal process procedure status (D-008):** one unfair finite process block
can now call disjoint procedures in both syntaxes, including recursive and
self-tail transfers. Slots and stack are owned by the process ID, while explicit
source `stack` expressions remain as written by the reference. Invalid upstream
artifacts, including empty self-tail conjunctions, are still rejected in both
profiles. Multiple blocks with procedures, scoped rewriting/fairness and strict
runtime-namespace witnesses remain named limitations; canonical witnesses do
not certify TLC replay. See [process procedure use](../pluscal-translation.md).

### Procedure names can collide with generated ProcSet (D-008 candidate)

This minimal C-style PlusCal body uses a parameter named `ProcSet`:

```tla
--algorithm A {
  variable x = 0;
  procedure First(ProcSet = 0) { F: x := ProcSet; R: return; };
  process (Worker = 1) { L: call First(2); E: skip; }
}
```

The pinned translator exits **0**, declaring variable `ProcSet` and also emitting
operator `ProcSet == {1}`. Clean SANY with `-error-codes` exits **4**, reporting
the multiply-defined symbol. Naming the procedure itself `ProcSet` similarly
produces a zero-exit translation with conflicting zero/one-argument operators.
Both C/P versions are retained: four invalid primary artifacts in
`crates/dive-pluscal/process-procedure-acceptance/names/primary`.

Successful generation is not a usable artifact. Public native translation in
both profiles rejects the enclosing module and claims neither acceptance nor
proof. These rows earn diagnostic credit only; no automatic rename, corrected
output option or new default has been adopted. Status: measured upstream
artifact-generation defect, with source/raw/SANY and CLI/API rejection controls.

**Later shared process procedure status (D-008):** multiple disjoint finite
process blocks can now call the same procedures without sharing their frames.
Slots and stack initialize once over all IDs; procedure actions execute for the
calling ID. This lifts the earlier single-block limitation, while scoped
renaming, cross-process locals and process-declared fairness remain refused.
Both profiles reject invalid upstream artifacts; canonical witness checks
still do not certify strict namespace replay. See
[shared procedure use](../pluscal-translation.md) for executable C/P and
Rust examples and the exact supported/refused contract.

### Procedure local self in a constant-ID caller (D-008 candidate)

```tla
--algorithm A {
  variable x = 0;
  procedure First(a = 0) variable u = self; { F: x := u; R: return; };
  process (One = 1) { L: call First(1); E: skip; }
  process (Two = 2) { M: call First(2); N: skip; }
}
```

Pinned translation exits **0**. Its Init binds `self` when initializing the
procedure slot, but the constant-ID call actions initialize `u` with a free
`self` instead of the caller's literal ID. Clean SANY exits **4** with two
unknown-operator errors. Both C/P source/raw artifacts remain retained as
`MultipleProcedure_*_local_self` in the shared procedure study. Native public
admission rejects the enclosing artifacts in both profiles. No corrected
initialization default or strict artifact credit has been adopted.

The P-syntax empty-process control also retains a zero-exit translation with
an empty process operator and a `"null"` initial program-counter label; SANY
rejects the generated module. Native normalization refuses the empty body by
name. These observations explain why successful translation is not enough for
an admitted usable module; they are not silently repaired.

**D-008 process-owned fairness update:** finite, disjoint process callers now
receive WF/SF clauses for their reachable procedures in the pinned call order.
`Entry: +` adds a strong-fairness clause under a weak caller; `Entry: -` guards
that procedure's caller-owned pc location. Original `Entry+:` controls remain
source grammar rejections; the corrected syntax was captured separately before
implementation. `-nof` suppresses all fairness for process algorithms. Native
formula construction and reachability have explicit bounds, returning a resource
failure rather than silently dropping a clause. Generation does not prove
liveness or remove strict #124 namespace replay restrictions. See
[fair procedure use](../pluscal-translation.md) for runnable C/P and Rust
examples and the complete supported/refused contract.


### Current identity metadata limitation and evidence

The final-only JSON and Rust API expose three distinct temporal identity vectors:
configured properties, original expanded Actions and derived role labels. A
conjunction can have one configured `P`, two Action names `P`, and two source
location labels. Those labels are not interchangeable. This is a matching
metadata contract, not a new divergence or ordinary liveness certification.
Exported temporal-formal INSTANCE cases still refuse when the pinned reference
fails before producing that projection. See the identity guide.

## Concrete successor representation boundary

The bounded final TLC path uses explicit stored Enum/Raw set elements and stored
function entries before fingerprints/comparison, including normalized Raw sets.
This is a native representation admission change, not a changed TLC semantic
default. Cached provider/deferred sets remain refused. Existing8m/depth256
Session bounds and separate50k/128 context bounds are unchanged. Pinned
`LiveCheck.addNextState`/`SetOfStates` concrete-value behavior is retained; see
stored-state guide and
paired source/evidence.
Ordinary schedules and whole-family parity remain partial.

### Bounded native Append forwarding limitation

Explicit final TLC checking now admits pure fixed-arity `Append` operator actuals
with both expression scopes preserved. The pinned 33-case sequence selection
retains all Tail/SubSeq refusals: their string overloads can intern names, so
tuple-only examples do not authorize those natives. Ordinary scheduling and
deciding type-error parity remain pending. See the Append guide.

## Bounded partial temporal SCC diagnostics (#185)

Native ordered source commits now have a separate bounded partial-check API.
It skips unfinished records, retains original root/EA/OOS/PEM order and candidate
identities, and charges projection/SCC/witness work against the existing shared
graph quota. Successful partial checks never mean temporal `Holds`; ordinary
strict checking remains refused until real clock/quiescence/error/stop-count
contracts are implemented. The complete32 explicit-prefix projections and8
repeated forced original checks are diagnostics, not ordinary schedule runs.
See the user/API guide and
complete controls.
No new result-changing default or family certification is claimed.

## Incomplete VIEW diagnostic limits (#188)

The #188 investigation also retains incomplete VIEW states under
`conformance/probes/view-concrete-normalization/incomplete-primary/`. Leaving
`y` unassigned while storing `{<<1>>, [a |-> 1,b |-> 1]}` makes Init diagnostic
printing throw EC1000/exit255; incomplete Next reports EC2109/exit75. The
well-formed set counterpart reports EC2102/exit255 for Init and EC2109/exit75
for Next. Mixed-value diagnostic printing remains unproved: rejecting an
incomplete state before keying does not establish its exact printer/error
counterpart, and no malformed-input-wide conformance is claimed.

The bounded periodic backend diagnostic exposes source-qualified graph growth, actual pool-prefix facts and first-candidate stopping. It still refuses to claim ordinary periodic scheduling or Holds. TLC uses pointer-table buckets and records processed OOS sizes even after a found violation; four measured thrown-check faults retain old snapshots. These reference quirks and native limitations are preserved in the complete controls, without native IO-error agreement credit.

D-008's macro checksum continuation separates exact expanded AST CRC derivation
from admitted TLA+ output and checked models. A minimal bounded macro
`macro Set(v,a) begin v:=a; end macro;` can now derive Full checksums for
`Set(x,1+2)`. Multiline actuals and substituted junction columns remain named
refusals. The pinned translator accepts algorithm comments ending with arbitrary
prose after `end algorithm;`; native embedded selection now stops only at a completely parsed algorithm end.
The two original C/P failures remain archived; standalone parsing stays EOF strict.
See [the macro checksum guide](../pluscal-translation.md) for evidence and
the unchanged fair-header-only checksum limitation.

The macro continuation also retains dive's original `Lock` enclosing-module
ordering failure: generated declarations were appended after definitions using
`lock`, `pc` and `ProcSet`. Inserting them immediately after the algorithm
comment restores the pinned declaration order; genuine unknown-name controls
still reject. This is a native defect correction, not a new TLC divergence.

The same study keeps two original standalone inline-definition false
acceptances and four block-comment-tail false acceptances. Pinned translation
rejects non-line-comment text after the algorithm comment on its closing line;
native admission now enforces that exact wrapper boundary. Invalid formfeed
reference artifacts remain uncredited. No new default grammar relaxation is
introduced.


### D-008 embedded marker ownership repair

A minimal comment containing `--fair algorithm A` (assigning `x := 1`) and a
complete `--algorithm B` (assigning `x := 2`) on a later line exposed a native
selector defect: the pinned translator emitted `x' = 1`, while source5 emitted
`x' = 2` and admitted the module. Exact selector bytes in shipped main `fe35c73f5`
match the original base; the source5 runtime evidence is not retagged as a main
runtime. Original 16-case controls retain four raw and eight public wrong-answer
rows. The repair uses pinned line order, preferring unfair only when both markers
share the same line, in both profiles. Expanded split/tab/space and malformed
controls preserve real reference rejection boundaries. See the
macro checksum study.
Artifact admission and CRC equality still never establish model correctness.


The native clock/dispatch lifecycle diagnostic now samples its own clock and holds real pool dispatch authority during commit-boundary checks. Mandatory final SCCs run at actual safety EOF. TLC’s asynchronous progress-thread/queue timing remains unimplemented: no ordinary verdict or Holds is claimed, and PERIODIC/checkpoint transactions stay refused. Selected96 real queue controls are narrow lifecycle evidence, not model scheduling proof.

D-008 native PlusCal DEFINE limitation: ordinary global helper declarations now
have a bounded [native translation and Full-checksum path](../pluscal-translation.md).
For `F == x; x := 1; y := F`, the pinned translator emits `y' = F`, so the helper
reads the pre-state `x`. Dive preserves this in both profiles. A definition using
a process-local variable remains before that variable's generated declaration,
and the invalid artifact is rejected rather than reordered into apparent success.
General recursion, scoped renaming and procedure definitions remain unfinished.

## Native periodic hook boundary

The pure supplied-expression API now composes a bounded hook at real native dispatch pauses, with original FALSE comparison and evaluation errors. It does not bind configured `_PERIODIC` and does not enable ordinary checking. The complete59 actual-TLC controls retain even setup failures: an `_PERIODIC` constant name throws ClassCastException rather than a stable binding diagnostic. Native configured binding remains a named refusal, so no fabricated error-class agreement is claimed. Checkpoint/host effects, ordinary schedule and stopping-count parity remain pending; no result default changes.

D-008 LOCAL/function DEFINE limitation narrowed: bounded global `LOCAL F == x`
and `F[a \in 0..x] == a+1` now preserve the reference declaration and pre-state
lookup rather than substituting an earlier assignment's new value. The
[LOCAL/function guide](../pluscal-translation.md) describes supported scope,
CLI/API examples and remaining recursive/action/temporal/name/procedure refusals.
Reference-invalid artifacts and fixture filename failures stay visible without
admission or proof credit. Neither profile silently repairs the reference output.
### Configuration-owned periodic diagnostic limitation (#197)

`_PERIODIC Hook` is now selectable through a separate bounded Rust factory, preserving the original pre-constant body. Ordinary CLI scheduling remains unsupported. A constant-valued periodic role throws a real `ClassCastException` in the pin; native diagnostic binding refuses it by name rather than inventing a TLC code. The complete 26-row selection plus 12 startup/precedence rows show unused `PrintT` startup before name/arity failures and symbolic constant startup work that TLC permits. The guarded native factory refuses these before output under its unchanged inspection/startup policies, so those rows are refusals, not TLC-error agreement. User contract and executable example; [ADR 0057](../decisions/0086-rescue-reset.md). No ordinary-schedule or family certification is granted.

### Stored trace transport (#190): withdrawn

The opt-in version-2 trace format (`--trace-format representation-v2`) was
removed on 2026-10-12. It carried TLC's internal stored representation of
values, which is not part of a trace's meaning. dive writes and reads trace
format version 1 only, and the flag is gone. The 36 original and 108 altered
shape failures of version 1 described above remain.

**Historical checksum-era scope (superseded by issue #241):** D-008 bounded native PlusCal procedure-checksum continuation: one explicitly
labelled sequential procedure can now use Full editing. The pinned original
header AST retains top-level calls but shared If/With/Either branch vectors are
mutated by Explode; these six new shapes remain named checksum refusals. Two
retained local_update controls are primary missing-label errors while raw native
lowering admits them; neither receives compatible artifact credit here. Read
[the procedure checksum guide](../pluscal-translation.md) for exact scope,
D-020 fair-header CRC limits and strict #124 pending replay.

### Progress-request/worker stopping boundary (#201)

The producer-clock Rust diagnostic now has a real expansion claim barrier and joined dispatch pause. It still cannot use an uncommitted expansion as a written temporal graph. Complete33 actual pinned method controls retain successful resume, early temporal/hook stops, evaluation errors, real EOF and failed Init. At widths1/4/8, early temporal2116 and false-hook2104 return with workers alive; later probe cleanup joins are not original runTLC behavior. A worker division-by-zero sets2179 with keepCallStack=true while runTLC itself returns0, so that return is not model-check success. These are measured method boundaries, not newly declared TLC bugs or ordinary native parity. Ordinary scheduling/count/checkpoint/model-proof authority remains unfinished.

### Native tuple-function storage repair (#195)

This was a dive defect, not a TLC defect. For `s=[i \in 1..1 |-> 1]`, the
pinned TLC run reports `Attempted to apply function with integer domain to the
non-integer argument "a"` when Next evaluates `s["a"]`. The old frontier/recovery
path instead reported a tuple-index error because it discarded the representation
bit. Both had one generated/distinct state, depth one and TLC exit 75; the clean
message stream carries EC1000 then 2103. Memory and predecessor-only storage
already retained the correct function diagnostic.

Structural storage format 2 now retains this distinction, including nested
functions, in both profiles. CP01/FQ01/FI01 imports explicitly refuse; users must
recreate old archives. This is not a result-changing default or a new divergence:
mathematical canonical keys and fingerprints are unchanged. Native error metadata
still reports `verified: false` for this ported diagnostic site; representation
matching does not certify every error class. Independent transported-witness
checks remain separate. The consumed comparison controls cover both literal
and integer-domain representations against a string-domain record: TLC mode
keeps its comparison error, while default D-009 yields the structural answer.
Those comparisons cannot detect the lost representation bit. Application remains
the representation-discriminating control; independently checked invariant
witnesses validate mathematical word correctness and cannot detect that bit.
See [storage and recovery](../user/storage-recovery.md) and the consumed
`storage_tuple_function` tests.
### Actual committed request-prefix limitation (#204)

The Rust request diagnostic publishes only an actually returned contiguous source prefix before its periodic check, using the shared safety/output/count path and retaining later buffers. Graph/source publication can precede physical frontier append; deciding callbacks preserve that boundary. Complete24 actual pinned method observations include returned-but-unwritten Next output, later graph writes,2179 worker errors,2116 candidates,2104 hook stops and EOF. Hook exceptions exit2 without probe cleanup; early nonexception returns can leave workers alive. These measured boundaries earn no ordinary native scheduling/count/verdict credit. The original8192-binding candidate retains its independent-validator work refusal. No new semantic default or D-ID applies; ordinary scheduling/checkpoint/model proof remain unfinished.
**Historical checksum-era scope (superseded by issue #241):** D-008 nested sequential procedure checksum continuation (ADR 0061): Full editing
uses typed shared header vectors for proved If/With/Either/While transfers. The
180-row selection retains the old six byte mismatches/refusals and all primary
errors/invalid outputs; no reference defect is silently repaired. Broader call
scopes and strict #124 saved-frame replay remain qualified. See
[the user guide](../pluscal-translation.md).

**Historical checksum-era scope (superseded by issue #241):** The bounded nested-procedure checksum implementation retains a failed development
producer that sorted shared call frames even after `while TRUE` discarded their
generated tail. The repaired path preserves pinned creation order for that tail.
Separately, TLC optimizes `(TRUE)` only when its tokens occupy one logical line;
line breaks in a comment between tokens prevent that optimization. Native output
retains the conditional stored-header path in that spelling. Public generation
continues to name the unsupported multiline procedure expression layout, with no
replacement/CRC claim. The smallest paired C/P
controls and original mismatches are in the nested-procedure checksum study;
these are fidelity repairs, not new result-changing defaults or full parity.

### Disk seen-state retention is a library increment

D-007 now has an additive Rust disk-membership library, described in
[the user guide](../user/disk-seen-library.md). It preserves full fingerprints or
canonical exact bytes and reports storage failures explicitly. This is not yet
ordinary checker evidence by itself: the default seen sets remain resident.
The later #212 opt-in inline route has a separate contract below. Its
physical entry count does not certify a discovery prefix, and bounded collision
summary refusal cannot be counted as completed safety. Frontier/predecessor
budgets, recovery and total-memory qualifications remain unchanged.

### Request-owned service limitation (#208)

The Rust diagnostic now services one real request after successful source-prefix graph publication and physical frontier append, including an empty request with no invented source. Pin94d0c50 measured empty FALSE2104 and repeated unchanged graph FALSE2104, hook division errors2179/exit2, and written-graph temporal2116 before the hook. Actual candidate words are replayed independently; clock/suspendAll/count/checkpoint parity remains unfinished. Every failed adapter/EOF-race and incorrect native test assumption remains preserved; no new TLC bug/default or ordinary verdict is declared.
**Historical checksum-era scope (superseded by issue #241):** D-008 P elsif procedure-checksum continuation (ADR0066): the native flat source
branch list now has a typed virtual nested-If header view for the pinned shared
Else-vector mutation. Before this increment the32 clean C/P controls produced17
exact pairs and15 named P refusals; focused stored-header bytes/CRC now match all32.
Public artifact/model evidence is separately source-bound, never inferred from
CRC equality. General procedure/scoped checksums and strict saved-frame replay
remain qualified; see [the runnable guide](../pluscal-translation.md).


**Historical checksum-era scope (superseded by issue #241):** D-008 disjoint sequential procedure checksum continuation (ADR 0069): main can
call globally disjoint, explicitly labelled procedures through the bounded Full
editor. With declarations `Put(z,a)` then `Get(q,b)`, main calls `Put(1,2)` then
`Get(3,4)`, the pinned stored header keeps `Put.proceduresCalled` empty and stores
`<<"Put", "Get">>` on `Get`. The measured algorithm CRC is `99cb7254`. Reversing
main calls yields `182d4367`; reversing declarations yields `8197b276` and moves
the shared vector to `Put`. These original stored-header fields differ from the
separate exploded call frames, whose saved slots belong to the actual target.
Native serialization preserves both in both profiles; it does not infer header
fields from reachability. Procedure-to-procedure/recursive/process/scoped checksum
ownership stays refused by name. Fair CRC-only policy and strict saved-frame
replay #124 remain unchanged; CRC equality does not certify a usable artifact,
model proof or independently valid witness. Read [the guide](../pluscal-translation.md).

The 98-row study also retains two invalid `-fair` command controls: clean pinned
translator exit 255, separate development stage-probe exit 0 carrying the error,
and native CLI exit 2. Those errors are not TLA grammar or checksum equivalence
credit. Original verification-driver failures remain archived; corrected command
accounting never changes their oracle or native bytes.
The bounded periodic backend diagnostic exposes source-qualified graph growth, actual pool-prefix facts and first-candidate stopping. It still refuses to claim ordinary periodic scheduling or Holds. TLC uses pointer-table buckets and records processed OOS sizes even after a found violation; four measured thrown-check faults retain old snapshots. These reference quirks and native limitations are preserved in the complete controls, without native IO-error agreement credit.


The native clock/dispatch lifecycle diagnostic now samples its own clock and holds real pool dispatch authority during commit-boundary checks. Mandatory final SCCs run at actual safety EOF. TLC’s asynchronous progress-thread/queue timing remains unimplemented: no ordinary verdict or Holds is claimed, and PERIODIC/checkpoint transactions stay refused. Selected96 real queue controls are narrow lifecycle evidence, not model scheduling proof.

## Native periodic hook boundary

The pure supplied-expression API now composes a bounded hook at real native dispatch pauses, with original FALSE comparison and evaluation errors. It does not bind configured `_PERIODIC` and does not enable ordinary checking. The complete59 actual-TLC controls retain even setup failures: an `_PERIODIC` constant name throws ClassCastException rather than a stable binding diagnostic. Native configured binding remains a named refusal, so no fabricated error-class agreement is claimed. Checkpoint/host effects, ordinary schedule and stopping-count parity remain pending; no result default changes.

### Configuration-owned periodic diagnostic limitation (#197)

`_PERIODIC Hook` is now selectable through a separate bounded Rust factory, preserving the original pre-constant body. Ordinary CLI scheduling remains unsupported. A constant-valued periodic role throws a real `ClassCastException` in the pin; native diagnostic binding refuses it by name rather than inventing a TLC code. The complete 26-row selection plus 12 startup/precedence rows show unused `PrintT` startup before name/arity failures and symbolic constant startup work that TLC permits. The guarded native factory refuses these before output under its unchanged inspection/startup policies, so those rows are refusals, not TLC-error agreement. User contract and executable example; [ADR 0057](../decisions/0086-rescue-reset.md). No ordinary-schedule or family certification is granted.

### Progress-request/worker stopping boundary (#201)

The producer-clock Rust diagnostic now has a real expansion claim barrier and joined dispatch pause. It still cannot use an uncommitted expansion as a written temporal graph. Complete33 actual pinned method controls retain successful resume, early temporal/hook stops, evaluation errors, real EOF and failed Init. At widths1/4/8, early temporal2116 and false-hook2104 return with workers alive; later probe cleanup joins are not original runTLC behavior. A worker division-by-zero sets2179 with keepCallStack=true while runTLC itself returns0, so that return is not model-check success. These are measured method boundaries, not newly declared TLC bugs or ordinary native parity. Ordinary scheduling/count/checkpoint/model-proof authority remains unfinished.

### Actual committed request-prefix limitation (#204)

The Rust request diagnostic publishes only an actually returned contiguous source prefix before its periodic check, using the shared safety/output/count path and retaining later buffers. Graph/source publication can precede physical frontier append; deciding callbacks preserve that boundary. Complete24 actual pinned method observations include returned-but-unwritten Next output, later graph writes,2179 worker errors,2116 candidates,2104 hook stops and EOF. Hook exceptions exit2 without probe cleanup; early nonexception returns can leave workers alive. These measured boundaries earn no ordinary native scheduling/count/verdict credit. The original8192-binding candidate retains its independent-validator work refusal. No new semantic default or D-ID applies; ordinary scheduling/checkpoint/model proof remain unfinished.

### Ordinary disk seen-state safety (#212)

D-007 now has an explicit ordinary safety route: `--seen-disk-entries N` stores
full fingerprints or canonical exact keys on disk through Result operations.
The original resident command remains default; admitted complete outcomes and
concrete witnesses match that reference. Requested parallel widths currently
use reported inline fallback, not advertised scaling. Entry/probe/summary limits
and I/O/cleanup errors refuse completion. Unproved configured role contexts, graph, temporal, checkpoint/
recovery and DFID combinations remain unsupported. Separate frontier and
predecessor budgets can combine, without a total-RAM/crash/import claim.
[Executable user/API guide](../user/disk-seen-safety.md).


The first disk slice refused invariant-evaluation error call-stack replay.
That guard followed a real bug: `ReplayMembership` starts at `x=0` or `x=2`,
visits `x=1` then the already-seen `x=2`, and divides by zero at `x=1`.
Clean TLC and resident checking report 3 generated/3 distinct/1 queued; the
failed disk attempt incorrectly reported 4 distinct/2 queued by reading an
empty resident reference. Those original observations
remain separate from the positioned implementation.

The next increment queries and inserts actual disk keys through fallible
operations in source/ordinal order. `ReplayNew` proves that successful replay
can add distinct/queued states while generated stays frozen: clean TLC gives
2 generated/4 distinct/2 queued, depth 2, exit 76. Storage/bound/cleanup failures
retain the original error and diagnostic prefix while refusing completion;
unknown error classes remain unverified. Unproved PROPERTY/SPECIFICATION contexts,
parallel disk/capture/temporal and recovery restrictions remain. See
[the user guide](../user/disk-seen-safety.md) and source-bound replay study.

The disk error-replay continuation has a separate output refusal: clean
`ReplayPrint` emits `"expand"` three times at 2 generated/4 distinct/2 queued,
where the pre-review native run emitted it once. Actual Print/PrintT execution
before or during reconstruction now yields a named unsupported result in both
profiles, including empty events, Init-only printing, wrappers and configured
aliases. The original error/counts/output remain diagnostic; neither a TLC
class nor a counterexample certificate is claimed. Earlier resource/storage
causes retain precedence. Warnings and unexecuted Print definitions do not
trigger this event-based guard. Pure positioned replay is unchanged; resident
memory replay's repeated-output contract is not extended here. Clean primary
records and the original failed output are retained in the replay study.


**Historical checksum-era scope (superseded by issue #241):** The bounded acyclic procedure continuation also preserves TLC's call-list alias.
For `Outer(p)` calling `Inner(p+1)`, with main calling `Outer(1)`, pinned stored
header lists Outer=[Inner], Inner=[Outer], CRC `256207b7`. Reversing declarations
lists Inner=[], Outer=[Inner,Outer], CRC `90b81514`. A runtime chain to Leaf has
CRC `37b455c8` and its last header still records main's Outer call; an unused
Outer-to-Inner procedure has CRC `6e37c648` while the last header stays empty.
These measured checksum fields are not a recursion or reachability graph.
The native Full editor now preserves those fields separately from actual body
edges, admitting proved disjoint acyclic sequential calls. Real recursive,
process-owned and scoped checksum contexts remain named unsupported results.
Both profiles retain reference CRC behavior, including fair-header-only weak
detection. [Runnable C/P and Rust examples](../pluscal-translation.md)
explain artifact/model boundaries and the unchanged strict saved-frame #124 gap.


## Historical D-008: Recursive procedure headers without a termination claim

The retired bounded editor derived pinned Full headers for disjoint
sequential recursive source programs. `Recur(n)` with a decreasing base case
has paired C/P header CRC `72442bcc`; an unused recursive owner still serializes.
Source call vectors and the LAST main alias stay distinct. Self tail calls keep
the existing frame; cross-owner tail calls restore caller locals and replace
its head before entering the target. Both profiles match the same checksum
behavior; execution is never unrolled to compute a header.

The zero-parameter `R1:call Recur(); return;` control has paired header CRC
`3db87d9a` but TLC's generated TLA is rejected by SANY. Native raw equality is
diagnostic-only: public editing emits no admitted replacement. Nonterminating
recursion also earns no completion claim. Strict saved-frame replay stays
pending under #124; canonical-only evidence is not strict TLC validation.
Read [the C/P and Cargo guide](../pluscal-translation.md).
Broader process/scoped/automatic-label/multiline procedure checksum contexts
remain named refusals, not conformance successes.

The candidate-admission delivery passes its45-stage/1057-test gate and admits12 actual EOF words via two independent routes;48 deliberately corrupted words fail both. Source-error history is retained: an unparenthesized IF material workload leaves one variable undefined (pin exit75), whereas the separately parenthesized workload gives exit13. This is ordinary TLA+ grouping, not a TLC bug or semantic divergence. The required37 safety gates at1/4/8 remain green; none of these results certifies ordinary temporal scheduling.

### Genuine optional configured-hook diagnostic (#220)

The bounded Rust diagnostic retains actual absence
instead of requiring a synthetic configured TRUE hook. Pinned26 selected method
controls show unused FALSE/nonzero-arity declarations still yield null; unforced
no-hook/no-liveness service returns0 without suspension, while a separately forced
service really suspends/resumes and retains force when no liveness runs. Native
request receipts likewise distinguish initial progress publication from forced
liveness; an empty request is not EOF. This is not a new TLC bug/default change.
Original hidden-startup/state/temporal/error refusals and all quotas remain; no
ordinary scheduling, stopping-count, Holds or family certification is granted.


### Exhausted-queue terminal service (#222)

Bounded additive Rust terminal diagnostics preserve
pinned `AbstractChecker.runTLC`, `ModelChecker.doPeriodicWork/modelCheck`,
`Worker.run` and `StateQueue.finishAll/suspendAll` ordering. A genuinely finished
queue refuses suspension and skips the hook before final selection. The complete
60 corrected full CLI and 60 controlled actual outer/queue observations remain
separate from native authority. Original misleading safe labels and observer
compile failures are retained; independent replay confirms their legal stutter.
No new TLC bug/default divergence is inferred. Existing ordinary CLI/clock/queue,
checkpoint/error/count equivalence remains unproved; F-023 stays missing and
audit-required. Controller Complete and independently Valid concrete words are
not temporal Holds or model completion. See [ADR0076](../decisions/0086-rescue-reset.md).


### Distinct partial-stop and full-CLI observations (#222)

The separately controlled 12-row queue study holds a genuinely dequeued state:
physical queue size 0 is not finished while a Worker owns outstanding work.
Actual suspension succeeds, partial checking precedes the hook, and FALSE or
division stops without final selection; a deciding temporal2116 precedes FALSE.
These bounded scheduler observations are separate from the natural full CLI.
The original observer discards the integer returned by `AbstractChecker.modelCheck`;
its earlier description as a void method was incorrect and is preserved as history.
The separate corrected twelve-row observer records actual returns at widths 1/4/8:
TRUE 0, FALSE 2104, division 1000, and temporal violation 2116. Reflected
`errorCode` 0 and observer process exit 0 are separate fields. The subprocess
exits zero because its adapter does not forward the method result to `System.exit`;
none of these early-stop observations establishes temporal success.

The separate unmodified full CLI FALSE-hook counter exits 10, with 3,913,590
states generated/distinct and one queued state after 4.2456s; it prints no success
and no 2104 message. The declared 512MiB heap and 30s observer bound remain recorded.
This is an intentional-boundary observation, not a newly declared TLC bug. A
native early hook/service stop remains incomplete or refused and cannot be Holds.
All original selections, cleanup events and raw command/timing facts are retained
in the evidence.

## Stopped-worker diagnostic boundary (issue 224)

The bounded Rust stopped-service API preserves the original deciding worker
error: pinned Next division returns2179/CLI255, invariant2110/12, action
property2112/13, incomplete2109/75 and deadlock2114/11 at1/4/8. An actual FALSE
hook does not override a prior division/invariant error. This is an honest native
limitation, not a new default divergence or upstream-bug claim: Java can write
liveness successor records after its error winner, while native diagnostics
contain only the earlier actually committed prefix. No ordinary queue/count/graph
or clock parity follows. The guide distinguishes
actual method returns, CLI exits, callback failures and joins.

### Disk safety roles (#221)

Disk checking binds `PROPERTY` and `SPECIFICATION` roles through the same binder
as the resident route and gives the resident route's results, including for
effectful, quantified and fairness-only specifications. Temporal obligations are
an explicit unsupported combination. [CLI/Rust contract](../user/disk-seen-safety.md).

**Historical checksum-era scope (superseded by issue #241):** D-008 process-owned procedure checksum continuation (ADR0079): the stored
original header gives each process its own called-procedure vector. Calling a
procedure from a process does not append the call to the last procedure header,
as sequential main does. Parameter/local/frame fields remain unindexed in this
header; generated runtime state is separately indexed by process ID. The bounded
native Full scope has one unfair finite process and one disjoint labelled
procedure; broader fairness/scoped/multiple-owner contexts refuse explicitly.

A clean minimal C/P `procedure First(a=0) ... process Worker=1 ...
with a=2 do call First(a)` example generates TLA with duplicate operator `a`.
Pinned SANY rejects both outputs. The complete242 selection retains both raw
artifacts, their original/header/direct-exploded stages and SANY errors; the
native scoped-name guard returns a named refusal and grants no successful Full
replacement. No new default correction is made. Exact CRC, CST/SANY/Validator,
finite model results and canonical/strict replay remain separate obligations.
Read [the runnable process/procedure checksum guide](../pluscal-translation.md).


## Terminal finalization and postcondition order (#228, ADR0080)

The bounded Rust terminal guide exposes actual private
capture/resource completion, OOS/PEM final checks, independently replayed words
and subsequent direct post errors. `ModelChecker.modelCheck` runs finalCheck
before normal `Tool.checkPostCondition`; `LiveWorker.printTrace` also calls
`checkPostConditionWithCounterExample`, ignoring returned errors while getter
exceptions can abort rendering. The complete24 minimal pinned rows distinguish
2405/CLI10, ignored2405 then2116/CLI13, cached-role GENERAL1000/CLI255 and the
candidate getter exception/CLI1 with no method return. These are measured
ordering contracts, not a newly declared TLC defect or approved default change.
The unchanged8820 wrong post-before-final observation and all84/84/84/96
reference development/selection epochs remain in
the acceptance evidence.

Only direct bounded scalar/Boolean posts are admitted; calls, effects, configured
constants, constructors and unproved partial-render/getter exceptions refuse.
Current final records require original source/commit fingerprint alignment,
per-node write coverage and actual tableau done authority under existing graph
budgets. Completion remains diagnostic: ordinary clock/queue/error/count,
no-action and checkpoint composition are unfinished. No Holds, model-proof or
family credit is claimed; F-023 remains missing/audit-required.

### Public Rust IR: Strict context admission (#234): withdrawn

The structural pre-entry proof for `Pass::Strict` substitutions was removed on
2026-10-12 (ADR 0016, amendment 3). The validator now refuses only bindings
whose effects annotation is impure or whose context belongs to another model.
A caller that forges an inert annotation on an effectful call can make
validation run that effect (a `Print`, for example). This never changes a
verdict.

The retained public serialized-IR control references `Noise(v) == PrintT(v)`
from a Strict binding but annotates that copied call `Effects::NONE`. Its valid
owned context previously produced three Print events before `validate_doc`
returned Valid, and one before `evaluate_lasso` returned Unsupported. Independent
existence search already refused without effects. This is a dive public-IR
admission limitation, not a false-word or naturally lowered TLA+ parity claim,
and does not introduce a TLC divergence/default ID. Repair source is present;
fresh receiving qualification is pending under [#234](https://github.com/Chelis-Lang/dive/issues/234).

#### Pure eager instance operands and independent admission (#234)

The initial blanket Strict refusal also refused legitimate literal TRUE and
nested constant forwarding in existing pinned instance tests. The bounded
repair inspects actual closed structure and scoped values before eager entry,
keeps malicious Print refusal, and trusts no inertness flag. This is a dive
admission limitation/repair, not a new reference-tool divergence. Native/user
calls, providers and exhausted/opaque eager contexts remain unsupported. The
complete original 1,762-pass/two-failure gate is retained; fresh qualification
is pending.

---

