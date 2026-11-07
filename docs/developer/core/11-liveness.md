# 11. Liveness on the core

Status: L1–L4 and the native L5 capture/analysis/report path are implemented,
2026-10-29. The production core retains persistent original rows and CSR,
reuses bounded flat-value conversions during demanded reads, and releases native
capture storage before independent candidate replay. The reference-library
entrypoints and explicit caller limits remain available unchanged. Complete
memory accounting and measured TLC capacity remain L5 acceptance work (§7).

This document describes how `--engine core` checks genuine temporal properties
(`<>P`, `[]<>P`, `P ~> Q`, `WF`/`SF` fairness). The safety parts of properties
(implied initial predicates and implied actions) run on the core
(`07-engine.md` §7). Reference and explicit Rust capture limits remain in force; ordinary core file
requests use actual run budgets and checked machine capacity (ADR 0124).
The dated census in §10 precedes the native backend; it establishes no new
coverage, performance or capacity for that backend.

## 1. What the reference engine does

A temporal run in `dive-api` (`crates/dive-api/src/temporal.rs`) has four
stages, each with its own owner:

1. **Capture** (`dive-check`, `crates/dive-check/src/capture.rs`). The
   ordinary safety search runs with a collector attached and returns a
   `BehaviorGraph`, which has:
   - `states`: one `StateVals` per distinct state inside the model, in
     discovery order;
   - `edges[s]`: the generated successors of `s` inside the model, in
     generator order, each as `{to, ordinal, label}`, then one implicit
     stutter `{to: s, ordinal: None, label: None}` once `s` is fully
     expanded;
   - `concrete[s]`: the original successor values of those edges, including
     successors already seen;
   - `roots`: the distinct initial states inside the model.

   The safety checks (invariants, constraints, deadlock and implied
   properties) run as usual during capture. A safety ending becomes
   `Outcome::Safety` with ordinary counts. Capture is bounded by
   `capture::Limits` (1,000,000 states, 8,000,000 edges, 32,000,000
   generated, 65,536 successors per source).

   `Complete` and `Prefix` wrap the graph with a receipt that only the
   producer can construct (`GraphCompletion::finalize` checks the graph's
   shape against the counters).
2. **Analysis** (`dive-liveness`, `analyze.rs`). `prepare` lowers each
   property's negation to a tableau, once. Then `analyze_captured` does the
   rest on the reference evaluator:
   - evaluates state atoms on `graph.states`;
   - evaluates action atoms and fairness `ENABLED` on the edges, using
     `concrete` for edge targets;
   - builds the product and searches for an accepting SCC;
   - returns a lasso (`states`, `back_to`, `edges`), or nothing.
3. **Scheduling.**
   - Default profile: the analysis runs at completed levels chosen by
     `periodic::Schedule` (D-014; at least 2^16 states, doubling). It
     considers only the first property, and a final check runs at
     exhaustion (`analyze/periodic.rs`).
   - `--compat=tlc`: only the final check runs (`-lncheck final`).
4. **Validation and report** (`dive-api`).
   - `finish_final` and `finish_periodic` rebind the model separately.
   - `dive-trace` validates the lasso twice: in memory, and as the
     serialized document.
   - Only then is the result built as a `temporal::Report`, which
     `check.rs` renders as `Phase::Temporal`. A lasso that does not
     validate is a validation failure, never a verdict.

## 2. Principle: reuse the analyzer and independent validator

The core explores and captures native original state rows and CSR metadata.
It reuses the existing property planning, predicate evaluation, tableau,
product/SCC search, attribution and deterministic schedule. Private adapters
separate topology, original-edge metadata and state-value demand; they do not
introduce a new liveness algorithm.

`NativeGraph` owns addressable persistent rows and keeps their `Store` alive.
`NativeValues` admits every accepted original in producer order, then converts
rows again only when the analyzer requests them. Production capture never
constructs a `BehaviorGraph`; `core_graph.rs` remains a test-only legacy adapter.
The same graph-independent producer facts and selected-word assembly serve both
representations. Counts and completion facts come from the actual producer.

A selected word retains concrete values, original edge positions and property
attribution. The native graph, converter, plan and store owners are explicitly
dropped before `dive-trace` rebinds the model and validates the typed word and
serialized document. A validation failure publishes no temporal violation.
The original model remains available for report qualifications; native storage
supplies no authority to the independent validator.

Storage/result equivalence controls compare the native path with reference
capture and analysis in both profiles, including original aliases, duplicates,
ordinal gaps and committed prefixes (§8). Observable parity remains ADR 0086's
contract; temporal error-demand order, VIEW/symmetry quotient semantics and
broader language support remain separate work.

## 3. Graph capture on the core (`dive-explore`)

A `GraphSink` passed to `dive_explore::explore_with` (`graph.rs`) receives
these events, all from the single-threaded commit, in commit order:

| Event | When | Reference rule (`dive-check/src/engine.rs`, `on_state_checks`) |
|---|---|---|
| `emitted(generated, ordinal)` | every generated state, before its checks (`ordinal` is `None` for an initial state) | `capture.allow_emitted` |
| `state(heap, Step)` | a state inside the model that passed its checks, new or seen. `Step.original` carries its actual closed persistent representation; `new` separately carries a new id and canonical frontier handles. The key, parent, ordinal and action index remain attached to the original | `capture.record`; a root when `parent` is `None` |
| `stutter(source)` | a source is fully expanded with no stop | `capture.stutter` |
| `level_done(level, stats) -> bool` | the commit of a completed level; `true` stops the search there (`Verdict::PrefixStop`) | `Observer::level_done` and `stop_after_level` |

Each event may return `Stopped`; the search then ends with
`Verdict::GraphStop` and the sink keeps the reason. Vertex ids are the
predecessor log's ids, which are the discovery indexes.

The reference collector maps keys to discovery ids. Equal keys identify one
graph vertex under fingerprint-mode identity and its collision qualification;
they never license substituting that representative for an original successor.
The production sink (`dive-api`, `core_native_graph.rs`) admits every accepted
original through `NativeValues`, including seen targets. Capture-only `intern_exact` retains available kind and view flags
before fingerprinting can replace a row with canonical identity handles.
The core still refuses exact storage (`07-engine.md`).

