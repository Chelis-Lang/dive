# 0101: Independent query runtime contexts and weak function memo ownership

- Status: superseded by 0103
- Date: 2026-10-17

## Context

The independent complete-cover evaluator recreated deeper lexical contexts for
each complete candidate. Pinned primary lifetime controls show that a LET
reached before hidden membership retains one lazy binding across its choices,
while a LET reached after membership creates one binding per choice. Leading
context expansion cannot express both. The inverted invariant control exposes
false acceptance when this lifetime is lost. Independent replay must obtain
its own evidence from original source and must build without enumeration.

A pure invocation also needs to retain recursive lazy function results without
materializing them. A memo owning a function whose captured St owns that same
memo would form a reference cycle. Refusing every lazy function regresses
previously supported finite recursive domains.

## Decision

Keep source purity and necessary finite-cover preflight independent. Execute
bounded queries over partial named assignments in source order. Retain pending
predicates with their actual scopes across nested choices; create LET and call
contexts only when reached. No checker generator, normal cached result, witness
or admission proof supplies a result. The existing unbudgeted public finite
cover API retains its original admission and work accounting.

Each pure invocation owns a bounded evaluation memo. A captured lazy function
shares the original work counter but holds weak references to that memo and any client resolver. Memo
lookup requires the actual thunk identity and compatible root-state projections,
control and resolver. Function storage is inspected without executing providers
or materializing functions. Retained ordinary cache owners are inspected only
for reference safety, never reused to establish query truth. Deferred provider
values and functions retaining an owning memo or owning client resolver refuse.

## Consequences and validation

Runtime binding order and lexical lifetime are explicit. Partial states also
preserve the primary's reached-error behavior rather than reading a future
assignment from a complete candidate. Branch/copy/traversal work is bounded;
this is not a total byte-memory guarantee. Function values may retain lexical
frames until the bounded invocation ends, but cannot keep that invocation's
memo alive. Reusing a function after its memo owner ends is an explicit refusal
if it needs another memoized force.

The separate 24-model lazy-query-lifetimes corpus was generated from the pinned
primary before this runtime change. Genuine and fabricated endings must be
checked through typed and serialized trace APIs in both profiles. Established
recursive-domain, query budget, source-order and cache-isolation tests must
continue passing. Focused execution, a fresh independent adversarial review,
the full gate and the established corpus remain required before acceptance.
