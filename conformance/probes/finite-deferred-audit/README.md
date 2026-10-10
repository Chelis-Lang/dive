# Finite deferred-domain and recursive scope blockers

The original six tiny specs reduce two refusal paths observed in the complete
[ten-model study](../../../docs/lazy-domain-study.md). The initial captures
diagnose the refusals; the subsequent bounded-filter implementation is
accounted for below. Resource
defaults and strict conformance claims are unchanged. Goldens are unchanged.

| Pair | Pinned TLC | Initial dive source `2b305ee` | Source path |
|---|---|---|---|
| `FdFilterHolds/Fails` | hold / liveness violation | both refused at bounded bindings | Function-set filter captured as `lazy::Filter` |
| `FdMapHolds/Fails` | hold / liveness violation | both refused at the inner filter binder | Set map over a filtered function set |
| `FdRecursivePermHolds/Fails` | hold / liveness violation | both refused at opaque scoped level | Recursive LET function `perms[ss]` |

All six pass SANY. Every hold has 3 generated states, 2 distinct states and
complete depth 2; every violation has 2 generated states, 1 distinct state and
depth 1. TLC exits 0 for each hold and 13 for each violation. The clean raw
transcripts and separate, cross-checked JSON trace captures are retained under
`tlc-2026.10.06.014338/`.

## Exact diagnoses

`Core::eval_filter` keeps a filter over a structural function set as a
`DeferredKind::Filter`, retaining the input value, predicate expression, scope
and state. The initial `Budget::set` refused every deferred domain before
`size` or exploration could invoke it. The first pair exercises this guard; the
map pair reaches the same guard while preflighting its inner binder. Quicksort's
`Automorphisms`/`PermsOf` and YoYo's captured `keep` filter have these source
patterns. The reductions do not establish that every path in those full models
is supported once this guard is extended.

`Planner::scoped_level` admits a local slot represented as a value,
analysis-bound variable or lazy expression, but refuses a local recursive
function. The recursive pair retains the allocator's `PermSeqs` recursive LET
function and reproduces the exact refusal at the recursive local function name.
This is a scoped-level capability in #63, separate from finite set admission
in #107. A future change must preserve the function's captured domain/body,
recursion limit and state-dependent levels rather than treating it as a constant.

## Next implementation controls

Finite filters must preflight their input cardinality and nested materialization
before evaluating the predicate, then preserve the original normalization,
enumeration and profile order. A captured filter retains its creation state.
Fresh evaluation and priming must use the actual state/edge. Predicates must
not be duplicated or effects silently admitted. Infinite/opaque inputs and
oversized work remain explicit refusals. Filter-result cardinality may use a
conservative input upper bound. Set operations and recursive function bodies
need separate bounds; this evidence does not authorize arbitrary deferred sets.

Before claiming support, both pairs must pass in both profiles and stores, with
fresh independent validators accepting the alternating Boolean word and
rejecting its stuttering mutant. Recursion/scoping and infinite/oversized negative
controls are also required. Re-run the full ten-model census with unchanged
limits and retain the next blockers instead of counting a passed guard as parity.

## Reproduce

The TLC harness source is `2b305eea34e5726d16cfa680bcd1ee2691852afc`.
The pinned oracle, jar and module-library bundle match the ten-model study;
the command captures seed 0, fingerprint polynomial 0 and one worker. Use an
independently copied verified cache, as described in the harness README.

```sh
HARNESS=target/release/dive-conformance
"$HARNESS" dev capture --oracle tlc-2026.10.06.014338 \
  --specs conformance/probes/finite-deferred-audit/specs \
  --out conformance/probes/finite-deferred-audit \
  FdFilterHolds FdFilterFails FdMapHolds FdMapFails \
  FdRecursivePermHolds FdRecursivePermFails
```

The reduced dive refusals were observed using that same source's binary,
SHA-256 `e9e880ce5326c2688df9e8071103f0b8547135679ef221d5b9252777c924a486`,
with `check --compat=tlc --output json --workers 1 --seed 0 --fp 0 --lncheck final
--lasso short`, the respective spec and its config. The raw generated diagnosis
is `dive-source-2b305ee.json`, SHA-256 `4a2120dfed78dfb330beaea41667e89604a6caf9fec1de70b7213f8f6a0b92d3`.
Its failure-worker metadata predates #109 and is not credited as execution
or parity evidence. All six observations remain named unsupported results.