Capture also retains first-discovered original rows for the current/next
frontier. Source evaluation uses these rows so `UNCHANGED` or `EXCEPT` does not
inherit a class erased by canonical identity interning. Seen aliases never
replace them. `Step.new` and replay expectations remain canonical. Generator
errors take the replay expectation directly from the canonical frontier row,
not the original source used for evaluation; checked successor endings
canonicalize their expected parent, and deadlock uses the frontier identity.
The extra frontier payload is eight bytes per variable per current/next
frontier state; the existing capture admission bounds apply, and all interned
values remain in store accounting. These frontier rows are separate from the addressable all-vertex/original
rows retained by `NativeGraph` (§4).

Parallel capture uses the reference's `MAX_EXPANSION_UNITS` (eight sources per
wave). Explicit collectors retain `Limits::successors_per_unit` (65,536 by
default); ordinary core file capture uses checked ordinal capacity instead. An overflow emission
retains only a stop marker; its emitted event delivers the existing named refusal
in commit order. Explicitly limited original handle payload is at most eight times the
per-source limit times the variable count per wave, plus transient row snapshots.
The ordinary core route uses fallible provisional/frontier growth and actual
run-budget checks instead of this artificial per-source count.
Workers also poll the armed budget before retaining each capture emission.
This is an entry-count bound, not a byte bound: original compounds interned by
speculative work remain in the shared store even when their wave is discarded.
The final store node/handle statistics include that work, and whole-process RSS
sampling includes its allocations. No graph means no added original retention,
per-emission polling or change to the ordinary safety chunk size.

How the events follow the reference engine:

- **States outside the model** (`CONSTRAINT`, `ACTION_CONSTRAINT`) produce
  no vertex and no edge.
- **Ordinals** are the generator's emission index within the source.
  `selected_targets` in `dive-liveness` checks them against `concrete`.
- **Labels** are `model.actions[action].label()`, the same strings as
  `Successor::action.label()`. The core keeps the action index, and
  the native collector retains a run-local label table.
- **The parallel engine** commits in position order (`07-engine.md` §3), so
  the events, and therefore the graph, are identical at every worker count.
  Expansion stays parallel. Only the event delivery is ordered.
- **Refused at first**, each with a named reason:
  - `SYMMETRY` and `VIEW` with temporal properties. The reference keeps a
    representative plus concrete values, and has its own symmetry-liveness
    rules (`symmetry_liveness`).
  - `POSTCONDITION` with temporal properties (the reference refuses it too).
  - Checkpoints, DFID, `--temporal final-short` and host I/O hooks.

Initial-state recording alone does not trigger a completed-level callback. The
first callback follows expansion of the initial BFS frontier, with completed
source rows and any newly discovered frontier. `native_level_due(stats)` checks
the exact schedule/decision/nonzero-queue predicate before a full CSR validation
scan. Final-only and not-yet-due callbacks skip that scan. A checked-shape or
budget failure travels through the sink as its original failure, not a successful
prefix.

`dive-explore` gains no dependency: the sink is a trait over core handles.

### Native name-domain admission

ADR 0127 discharges only the binder's typed runtime-name-only sequential
restriction for the private freshly bound native producer. Full conservative
source/configured/captured effect checks remain. Admission verifies actual scope
Core/IR/Interner ownership and every startup token/text against the actual sealed
native Store; arbitrary public bound models/providers are not accepted by this
private entrypoint. Native generation retains the existing new-name refusal.

A wrapper observes the exact append-only reference Interner across the entire
temporal operation, including all early returns, planning, capture, periodic and
final analysis, attribution and report construction. Growth prevents semantic
publication; nonsemantic resource/interruption/bug transport retains its phase.
Fresh independent replay separately refuses new names in its own domain. The
reference concrete-pool source policy is unchanged. Actual callback and original
row evaluation still supplies truth; absence of new names is no state-space,
class, representation or word proof.

The unchanged original pinned EWD998Chan source/property, with only N=1 derived
configuration, agrees with fresh pinned TLC and reference at 6 distinct /13
generated /depth2 in both native profiles at W1/W8. Original N=3 also completes
at W1 in both profiles on reviewed source `3d242d83`: 1,524,022 distinct /
12,882,257 generated /depth58, verified success and exhausted capture. Under
unchanged 900-second/16-GiB limits, TLC takes 535.537s with 1,866,358,784-byte
peak RSS; default takes 861.826s with 1,938,173,952-byte peak RSS. These are
single-run capacity observations, not quiet TLC-relative throughput or W8
qualification. See the [source-bound record](../../../conformance/reports/perf/2026-10-30-ewd-native-actions-capacity.json).
The complete joint matrix and final performance panel remain open.
Tuple SubSeq, new-name producer and
state-atom demands, early periodic/error returns, foreign scopes/configured
captures and nonsemantic stop priority have focused controls. No source rewrite,
worker fallback, new mode or static state-type approximation is used.

## 4. Native rows and on-demand `StateVals` (`dive-api`)

`core_native_graph.rs` retains flat persistent handle rows, representative row
ids, CSR offsets/32-bit targets, original ordinals, action indices/labels,
original target row ids and roots. An edge shares its destination representative
row only when the handle slices match exactly. Extensional value equality or a
matching fingerprint never licenses representation substitution. Every vertex
and differing original remains addressable until analysis and lasso extraction
finish. The capture-only key index is released before final analysis.

Action-label selection retains the first original's physical position for each
destination, or an implicit-stutter descriptor. Native predicate evaluation
resolves that selected original directly rather than scanning the outgoing row
again per destination. Ordinals and labels for the complete row are still
validated before evaluation; selected values and the source representative are
converted in their existing order, including failures and stops. A generated
self-edge keeps its original value instead of becoming implicit stutter.
The descriptor lives only in the existing per-source selection map; this does
not supply a whole-process memory metric or original-capacity qualification.

`core_native_values.rs` uses
`dive_core::print::to_value_controlled(.., Conversion::Graph)`. Every accepted
original, new or seen, is admitted before state/edge retention admission;
emitted-count admission still comes first. Admission succeeds exactly when the
Graph conversion of the row would, and interns the same new names in the same
order, but builds reference values only where that is needed to know it. It
walks the row in conversion order (set elements and integer-function values in
order, function pairs key first). It enters a node without converting it only
when the node's own reference construction cannot fail once its children
convert: an eager normalized `IntFcn`, a `Set` that is empty or holds inline
integers only, strings only or booleans only, and a `Fcn` whose keys are all
strictly increasing inline integers (not a record literal) or all distinct
strings. Every other node, including any set with compound or mixed elements
(`{1, "a"}` is refused by the reference constructor), is converted in full, so
its refusal text is the conversion's. A string or model value is converted, and
so interned, at its first occurrence in walk order; a run-local bitset of
converted name ids skips later occurrences. A persistent node that was walked
or converted successfully goes into a lossy direct-mapped table
(`min(8 MiB, memory_budget / 512)`): its later conversion would succeed and
intern nothing. `values.skipped_admission_conversions` counts rows admitted
with no conversion, `admission_memo_hits` the table hits. Graph conversion preserves general-function
versus tuple/record literal classes recursively (ADR 0121). Raw or symbolic set
representations without certified normalization history are refused; report
formatting conversion does not supply that history.

