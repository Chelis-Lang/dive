# Finite deferred-domain and recursive scope blockers

These six tiny specs reduce two refusal paths observed in the complete
[ten-model study](../../../docs/lazy-domain-study.md). This is diagnosis before
implementation. No new domain support, resource default or conformance claim is
introduced by these captures. Goldens are unchanged.

| Pair | Pinned TLC | Current dive final/short | Source path |
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
and state. `Budget::set` deliberately refuses every deferred domain before
`size` or exploration can invoke it. The first pair exercises this guard; the
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
enumeration and profile order. Captured state must stay live, and predicates must
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
