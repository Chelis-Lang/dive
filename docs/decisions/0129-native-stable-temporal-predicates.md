# 0129: Evaluate stable temporal predicates on native original rows

- **Status:** accepted
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
read closed integer-domain functions and compare scalar values. Its fairness
wrappers remain separate reference demands. Original EWD's predicate closure
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
forcing or normalization. Raw/deferred set history, function constructors, CHOOSE, EXCEPT,
ENABLED, live TLCGet and effects remain unproved. Actual input handles must
independently satisfy the closed set-free shape on every call.

Use a separate analysis Machine, arena, frame and memo owner. Disable analysis
memoization and folding; no native evaluation interns results into the producer
Store or warms its worker caches. Existing strategy switches otherwise apply.
Bound optional code and input inspection to 4,096 nodes/components and machine
loop work to 4,096 units; exponentiation and recursive code stay on reference
execution. These optional limits and any semantic
failure abandons only the optional native attempt and runs the existing
reference predicate. Actual run interruptions are polled before and after work,
validated against the producer and analysis control owners, and propagated with
the analysis phase. A foreign interruption cannot become this run's stop.

Every original still undergoes the same eager Graph admission, including seen
states. Native errors supply no terminal proof or TLC error-class authority.
Reference attribution and fresh independent trace validation remain mandatory;
all native graph, converter and analysis owners are released before fresh word
replay. No backend, mode, dependency edge or semantic divergence is introduced.

## Consequences

Report additive prepared/consumed/fallback atom counts. Preparation coverage is
not throughput or completion evidence. A diagnostic over original ABA may use a
smaller public periodic schedule to reach actual consumers quickly; that earns
no default-profile performance or capacity credit. Original capacity workloads
and quiet paired performance qualification remain separate requirements.
