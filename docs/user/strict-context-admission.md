# Strict substitution and independent trace admission

Status: narrow source repair implemented and focused regression checks pass;
complete receiving gate and combined-runtime qualification are pending.
The earlier effect observations below belong to the retained before producer.

`validate_doc` and `temporal::evaluate_lasso` inspect every eager substitution
operand before `scope_subst` can execute it. Closed literal/tuple/record structure
and actual stored or lazy constant/local forwarding can be admitted without
evaluation. Native/user calls, state/prime reads, providers and opaque contexts
retain the named refusal. A caller's serialized expression inertness flag does
not authorize effects during independent admission.

For example, a Rust caller can build an `Expr::Subst` around Next with a binding
`C <- Noise("")`, where `Noise(v) == PrintT(v)`, and mark the copied call node
`Effects::NONE`. With a structurally owned, admitted context, the old validator
returned `Valid` after three Print events; the old formula endpoint returned
`Unsupported` after one. These were actual effects from public caller IR,
not a false trace or a naturally lowered TLA+ parity result. The endpoint
repair is tracked separately as [#234](https://github.com/Chelis-Lang/dive/issues/234).

The guard uses actual owned structure and captured values, never an inertness
annotation or supplied word. It shares 512 visits per inspected substitution,
depth 64, at most 512 bindings and 4,096 bytes per inspected literal or name.
These are admission policies, not exact CPU or total-memory bounds. Unknown,
exhausted or foreign contexts refuse before entry. Definition calls (including
configured redirects), general functions, sets/providers and stored big integers
are outside this narrow eager proof. Cached numeric literal errors may still
fail ordinary entry: admission proves no truth or unconditional success.
String literals and record field names are planned by evaluator construction;
admission does not intern them. Forwarded atoms must match the receiving token.
Every binding is inspected in the same original outer scope, matching
`Core::bind_subst`, rather than an evolving substitution scope.

Ordinary evaluator entry retains Strict evaluation. Genuine closed literal
Strict operands retain independent validation, including nested
`MC <- TRUE; C <- MC` forwarding. Lazy and Direct expressions remain thunks,
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

The blanket-refusal receiving focused suite passed all four controls. Its original identity-call
fixture failure and the subsequent stale-build reuse are retained separately;
only the final actual compile receives corrected-source credit. Combined gate
and runtime qualification remain pending.

The first combined blanket-Strict producer passed 1,762 of 1,764 tests and
failed two existing temporal substitution controls. That complete failure epoch
is retained. The narrow structural repair restores the unchanged pinned instance
and nested state/priming assertions. Both previously failing tests, the four
endpoint controls and three private proof controls pass in the focused patch.
After a rustdoc backtick correction, targeted Clippy and all 49 tests in the
complete temporal/Strict API targets pass, with none skipped. The original
failed epochs are preserved. Complete receiving qualification remains pending.