## Bounded captured-filter implementation

`FdFilter*` and `FdMap*` now match final truth and complete counts under both
profiles and stores. The additional `FdCaptureHolds/Fails` pair puts
`Domain == {f \in [1..2 -> BOOLEAN] : f[1] = x}` inside a temporal atom,
`[]<>(\A f \in Domain : f[1] = FALSE)`. It detects a domain frozen at planning
time or reused across different concrete states/periodic closing edges. These
new captures use the same pinned clean oracle and have the same hold/violation
counts and exits as the original six.

The source predicate is retained with its original IR span:
`FdFilter*` line 4, columns 38–49 contains `f[1] = FALSE`;
`FdCapture*` line 4, columns 38–45 contains `f[1] = x`;
`FdMap*` line 4, columns 52–63 contains the inner `g[1] = FALSE` predicate. Preflight inspects
only structural input/cardinality and closed purity; it never replaces,
evaluates or caches those expressions. Original predicate evaluation and
profile-specific exploration determine errors and order. Non-filter deferred
sets, unresolved/effectful predicates and infinite/work-heavy inputs refuse.

`crates/dive-api/tests/deferred_domains.rs` runs six supported specs in 72
profile/store/requested-worker settings (2 × 2 × 3 × 6). Actual final capture
remains one worker. Every violating report validates independently; fresh
independent property evaluators accept the alternating Boolean word and reject stuttering
at TRUE. The recursive LET pair remains explicitly refused in both profiles.
Evaluator controls separately verify original enumeration/error order, budget
checks before predicate invocation, direct and bound-redirect effects-before-Print
refusal, cached-input
bounds, captured caller state and primed re-evaluation.

This is focused final/short evidence for the F-007 domain family, still partial
and audit-required. Recursive scoped-function support in F-006/#63 is unchanged.
The initial `dive-source-2b305ee.json` and the complete ten-model report remain
immutable historical evidence with all refused/timed-out rows retained. A new
whole-selection report is required before crediting those corpus rows.

The two additional captures were generated with the command above, selecting
`FdCaptureHolds FdCaptureFails`. No golden file was edited.


## Opaque Rust operator captures

A public Rust caller can supply an operator through `scope_apply` that does not
appear among the IR's analysed call sites. The retained source
`rust-operator-capture-before.rs` injects `TlcPrintT` into `Q(P(_))`. Before the
fix, `Q` has only `MAY_FAIL` effects, yet bounded binding returns two scopes and
prints two values. The exact pre-fix output is retained in
`rust-operator-capture-before.stdout`; this is a native purity-control repro,
not an oracle capture, a golden or a checking result.

The repro linked source `25290f9b070fdc9bce0d55f8139ab7203c82b0a0`; its standalone
executable SHA-256 was `ad584c9ad52ecff5a650b0d1c44dfbac1656f357e6fa01bc745dd5b0f3765aba`.
The fixed `dive-eval` regression
`deferred_filter_rejects_operator_arguments_injected_by_rust_callers` verifies
explicit refusal and no output in both profiles. Captured operator slots remain
opaque, including pure operators supplied by Rust, until scoped effect proof
exists. Closed effects are also required for all captured thunks/functions.
The environment's existing shared-side-effect test retains its original
behaviour; only the new captured-filter guard applies these stronger conditions.
This is an F-006 limitation retained while widening F-007, not higher-order
implementation credit.


The companion `rust-invalid-expression-before.*` control supplies an unchecked
`ExprId(u32::MAX)` as an unused captured expression through `scope_apply`. It
previously produced two bindings because missing capture metadata was treated
as pure. Filter admission now treats missing metadata as unknown and refuses
without evaluating the invalid expression or panicking, in both profiles. The
old shared-side-effect helper's missing-node behaviour remains unchanged. This
is conservative filter proof, not a general validation of Rust caller-created
IR identifiers. The fixed regression is
`deferred_filter_rejects_missing_captured_expression_without_panicking`.