**Deferred admission.** When every liveness atom was prepared natively
(`atoms.eligible_atoms == atoms.total_atoms`), the sink defers admission:
it retains rows without converting them, and the first read of any row
(a reference fallback, the source-row check, the audit or a report) first
admits, in retention order, every retained row not yet admitted. A seen
successor that is not retained repeats its representative's handles and
interns nothing; a seen root, which may differ from its representative
without being retained, is admitted when it is recorded, after the pending
rows. The names interned before any reference evaluation are therefore those
of eager admission, in the same order. A row whose conversion fails is refused
at that read rather than stopping the capture, and a run that never reads it
needs no reference value of it. `values.admission` is `"deferred"` or
`"eager"`; `deferred_rows` counts events recorded without admission and
`caught_up_rows` the rows admitted at a read.

The admitted row is dropped immediately; admission never fills or reads the
read cache. Reads can reuse successful values by exact
persistent `Handle` identity in a cache owned by this fixed run/store/profile.
Eligibility is limited to closed materialized trees with Bool/Int or run-owned
string leaves, recursively containing tuples, records, functions or enumerated
sets with complete stored elements. General
function keys must be scalar Bool/Int or run-owned strings. General string-domain functions retain their nonliteral flag;
literal and general representations never share entries merely by equality.
Raw/symbolic sets (even with cached elements), BigInt/model values, foreign name
owners and unproved shapes keep
the controlled conversion path. No failed conversion or normalization-history refusal is cached.

For one-variable reads, a retained row ID inside the table's current length
selects its slot directly. The exact Handle is still compared before reuse: row
IDs supply no value or completion authority. A changed handle at that ID misses;
the same handle at another ID may reconvert. Larger row IDs and multi-variable
components use bounded handle-hash probing and collision replacement. Both paths
share one fixed table and the same byte allowance. It is initialized
after 1,024 read lookups, avoiding a large allocation for tiny checks. Its
allowance is `min(128 MiB, memory_budget / 64)`, or 128 MiB with unlimited memory.
Actual table capacity plus requested eligible payload and one-component row arrays
must fit before insertion. Oversize, arithmetic/allocation failure or unproved
eligibility bypasses caching.
Each cache entry retains a one-component shared row; its array storage is charged
even for immediate values. A narrow value-layer inspector counts the actual
private function header/fingerprint cache and boxed tuple/field storage.
Enumerated sets charge their complete private header and stored element array;
inspection never normalizes, enumerates or invokes a provider.
General functions additionally charge the domain header, key/value arrays and
the full possible scalar-domain TLC-order array (`usize` per key). Scalar keys
cannot create the allocating product-domain index. The remaining lazy function
lookup/fingerprint facts are fixed header fields and carry no observable set
normalization history. Shared descendants are charged per occurrence; inspection
declines after 4,096 values or 64 compound levels, bypassing only reuse. Record Atom owners are included in field storage; name payload is shared
only after Arc identity proves the run Interner already pins it. Opaque Arc
headers/allocator overhead remain outside this requested-payload metric. It is
not a total RSS or allocation-time hard cap.

Actual owner/temporal control is checked before hits and misses and before a row
returns. A cache hit supplies the same immutable converted Value, including
representation flags. For one-variable models, hits return the same shared row
without allocating a new row; multi-variable models assemble their rows from
cached components. This does not change admission or independent word
validation. Cache speed and memory tradeoffs require source-bound measurement;
this implementation alone earns no capacity or throughput credit.

Materialized traversal, comparability proofs, deep normalization comparisons
and sorting cooperate with run control; temporary vectors use fallible
reservation. A foreign or missing interruption receipt cannot become this run's
budget stop. Constructor hashing, diagnostic rendering and final shared/Arc
allocations retain legacy units; this is not an allocation-time hard cap.

`core.metrics.liveness` retains `convert_ms`, `converted_nodes`, `memo_entries`
and `graph_states`; `retained_rows` reports the physical row-ID domain.
`convert_ms_scope` identifies timed admission/read slow paths: one-variable cache
hits avoid the per-read stopwatch and are excluded from `convert_ms`. These
conversion timings cannot be compared with earlier all-read timings as identical
units. Additive `storage: "native-csr"` and `values` report
admission/read/conversion counts, cache hits/misses, entries, configured allowance,
actual table/known-payload bytes, collision replacements, bypasses, direct-slot
lookups/hits and large-row-ID hash fallbacks. These
requested bytes exclude the opaque overhead above. `memory` describes covered collector buffers, action-label
capacities and reserve-growth overlap; the overlap is an upper bound, not
observed peak RSS. `accounting_complete: false` remains explicit (§7).

### 4.1 Optional native predicate evaluation

The native reader can additionally evaluate a prepared state/action Predicate,
Enabled or Nonstuttering atom on the exact persistent source/original target
handles. The
existing Role/Handoff compiler and a separate analysis Machine perform that
work (ADR 0129). Shared label demand order, first original successor selection,
frontier handling, products, lassos, reference attribution and independent word
validation remain unchanged. Legacy row-only callbacks retain their signatures.

Eligibility is a set of Core plan facts (ADR
[0154](../../decisions/0154-native-atom-rows-by-materialization.md), amendment):

- the actual retained atom object and its source scope bind to the model's
  evaluator (`foreign_scope` otherwise);
- `compile_bound_test` accepts the atom, with the same value nodes safety
  checking uses (`compile_error`); Enabled adds its substitution, effect and
  call refusals, the B3 cutoff and the B2/B4 tlc-profile refusals;
- the plan has no effects (`effects`): no Print/PrintT, RandomElement, TLCSet,
  TLCGet registers or `TLCGet("level")`;
- the plan contains no `ToString` (`fresh_string`), and the analysis Heap runs
  with `Heap::set_existing_strings_only`, so any other builder of a text the
  shared string table lacks fails and falls back. A new token would change
  string order under `--compat=tlc` for the rest of the run;
