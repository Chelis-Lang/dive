# Bounded scoped operator admission

TLC Tool.getOpContext / Context lookup retain an operator's original parent and
substitution environment. The new Evaluator::scope_operator_analysis resolves
only an admitted known nonrecursive user target; it does not invoke operators.
All ordinary value formals become AnalysisBound for this proof only. Native,
builtin, overridden, unresolved and higher-order targets refuse. Configured
ConstBinding::Def provenance is erased by OpVal::Def, so every configured target
(including an explicitly passed same def) conservatively refuses in this slice.
No claim of original operator provenance is inferred from spelling or effects.

Every supplied operator is inspected even when unused. The callback-free
scope_operator_captures projection traverses retained lexical, substitution and
thunk environments, charging the caller's remaining work before contexts, slots
and outputs are retained. Known operator bodies use analysis-only value formals;
lazy expressions retain real caller slots. An opaque function/native/configured
operator refuses. Independent walkers prove every projected expression; summary
effect flags alone never authorize hidden callbacks. Stored values are not
forced by this projection and it is not a standalone value-purity certificate. Planner and independent
Word walks separately inspect its body/captures and native callback authority,
then resolve each actual application using scope_apply_param with real arguments.
Unknown value slots cannot be forced or exported as runtime truth. Literal,
state and temporal actuals remain in their actual caller scope; priming and the
periodic closing edge therefore see that caller's successor. Old exported
INSTANCE temporal-expression formals remain refused; only actual accepted pin
operator-formal controls receive credit. Recursion and opaque callbacks refuse.

The finite guard uses analysis-only scopes without materializing unknown binder
domains, then retains actual runtime finite-binding enumeration and original
ceilings. Capture proof retains shared50k expression/value work and128 depth.
Planner/Word/formula, ownership and context limits remain unchanged. These are
structural entry/work budgets, not aggregate memory/byte or evaluation-time caps.
The accessor's configured-target membership inspection is bounded by admitted
constructor metadata; it is not presented as an aggregate metering improvement.

No new backend, schedule, defaults, dependency edge or public graph authority is
introduced. Pre-bind admission continues to inspect all rooted user/constant/role
bodies and actual module-context aliases before host callbacks. Bound context
proof supplies the final authority for formal calls; a structural Param alone
has no runtime or model-proof credit. Supplied value-order/namespace safeguards
and all old material before/refusal/corpus evidence remain on the9b producer.

Capture traversal keys distinguish the substitution head and cutoff as well as
the lexical frame. Owners remain retained until traversal ends, so allocation
addresses cannot be reused as proof identity. Dynamic `@` contexts conservatively
refuse; value hashing/equality is never capture authority.
