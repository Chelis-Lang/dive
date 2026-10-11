# Bounded concrete capture proof

The pinned primary pairs all execute: eight hold (exit 0), eight violate (13).
The unchanged frozen scoped producer refuses every one at widths 1 and 4,
with zero native checking/validation credit. Those raw outcomes remain immutable.

The next slice changes only the capture purity proof's treatment of values.
Admission already checks IR, evaluator/model ownership, scope context and atom
text/token membership in the admitted evaluator interner (`context.rs::atom`).
Keep that authority before value inspection; public caller data are not native
callback authority merely because a static annotation says pure. Also check the
retained interner's resolution while visiting strings, model atoms and record
keys; inspect no values to guess purity by execution.

Only these callback-free stored views are candidates:

- Scalars: bool/int/bigint and admitted strings/model atoms.
- Concrete functions: tuple, record and general stored domains/values; these
  cannot contain a function provider, but nested set providers still refuse.
- Explicit Enum/Raw sets from `Set::stored_elements`, with recursive values.
  A deferred provider's cached enumeration remains deferred and refuses.

Do not call normalization, membership, hashing, fingerprinting, equality,
`DeferredSet::key`/`elements` or native callbacks to prove material purity.
Constructor-time callbacks happen outside this stage and must be recorded as
baseline: arm negative providers only after successful construction and demand
zero additional callbacks. This is not a stronger constructor safety claim.

Recursively charge the existing predicate proof's 50,000 work and 128 depth
ceilings before visiting each value/key/component. No resource defaults change,
no aggregate allocation/bytes guarantee, no wider opaque/effectful operator
capture admission. Treat unknown representation or exhausted proof as named
refusal, never a guessed Boolean or a model success.

Required implementation controls: all 16 paired native final results at four
widths, exact reference statistics/selection and fresh independent memory/JSON
witness replay; nested opaque cached-provider refusals with callback counters;
malformed model owner/interner and foreign atom controls; exact recursive work
and depth boundary negatives; original action/caller scope and purity controls
unchanged. Same-increment runnable CLI/Rust documentation and current source
receipts are required. The historical full140 census remains6agree/134unsupported
until a fresh unchanged full selection is measured.

Implementation also resolves potential OPEN only through existing scoped
constant/local/substitution dispatch. The IR effect flags are not cleared.
Unbound/Unevaluable leaves and unresolved Param/Subst/Unbound operators explicitly
refuse rather than passing children().all vacuously. The first64 native rows
(32completed/32OPEN-bearing refusals) are retained in development-open-refusals.