- every compiled string, function and Select-key constant, and every bound
  parameter value of the atom's scope (`\A e \in S : WF_v(A(e))`), is a
  materialized Core value (`const_unclosed`), and so is every input row.

Plan size has no semantic role and is not capped. Recursive operators and
recursive functions run in the analysis Machine's own frames; its 64-activation
call-depth limit is an unsupported error that falls back, and `call` restores
the depth and frame on every path. A Memo site evaluates its body directly: the
analysis Machine compiles with `memo = step_memo = false` and owns a table-less
`Memo`, so not even a step site reads or fills a table. `^` charges an upper
bound on its result's 64-bit words to fuel before allocating digits.
There is no separate reference stability certificate: ADR 0129 kept one to
protect reference constructor, normalization, name and thunk history, which
ADR 0153 treats as a TLC artifact the default profile does not reproduce. A
fresh conversion per read means skipping a reference call cannot change a later
value.

Admitted shapes therefore include bounded CHOOSE over any enumerable set (the
first satisfying element in `Heap::cmp` order, which equals the reference value
order per profile; an error before the first witness falls back), Cardinality,
set filters, SUBSET, UNION and set algebra, records, function constructors with
several binders, and EXCEPT with several clauses, nested paths and `@`. They run
through the analysis Machine's bounded normalization and enumeration
boundaries, which preflight SUBSET, function-set and union materialization
before allocation. The analysis Machine runs with
`Machine::set_refuse_unrecorded_warnings`: an EXCEPT key outside the function's
domain, where the reference prints warning 2144, is an unsupported error and
falls back, so the reference still prints it. Warning 2143 is recorded in
`Machine::warnings`, and any recorded warning or printed output also falls
back. Enabled fairness tests reuse existing core successor search and INSTANCE
identity/cutoff checks; fairness subscripts reject stuttering candidates and
continue the search. Ordinary Nonstuttering compares the retained before/after
scoped subscript, in source then target order, before the action; unlike
Enabled, it creates no branch cutoff and preserves nonidentity substitutions.
NegatedEnabled atoms stay on the reference lane.

Unknown representations, optional work
bounds and semantic failures use the same reference fallback. Actual owned
budget stops propagate; foreign interruption evidence is rejected. Optional query
binding streams the existing cursors under its work quota, preflights eager
materialization before allocation and charges universal context buffers. Central
bounded Machine normalization/enumeration/cardinality boundaries cover all direct
interpreter, generator and library callers, including subset comparison and
candidate values; higher-order sequence copies are charged before allocation.
Unknown eager symbolic shapes fall back before calling Heap, and optional
materialization never interns into the producer Store. Its
in-query checkpoint uses the actual retained budget; quota exhaustion remains
reference fallback, with no fabricated resource stop. String Len charges its
O(1) UTF-8 byte-length upper bound before computing Java's UTF-16 length. Other
string operators (comparison, prefix, replacement) scan without charging, so
input rows keep a name allowance once the Store holds a text over 4,096 bytes
(below).
Each native demand gets `NATIVE_FUEL` (2^20) units of fuel. The allowance
counts explicit semantic traversal/copy work. It is not a
hard CPU-instruction, allocator-latency or wall-clock guarantee for each primitive;
existing whole-run cooperative time and sampled-memory limits retain their scope,
polled at least once per `WORK_POLL_UNITS` (32) charged units, and bound the work and the memory held until the
per-demand `heap.reset()`. Explicit loops such as UTF-16 length traversal are
still precharged. After `FUEL_STRIKES` (8) consecutive exhaustions an atom is
retired to the reference evaluator for the rest of that analysis; a completed
demand resets the count. This is scheduling only and changes no truth value.

Each private native Reader also owns positive input-proof bits scoped to its
borrowed immutable graph (ADR [0134](../../decisions/0134-native-atom-row-proofs.md)).
After the existing descriptor and control checks, `admissible_row` issues a bit
only for a successful actual retained-row demand (ADR
[0154](../../decisions/0154-native-atom-rows-by-materialization.md)). A row is
admitted when every handle is materialized as `compare::materialized` defines
it: sets, model values, big integers and functions with any key class qualify;
lazy, symbolic, SetEnum, transient and empty handles and a poisoned heap
decline. A handle proved by `Heap::proved_concrete` (a scalar, a known name or a
persistent node carrying `flags::CONCRETE`) needs no walk. Otherwise
`Heap::materialized_within` walks at most 16,384 handles; rows restored from a
snapshot or exchanged between workers take this path because those codecs clear
the bit. While every Store text is at most 4,096 bytes the bit is trusted; after
a longer text appears every row is walked and its string and model-name
occurrences count against a 4,096-byte allowance. Compiled string, function and
Select-key constants use the same predicate. Native truth is Core's evaluation
of the stored representation in `Heap::cmp` order, which a property test checks
against `Value::cmp_with` in both string orders. Source and target are checked
separately; Machine fuel is unchanged. Hits still check the current Machine Heap poison and exact Store
owner. No negative result, reference conversion or atom truth issues a proof.
One fallible, checked allocation requests and retains at most one MiB; oversized
graphs and allocation failure run `admissible_row` on every demand instead.
A new Reader at every periodic/final analysis starts empty, and its bits drop
before independent replay. Row proofs do not change admission. `atoms.row_proofs` reports scans, hits, certifications and peak
retained storage bytes across readers; these counters establish consumption,
not a measured original ABA speed or capacity improvement.

