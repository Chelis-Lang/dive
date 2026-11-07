# 0129: Evaluate stable temporal predicates on native original rows

- **Status:** accepted; input-row admission amended by [0154](0154-native-atom-rows-by-materialization.md)
- **Date:** 2026-10-30

## Context

Native temporal capture retains exact original persistent handles, but analysis
converts them into reference Values and evaluates every predicate through the
reference evaluator. Closed-function read reuse removes most conversion work on
EWD998Chan; its measured remaining work is predominantly reference evaluation.
Compiler acceptance or a purity flag alone does not prove replacing those calls
preserves shared reference constructor, normalization, name or thunk history.
The existing canonical producer certificate also deliberately excludes general
function storage and reordered interval domains.

The original aba_asyn_byz configuration supplies a smaller useful boundary:
its six ordinary state predicates quantify over an immutable integer interval,
read closed integer-domain functions and compare scalar values. Its fairness wrappers originally remained separate reference demands. A bounded
follow-up covers their actual successor search and single-key scalar updates;
the original predicate-only experiment still stopped at 900 seconds on ABA
and showed only approximately six percent on CoffeeCan1000, so it did not earn
inclusion by itself. Original EWD's predicate closure
additionally constructs functions and uses CHOOSE/EXCEPT, so remains outside
this boundary.

## Decision

Reuse the existing core Role/Handoff compiler and Machine through an optional
native reader capability. Legacy reader-only entrypoints and reference behavior
remain unchanged. The shared analyzer retains atom/source order, first original
successor selection per destination, frontier behavior, labels, products,
lassos and reference property attribution. A producer maps the actual retained
representative and first original target row IDs to their exact handle slices;
no extensional-equality substitution or reconstructed identity state is used.

Only the actual PredicateAtom objects retained by the still-live GraphCheck
bind compiled code. Validate their actual evaluator/IR/Interner scopes through
the callback-free stability inspector. A separate closed-predicate inspection
admits immutable intervals and stored closed functions with scalar domains,
retaining the old producer certificate unchanged. It checks captured scopes,
configured redirects/constants, cached bodies, source transfers, effects and
name ownership. General functions require the run's domain ordering; scalar
keys and recursively closed tuples/records/functions contain Bool/Int/run-owned
string leaves. Inspect every nested function's context and the actual stored
scalar-key sequence: keys must be strictly increasing in the run's order.
Lossless decoding, a matching order label or shape/byte inspection alone supplies
no canonical-domain fact. Unsorted or duplicate domains decline without sorting,
forcing or normalization. Raw/deferred set history, function constructors, CHOOSE,
live TLCGet and effects remain unproved. Actual input handles must
independently satisfy the closed set-free shape on every call.

For retained Enabled and Nonstuttering atoms, inspect the action and subscript
in their actual original scope. An additive bound-test compiler entrypoint uses
the same Role/Handoff, Compiler::enabled, generator and Machine value evaluation.
The existing enabled_substs identity/cutoff proof and g_free effect/call refusals
remain in force (semantic contract B2–B4). Enabled clears step-mode definition
caching and searches the generator with a fresh successor buffer, preserving
partial-first-success and B5's uncached generator LET/substitution behavior.
A fairness subscript installs the existing generator Changed continuation; it
must reject stuttering candidates and continue searching. Nonstuttering first
evaluates the subscript in source and target order, returning false on equality,
then evaluates the action. The existing value StepAngle action-first order is
not substituted for that contract.

