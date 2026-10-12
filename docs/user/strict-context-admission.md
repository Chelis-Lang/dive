# Strict substitution and independent trace admission

Status: source repair implemented; fresh receiving compilation, regression
execution and full qualification are pending. The observations below belong
to the retained before producer, not a passing repaired runtime.

`validate_doc` and `temporal::evaluate_lasso` refuse an inspected substitution
containing an expression passed as `Pass::Strict`. The named refusal occurs
before `scope_subst` can execute its binding. A caller's serialized expression
inertness flag does not authorize effects during independent admission.

For example, a Rust caller can build an `Expr::Subst` around Next with a binding
`C <- Noise("")`, where `Noise(v) == PrintT(v)`, and mark the copied call node
`Effects::NONE`. With a structurally owned, admitted context, the old validator
returned `Valid` after three Print events; the old formula endpoint returned
`Unsupported` after one. These were actual effects from public caller IR,
not a false trace or a naturally lowered TLA+ parity result. The endpoint
repair is tracked separately as [#234](https://github.com/Chelis-Lang/dive/issues/234).

The guard conservatively refuses even an actually inert Strict expression;
its annotation is not an independent proof. Ordinary evaluator entry retains
Strict evaluation. Lazy and Direct substitution expressions remain thunks,
and an unused lazy binding is not forced by the tuple-history proof. An
unresolved opaque substitution in independent existence search remains
`Unsupported`; refusal is never a proof of disabledness.

The controls in
`crates/dive-api/tests/strict_context_admission.rs` use fresh valid-owned Models
for all three endpoints. They require typed refusals, zero Print events/output,
and identical ordered name tables. Separate controls retain lazy substitution,
parsed tuple-word validity, invalid altered steps, and deliberate ordinary
Strict evaluator behavior. They do not borrow a checker successor enumerator,
raise limits, or turn unsupported contexts into certificates.

The fresh receiving focused suite passes all four controls. Its original identity-call
fixture failure and the subsequent stale-build reuse are retained separately;
only the final actual compile receives corrected-source credit. Combined gate
and runtime qualification remain pending.