The analysis Machine has separate arena/frame/memo owners, with memoization and
folding disabled. It does not intern predicate results into the producer Store.
Additive `core.metrics.liveness.atoms` counters distinguish prepared eligibility,
actual native evaluations and fallback. `atoms.rejections` counts unprepared
atoms by their first failing check (`foreign_scope`,
`compile_error`, `effects`, `fresh_string`, `const_unclosed`, `allocation`);
`atoms.fallbacks` splits actual fallback demands into `row_proof`,
`fuel_exhausted`, `error`, `warning_or_print`, `not_compiled` and
`fuel_retired`, and `atoms.fuel` reports the per-demand allowance, the strike
limit and the retired atoms. `atoms.reference_evaluations` counts the state
and action label demands that label demand answered with the reference
evaluator (unprepared atoms and fallbacks); attribution of a reported word is
not counted. A directly syntactic `~ENABLED A` atom compiles the bounded
search of `A` (both expressions must bind to the atom's scope) and negates a
completed truth; `negated_enabled_evaluations` counts those. These are capability measurements,
not performance acceptance; original workload completion and paired timing are
still required. The conversion-cache allowance is unchanged; admission follows
the deferred-admission rule above.

**Row reads only on fallback.** The Reader declares `reads_admitted`: the
producer converted every retained original through `NativeValues::admit`,
interning names in event order, before retaining it, or deferred that
admission to the first read (above). A later read repeats that conversion and
can fail only by interruption or a storage limit, or, after a deferred
admission, by refusing an unconvertible row. Label demand
then keeps its atom-major order and every metadata check, but reads a source
or target row only immediately before a reference fallback, and the implicit
stutter reuses the source row. Sources without that declaration, including
row-only readers, keep the eager per-vertex demand order.

**Audit mode (developer tool).** With `DIVE_NATIVE_ATOM_AUDIT=1` in the
environment (read once when atoms are prepared), every native `Some(truth)` is
also evaluated by the reference evaluator on the converted rows. A different
truth, or a reference error where native produced a truth, ends the run with an
internal error naming the atom location, test and row ids. Frozen WF reuse is
not re-audited: its column came from audited native evaluations. The audit adds
reads and reference work; `atoms.audit` reports whether it was on.

### Frozen behavior prefix columns

ADR [0138](../../decisions/0138-frozen-behavior-fairness-reuse.md) (amended)
permits one packed column for each atom of the literal cloned bound behavior
prefix (every WF and SF pair and other behavior temporal atom) across
configured properties. The private planner token comes from one frozen prefix;
expression IDs or scope-cycle helpers supply no equivalence. Property-owned
atoms and separately lowered contexts do not receive tokens. A cache belongs
to one immutable graph analysis, never the run or a later prefix/final graph.

Only a complete entirely native column is retained. Every reuse still demands
original metadata, the first physical target (including implicit stutter)
and current native control/descriptor/Store/poison/input
checks. Reader-only adapters decline input reuse by default. Actual evaluations
retain their fuel and fallback behavior; independent replay gets fresh state.

The optional cap is the smaller of 16 MiB and one 256th of requested memory,
including metadata, the per-atom column table and actual vector capacities. Fallible allocation or cap
refusal preserves the original path; interrupted copying publishes no column.
Single-property searches, including current periodic checks, allocate no columns.
`core.metrics.liveness.atoms.fairness_reuse` reports validated state/action
consumptions, stored columns, cap/allocation bypasses and peak accepted storage
bytes, including interrupted construction copies.
`evaluated` and state/action evaluation counters exclude reused truth; row-proof
scans/hits include the input demands made by both paths. Storage/hit counters
establish neither time saved nor capacity. Complete allocation accounting and
original ABA completion/headroom remain separate acceptance work.

## 5. Analysis entry points (additive APIs)

Existing reference callers keep their entrypoints and result types. Development
uses focused checks and required CI; the integrated milestone receives the full
gate (AGENTS.md).

- **`dive-check`.**
  - `capture::refusal(model, options, parallel) -> Option<String>` returns
    the reference capture's own admission refusals (VIEW, SYMMETRY,
    POSTCONDITION, checkpoints and the rest), with the same messages and
    order. `explore` calls it too, so the two cannot drift.
  - `capture::GraphBuilder` wraps the internal collector, so the limits,
    limit messages and row layout are the reference's. The producer calls
    `allow_emitted(generated, ordinal)` before each generated state's
    checks, `record(key, state, id, parent, ordinal, label)` for each
    accepted state inside the model (in commit order) and `stutter(source)`
    once a source is fully expanded. `graph()` gives the graph so far.
  - `GraphBuilder::complete(safety, options) -> Result<Complete, Verdict>`
    runs the same `GraphCompletion::finalize` shape checks against the
    producer's statistics. `GraphBuilder::prefix(level, safety, options)`
    builds a `Prefix` and requires an unexhausted frontier. The receipt
    certifies the graph's shape against the counters, nothing more, as
    today.
- **`dive-liveness`.** `analyze::periodic::external::GraphCheck` exposes the
  existing private steps. `periodic_unbudgeted` and the capture observer
  now share them, so the two paths run the same code:
  - `GraphCheck::prepare(model, options, limits, policy, labels, schedule) -> Result<GraphCheck, Box<PeriodicFailure>>`
    arms the run budget (`dive_check::budget::arm`), validates the context,
    builds the evaluator control and plans every property, once.
    `schedule` is `Some(Schedule)` for the default profile and `None` for
    `--compat=tlc`, which runs only the final check.
  - `GraphCheck::level(&mut self, level, &BehaviorGraph, Stats) -> bool`
    applies the periodic schedule and `analyze_captured` with only the first
    property, as the observer does. It keeps the decision and returns
    whether the search stops after this level.
  - `GraphCheck::finish_prefix(self, Prefix)` and
    `GraphCheck::finish(self, Complete)` both return
    `Result<(PeriodicAnalysis, Checks), Box<PeriodicFailure>>`. The first
    turns a kept decision into `PeriodicAnalysis::Prefix`. The second runs
    the final check over all properties. Both take the run budget's final
    sample as the reference does.
  - `GraphCheck::stopped(self, CheckResult)` reports a safety ending or
    stop of the producer's search as `AnalysisError::Exploration`, never a
    temporal verdict.
  - `GraphCheck::row_order(&self) -> bool` says whether the reference may
    evaluate atoms in TLC's row order (§11).
  - `native_level_due(Stats)` exposes readiness only, without scanning CSR or
    granting completion authority. `level_native(level, &CsrCapture, Stats,
    &mut NativeRead)` checks due committed levels through the same analyzer.
  - `finish_native(CsrComplete, &mut NativeRead)` and
    `finish_native_prefix(CsrPrefix, &mut NativeRead)` return
    `Result<(NativeAnalysis, Checks), Box<PeriodicFailure>>`.
    `NativeRead` receives a checked opaque row id and the actual temporal
    `EvaluationControl`, returning a complete row in IR variable order or an
    explicit error. A missing variable is an inconsistency, never FALSE truth.
    Final assembly consumes complete facts; prefix assembly requires a selected
    word and cannot assemble Holds.

The legacy APIs still finish through `finish_periodic` or `finish_final`.
The production native route uses `temporal::core_native_report` with owned
selected-word values and sealed facts. It reuses fresh independent typed and
serialized replay without reconstructing a reference graph.

## 6. Verdicts, counts and the report (`dive-api`)