The action inspector separately permits one EXCEPT clause with one path key;
this is no standalone update certificate. The bound-test compiler additionally
requires an unprimed root-variable base and a key that resolves to an actual
flat binder slot with an immutable, state-independent domain. Its ClosedUpdate
node confirms at invocation that this domain is an integer interval and the
ACTUAL input function has exactly that domain and contains the key; only a
scalar Bool/Int/run-owned-string replacement may succeed. FcnCons, CHOOSE,
EXCEPT @, nested/multiple updates, non-root bases, raw sets, hidden/nonidentity
INSTANCE queries and unknown source/value histories remain reference demands.
Missing/mismatched domains decline before evaluating the replacement: reference
EXCEPT can succeed unchanged while emitting warning 2144, so closed output alone
would not license skipping it. Existing function range replacement preserves
keys, order and tuple/record class, with no normalization/name/provider action.
Optional warnings/printed events also require reference fallback.

This generic rule covers the original ABA fairness action's interval-bound
single-key scalar updates without spec/config/variable-name whitelists. Coffee's
direct ENABLED Termination also qualifies; its fairness Next uses record/@
updates, and EWD uses source FcnCons/CHOOSE, outside this decision. Prepared and
consumed coverage on original sources must be measured; no descriptor count or
short diagnostic schedule establishes a speed or capacity improvement.

Use a separate analysis Machine, arena, frame and memo owner. Disable analysis
memoization and folding; no native evaluation interns results into the producer
Store or warms its worker caches. Existing strategy switches otherwise apply.
Bound optional code and input inspection to 4,096 nodes/components, including
owned string payload bytes in closed inputs/constants. Precharge Len's safe
UTF-8 byte-length upper bound before scanning for the actual UTF-16 result.
Machine semantic traversal/copy work has 4,096 units; this is not a hard CPU,
allocator-latency or wall-clock bound on opaque primitive operations. Explicit
loops are not exempt. Unproved long lexical input/constant payloads use reference
fallback. Exponentiation and recursive code stay on reference
execution. These optional limits and any semantic
failure abandons only the optional native attempt and runs the existing
reference predicate. Finite optional generator binding uses existing set cursors, preserving the
first-variable-fastest order and partial first success. Before existing Heap
normalization/cursor constructors can materialize a domain, a bounded preflight
charges component cardinalities and payload bounds; large/nested products decline
before allocation. Universal contexts and continuation payload are charged before
growth and unwind on exhaustion. The bounded Machine owns central normalization,
eager enumeration, materialization-cache and cardinality boundaries; every
interpreter, generator and library call to those Heap primitives routes through
them. This includes Subseteq, set algebra/UNION, domains, library enumeration,
candidate normalization and membership's materialization option. SortSeq/SelectSeq
charge tuple copies before allocation. Interval/eager-set/product and record-set cursor shapes with bounded component
domains have preflight bounds; other eager symbolic shapes decline before their Heap
primitive. Record-set preflight also serves the existing constant folder: known
cost exhaustion consumes its fuel and reports an abandoned fold; an unproved
shape stays unsupported without fabricated exhaustion. Source record-set
constructors remain outside the optional atom certificate. Optional materialization stays in the analysis arena, never interning
its result in the producer Store. Ordinary unbounded machines keep their existing
enumeration and caches. Actual run interruptions are polled inside query work through a
small core checkpoint backed by the original retained control, as well as before
and after work, validated against producer and analysis owners, and propagated with
the analysis phase. A foreign interruption cannot become this run's stop.

Every original still undergoes the same eager Graph admission, including seen
states. Native errors supply no terminal proof or TLC error-class authority.
Reference attribution and fresh independent trace validation remain mandatory;
all native graph, converter and analysis owners are released before fresh word
replay. No backend, mode, dependency edge or semantic divergence is introduced.

ADR [0130](0130-closed-ordinary-action-atoms.md) extends only the ordinary
Predicate/Nonstuttering construction and scalar-update subset. Enabled retains
the identity/cutoff and immutable-binder update boundary described here.

## Consequences

Report additive prepared/consumed/fallback atom counts. Preparation coverage is
not throughput or completion evidence. A diagnostic over original ABA may use a
smaller public periodic schedule to reach actual consumers quickly; that earns
no default-profile performance or capacity credit. Original capacity workloads
and quiet paired performance qualification remain separate requirements.