`core_engine::check` has a temporal route (`core_temporal.rs`), for
ordinary checking (`TemporalMode::Ordinary`): with the completed-level
schedule in the default profile, the final check alone under
`--compat=tlc`, as `check_backend` routes the reference engine. `prepare`
binds, checks the `ASSUME`s and compiles the plan for both routes; on the
temporal route the binder's temporal-property entry is not a refusal, and
the model is bound with the requested symmetry-liveness policy. When the
model has `implied_temporals`, the route:

1. refuses what the reference capture refuses (`capture::refusal`), what
   the reference's ordinary temporal dispatch excludes
   (`final_roles::route_exclusion`), `Print`, `--max-states` and
   `--max-depth`;
2. prepares a `GraphCheck`, so planning comes before the search, as on the
   reference engine;
3. runs the explorer with the sink, on a thread with the evaluator's stack
   (`RECOMMENDED_STACK`), as the reference runs its temporal checks. The
   safety checks behave as on the safety route, and a safety ending is
   reported as the reference's temporal route reports it (a safety result,
   with the bound model's symmetry resolution);
4. after a fully expanded level, tests `native_level_due` before validating
   borrowed CSR and calling `level_native`. A deciding candidate stops the search
   after that real committed boundary;
5. finalizes native metadata through `CsrComplete::from_producer` or
   `CsrPrefix::from_producer`, preserving actual successful EOF/prefix counters,
   and calls `finish_native` or `finish_native_prefix`;
6. retains only the selected word and sealed facts, drops native graph/converter/
   store/plan owners, then calls `temporal::core_native_report` for independent
   validation and reporting. Safety endings and actual capture budget failures
   retain ordinary safety reports and their actual counters.

The Rust `Outcome` adds `NativeHolds(CompleteEvidence)`,
`NativeViolation(ValidatedNativeCounterexample)` and
`NativePrefixViolation(ValidatedNativePrefix)` (boxed in the public enum).
Private validated wrappers expose `word()`, `evidence()`, `document()` and
`validation()`; successful callers cannot construct them from an unchecked word.
Existing graph-owning variants remain available to reference callers. JSON
continues to report schema 3 and the existing independently validated trace
shape; no dummy graph or alternate completion flag is introduced.

Planning, graph-limit/conversion, analysis and validation failures still use
the core route's explicit refusal boundary. Matching actual capture budget
exhaustion or measurement failure retains that run's actual capture cause and
counters. Planning, factory, analysis and validation budget failures use the
existing detailed failure renderer, preserving actual stage and full/prefix truth.
Already-observed periodic failures precede post-stop factory polls. `--engine auto` retries the reference route for the named refusals;
it does not turn actual native budget stops into successful prefixes. Temporal
error-demand/report parity remains bounded by the source-row lane below (§11).

The optional source-error lane consumes a source-bound `OwnedTemporal` into a
hidden row driver. A borrowed `GraphCheck` supplies only callback-free eligibility
inspection. Native binding occurs at most once, at the first actual scalar source
event; compound roots do not bind or run reference
fairness work. Existing constructed native action evaluation remains independent.
This first slice requires exactly one actual initial emission and accepted root,
then attempts only the first actually reached source. An unproved or quiet first
row disables it permanently, so no earlier row/residual is skipped
to promote a later error; ordinary analysis continues with its existing boundary.

`GraphSink::source_row` and reference checkpoint transport distinguish a real
Next-error terminal boundary from successful source expansion. Terminal events
retain the original failure, accepted concrete targets and actual counters; they
publish no implicit stutter, closed row or complete/prefix receipt. Native
generation is locally reproduced against its actual scalar self inventory
before row predicates run. Candidate-check errors retain their earlier event
priority and do not become source-row errors.

Separate `SourceProgram`/`SourceCertificate` types preserve the old full-MUST
row cache. They reuse normalization, support one DNF solution's AEAction and
EAAction bins before state bins, and treat later state-only tableau work as an
opaque barrier. Actual row and diagnostic errors can establish bounded primary
suppression before that barrier, with no residual/holding truth. Only fresh
independent version5 source replay permits publication of GENERAL1000/255 and
the original finite word. Other primary, candidate-ending and parallel scopes
remain explicit refusals (#502/#678); candidate scope earns no new parity credit.

`Fallback::of` treats the temporal route's refusals like any other refusal,
so `--engine auto` falls back with the reason named. The JSON report gains
the core's `liveness` metrics under `core.metrics` (§4), which are not parity
observables.
All other fields equal the reference engine's, except the fingerprint
collision estimate, which the core does not report on any route.

## 7. Budgets and remaining capacity acceptance

The native backend is implemented; TLC-relative capacity acceptance remains
open. Ordinary core file requests use checked machine capacity for capture,
product edges/work, acceptance bytes/materialization and attribution/word cells,
under their actual run time and sampled RSS/swap budgets (ADR 0124). Parallel
capture retains the eight-source wave schedule. Formula/tableau/property planning
limits, explicit Rust limits and reference-library routes retain their exact
settings. No stop or refusal establishes a holding result.

Implemented storage now includes all-vertex/original persistent roots, CSR
metadata, eager controlled Graph admission, on-demand state access, graph-neutral
completion/results and native validated reporting (§7.1–§7.4). It reuses the
analyzer and independent validator. Conversion reuse is limited to the bounded closed-materialized-value cache (§4).
Raw/symbolic sets, nonscalar general domains and opaque values remain outside
that cache. Closed recursive functions with scalar domains and already stored
enumerated sets are eligible under §4; no cache may recreate an unaccounted
complete Value graph.

Remaining qualification and accounting work:

- Complete accounting for persistent Store allocations, explorer frontiers and
  speculative waves, native rows/CSR/labels, retained and growing key/index
  storage, transient conversion, labels and product/search memory. Native
  collector metrics cover buffers and action-label capacities, with growth
  overlap reported separately as an upper bound. Store accounting is partial;
  opaque hash tables, Arc/allocator overhead and other owners are not a complete
  whole-run byte proof. The old 12-byte/vertex plus 8-byte/edge figure describes
  topology only.
- Bound or expose remaining uninterruptible units, including constructor
  hashing, diagnostic rendering and final shared allocation. Whole-process
  RSS/swap sampling and time checks are cooperative guards, not allocation-time
  hard caps or measurements of every transient peak.
- Measure end-to-end native completion against pinned TLC on original temporal
  configurations with comparable recorded memory/time policies, actual worker
  widths, peak-memory observations, every parity observable and independently
  validated words. Use `09-performance-method.md`; partial exploration or a
  timeout earns no capacity/performance credit.
- Qualify the integrated milestone with the full gate. Incremental development
  uses focused controls and required CI under the current AGENTS.md policy.

L5 does not implement VIEW quotient edges, symmetry closing-state semantics,
temporal error-demand order, new temporal language or effects. Those remain
separate semantic obligations. Native storage alone supplies no corpus,
throughput or capacity credit.

### 7.1 Implemented first dependency: borrowed CSR topology

`dive_liveness::product::CsrTopology` validates borrowed offset, 32-bit target
and root slices without constructing a nested adjacency copy.
`Product::from_csr` and `with_csr_recurrences` use the same product builder as
the existing dense input. The internal compact product uses that topology view
too. The input view retains root/edge order and duplicates; product construction
keeps its existing canonical root handling. Empty rows stay dead ends, and
witness edge numbers remain original per-row positions. Existing
product limits and acceptance-buffer policies retain their dense-input results.
This view validates references, not producer completion or temporal acceptance.

The native checker now supplies this same product builder with topology
borrowed from checked `CsrCapture`, without constructing nested adjacency or a
`BehaviorGraph`. The standalone product API still accepts caller-supplied
labels; it grants neither producer completion nor model-validation authority.

Reachable-product construction now looks up exact `(behavior vertex, tableau
node)` pairs in a private hash table. Bucket order never selects a witness:
after discovery, the table is released and a fallible, budget-polled merge
orders retained discovery IDs in the existing Cartesian order. Its scratch
becomes the existing renumber vector. Roots, duplicate edges, original edge
positions, acceptance conditions and explicit product limits keep their
existing semantics. Index growth uses checked logical entry sizes and fallible
reservation; opaque hash bucket/allocator overhead is not a whole-process
memory metric. This optimization alone does not establish original ABA
completion or capacity acceptance.

### 7.2 Implemented state-value access prerequisite

The private `analyze::state_source::StateSource` separates representative values
and original concrete successors from graph topology. Reference analysis uses
`CapturedStates`, which borrows the existing `BehaviorGraph`. Native analysis uses `NativeStates`, mapping representatives
and original physical edge positions to checked retained-row ids and the
`NativeRead` callback. Reference entrypoints and their graph owners remain.
Representatives may borrow an existing row or own a lazily converted row.
Original metadata can be inspected without converting its values. Native
reads use the actual temporal control and preserve typed stop/error results.

`selected_targets` checks ordinal/label correspondence in original row order,
keeps the first original successor for each destination, and reads no later
duplicate's values. Implicit stutters use their source representative only when
that destination has not already selected an original. Labels retain per-atom,
per-vertex demand and action evaluation once per destination. Inactive action
atoms and empty frontier rows request no values. Lasso extraction reads only
the chosen representatives and retains original edge positions and metadata,
including through a row-cache projection. Source failures and typed stops
propagate as errors; they never mean absence of an accepting word.

Tests compare the reference adapter with a lazy owner of separate value rows:
complete packed labels, candidate states, edges, property attribution and
lasso positions agree in both profiles, including `ENABLED`, WF and SF. They
also cover differing originals for one destination, ordinal gaps, malformed
metadata, untouched atom-error priority, every demanded read's failure/stop,
and a mutation that incorrectly replaces original values by representatives.

Native capture now retains addressable persistent handle rows for every
vertex and differing original target, alongside `Arc<Store>` ownership.
`Step.original` and capture-only `Occ::Old` preserve exact new/seen originals
under the existing wave/source bounds (§3). Native storage uses the shared
labels, selected-target, product and lasso path; it does not recreate a reference
value graph. The private source grants no completion or resource authority of
its own. Complete byte accounting and measured capacity remain §7 obligations.

### 7.3 Implemented producer-fact and result assembly prerequisite

`capture::CompleteEvidence` retains the existing `GraphCompletion`, the actual
ordinary safety result, original options and counters without graph rows.
`PrefixEvidence` retains actual successful committed-prefix facts and its level,
with a nonzero queue and no completion receipt. Their constructors remain
private. `Complete::evidence`/`Prefix::evidence` borrow these immutable facts;
`into_evidence` releases graph storage while retaining them. Edits to public
capture fields do not change the original facts or recertify completion.

The reference producer already distinguishes a successful unexhausted committed
prefix from a safety failure or resource stop. The public `GraphBuilder`
factories now preserve that distinction too: any non-success safety result
returns its exact original verdict, before constructing either kind of evidence
(#624). Successful completion still uses the existing `GraphCompletion::finalize`
shape/counter checks; successful prefixes retain the nonzero-frontier check.
No CSR shape, copied counters or caller completion flag alone creates these facts.

Private liveness result assembly consumes the existing candidate word or error
and the matching kind of producer evidence. Only complete evidence can assemble
a holding decision. Prefix assembly requires a selected word, and analysis
errors and typed stops remain errors. Existing final-only and periodic paths
use this boundary. Their compatibility adapters keep the original public
`Complete`, `Prefix`, `Analysis`, candidate and API `Outcome` values, retaining
the original graph allocations without copying or rebuilding rows.

The same result carriers are exported narrowly through
`analyze::periodic::external` as `NativeFinal`, `NativePrefix` and `NativeWord`,
with `NativeAnalysis` distinguishing final and prefix results. Consuming
`Analysis::into_native`/`PeriodicAnalysis::into_native` moves the selected word
and actual sealed producer facts and releases the legacy graph. This supports
the native report boundary without creating an empty placeholder graph or
generifying the existing public analysis/report types. Candidates remain
unvalidated; reference entrypoints keep their existing return types.

Tests detach real producer evidence and observe graph rows being released,
preserve every selected word/edge and legacy graph allocation, reject malformed
completion counters/rows, preserve original factory failures/stops, and keep
successful joined prefixes at 1, 4 and 8 workers in both profiles. Public
rustdoc controls prevent constructing completion facts or promoting prefixes.

The production native path now consumes these facts and result carriers
through `CsrComplete`/`CsrPrefix` and `GraphCheck`'s native methods. Native API
reports retain selected-word values and complete or prefix facts, not graph
rows. Reference result variants retain their original graph owners. No capture
limit or supported model population changes merely from this separation.

### 7.4 Checked CSR completion boundary

`capture::CsrCaptureInput` borrows adjacency and opaque retained-row metadata;
`CsrCapture::checked` validates it under the actual `EvaluationControl`.
Closed sources have one final implicit stutter, while frontier rows are empty.
Generated duplicates, strictly increasing but potentially gapped ordinals,
action labels and separate original target rows survive checking. Original
positions index generated entries in a source row, not generation ordinals.

`CsrComplete::from_producer` also requires successful safety EOF, actual matching
statistics, an empty queue and every source closed. The completion header is
shared with the legacy graph factory. `CsrPrefix::from_producer` instead requires
a successful committed boundary with unfinished frontier and nonzero queue.
Both reject any original safety failure before inspecting shape and preserve
typed matching run interruptions. Their immutable facts can detach without
building evaluator state values or a reference graph. See ADR 0122.

Tests compare metadata and completion facts with actual captures, retain distinct
original rows behind duplicate targets and ordinal gaps, reject malformed arrays,
offsets, ordinals, labels, roots and stutters, and reject promoting a real prefix
even after its public queue counter is edited. A deterministically expired armed
budget produces an actual capture-phase stop.

These are low-level trusted-producer APIs. Shape validation does not evaluate
opaque rows, authenticate a supplied search history or validate a temporal word.
The production native collector now uses this boundary for complete and prefix
finalization. No capture cap, model-coverage or capacity/performance claim is
changed by checking adjacency and producer facts.

## 8. Tests

- `core_graph_tests.rs` retains the test-only legacy converting collector as a
  reference. `core_native_graph_tests.rs` compares native rows, CSR metadata and
  original classes, including new/seen aliases, ordinal gaps, prefix frontiers,
  malformed events and actual budget stops.
- `dive-liveness/tests/native_capture.rs` compares native CSR/readers with the
  existing external graph analyzer in both profiles: final and prefix words,
  action labels, state-atom failures, facts and scheduling. Private state-source
  controls check first-original demand, untouched duplicates, failed reads and
  original lasso edge positions.
- Controlled conversion tests preserve values, literal classes, name interning,
  comparison-error order and exact history refusals, and interrupt normalization
  at its checkpoints. NativeValues tests retain no converted rows and reject
  foreign/missing interruption authority and malformed variable dimensions.
- `dive-api/tests/core_liveness.rs` checks native/reference reports, both profiles,
  completed-level scheduling and independently validated words. Native report
  controls reject forged words and validation stops; existing temporal evidence
  and core-budget tests retain completion truth and actual stopping counters.
- The integrated milestone still needs current-source corpus accounting,
  original capacity targets and the full gate. Earlier corpus populations and
  performance measurements are not silently promoted to native-backend evidence.

## 9. Delivery status

| Scope | Implementation | Qualification |
|---|---|---|
| L1–L4 | Original producer adapter, shared analysis, scheduling and report path built | Historical evidence in §10 |
| L5 native backend (#638, #651) | Persistent original rows, CSR, controlled admission/on-demand reads, bounded flat-value reuse, sealed facts and graph-neutral validated reports | Focused native/reference and negative controls; required CI |
| L5 capacity | Ordinary core file capacity follows actual budgets and checked layouts; partial byte accounting remains explicit | Complete accounting, bounded remaining units, original TLC-relative capacity/performance measurements and integrated full gate remain open |

## 10. Census and results

These historical measurements precede the native L5 backend. They retain their
original source/population and do not establish its throughput or capacity.

**Before L4.** The core refused every genuine temporal property
statically ("temporal properties (M0 core engine)"), so under `--engine
auto` every such model ran on the reference engine.

**After L4** (2026-10-28, `--impl dive:auto`, small tier of examples,
probes, validation-smoke and configured-temporal-operators, 1 worker, TLC
budgets capped at 900 s; harness reports kept outside the repository):

| Profile | models | agree on the core | agree after a fallback | unsupported | disagree |
|---|---|---|---|---|---|
| default | 156 | 114 (+4 expected divergences) | 28 (+3 expected divergences) | 5 | 0 |
| `--compat=tlc` | 156 | 108 | 40 | 6 | 0 |

- 31 models with genuine temporal properties now run on the core and agree
  with the goldens in both profiles: 29 from the examples (among them
  CoffeeCan1000Beans, 501,500 distinct; TokenRing, 46,656; nbacg_guer01,
  24,922; ReadersWriters, 21,527; and MCRealTimeHourClock, a liveness
  violation) and the two configured-temporal-operators models.
- Indicative timing, CoffeeCan1000Beans at one worker on M3: 0.77 s and
  94 MB on the core, against 2.28 s and 134 MB on the reference engine.
  The conversion took 152 ms for 501,500 states.
- The temporal models still on the reference engine are refused for
  constructs the core does not run yet: `ENABLED` (6), `SYMMETRY` with a
  temporal property (P4SymLive), and other refusals of the safety route.

## 11. Risks and open points

- **TLC row order for atom errors.** On the reference engine, a run at one
  worker evaluates its state atoms during capture, at TLC's checkpoints
  (`analyze/row_demand.rs`), whenever:
  - the run owns its model;
  - labels are uncapped (`GraphLabelPolicy::NoCellLimit`);
  - every atom is a state predicate.

  An atom that fails to evaluate then ends the run where TLC would, with
  TLC's counts. The core evaluates atoms after capture (or at a due level).
  For holding and violated properties the result is the same, because the
  row cache only saves work. An atom error can differ, though.
  `GraphCheck::row_order` indicates whether such reference row demand may be
  active. The native core route currently refuses analysis/atom failures and
  `--engine auto` retries the reference route. Storage integration does not
  reproduce that earlier evaluation order or supply counts for it.

- **Label strings.** If a core action's label differs from the reference's
  `Successor::action.label()` (for example, split actions inside INSTANCE),
  the graph-equality test fails and the model is refused until the two
  agree. Labels are never derived independently.
- **Conversion work and memory.** The native path removes the retained
  evaluator-value graph. Bounded closed-materialized-value read reuse addresses repeated
  conversion; admission converts only first names and nodes it cannot prove,
  and unproved classes still reconvert on demand; transient source/target rows and analysis
  allocations still coexist with persistent storage. Their cost and remaining
  accounting need measurement before extending cache eligibility or claiming
  original-workload capacity.
- **Symmetry and VIEW liveness.** A temporal property with `SYMMETRY`
  is refused for now; it will follow the reference's `symmetry_liveness`
  resolution. SYMMETRY under `--compat=tlc` (#574) and VIEW with SYMMETRY
  (#576) run on the safety route.
