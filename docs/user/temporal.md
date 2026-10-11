# Temporal checking

The exposed checker is an explicit bounded final/short mode:

```sh
dive check docs/user/examples/Counter.tla --config docs/user/examples/CounterTemporal.cfg --lncheck final --lasso short --seed 7 --output json
```

The example reports a real liveness violation. Although `Next` always increments
before four, TLA+ temporal behaviors can stutter at zero; there is no fairness
condition in this model requiring the increment action to occur.

This mode explores the supported graph, checks once at the end, chooses a
deterministic short periodic witness and independently replays it in memory and
as a lossless document. It checks the complete configured property claims,
including the closing edge. The flags are explicit because this is not TLC's
periodic schedule, exact selected lasso or all evaluation-error timing.
A TLC value profile plus these flags is reported as `tlc+overrides`.

Pure supported boolean/temporal operators, finite quantification and WF/SF
predicates have scoped limits. Unknown ENABLED/fairness covers, runtime effects,
dynamic/exported scopes and other unproved constructs refuse. Ordinary strict
`dive check` retains its temporal refusal. A diagnostic planning/tableau/product
API that completes has not issued a temporal verdict.

Schema 2 distinguishes opt-in temporal checking from ordinary safety schema 1.
`properties` names complete claims, and periodic counterexamples carry validation
and a closing edge. Read [the temporal API contract](../temporal-api.md) for the
Rust `temporal::check` example, supported forms, limits, source-qualified errors
and indexing conventions. The separately exposed bounded pinned final-only
contract is documented in [the final TLC guide](../tlc-final-api.md).

For Rust tool builders, the separate [ordered execution API](../temporal-execution-api.md)
retains concrete graph and deciding-prefix evidence. Its completion is explicitly
not a temporal verdict and it is not exposed as a CLI checking mode.

The explicit command `dive check --compat=tlc --lncheck final --lasso tlc`
uses bounded final-only pinned selection and schema 2, with a private witness
certificate followed by fresh independent replay. It does not enable ordinary
periodic TLC scheduling. Only the separately named `dive-final-tlc` harness
mode evaluates that contract. Its first full small-model census refused every
selected model; focused-control success must not be presented as broad parity.
Read [the final TLC API/CLI contract](../tlc-final-api.md) for complete flags, bounds, workers, refusal classes,
property identities and stuttering/loop report obligations.

Models with no implied temporal obligations use the actual ordinary safety engine
inside the final mode's unchanged capture ceilings. The report names the
`bounded-safety-only` route, keeps the real stopping/completion and worker facts,
and explicitly states that no temporal product or certificate was constructed.
A finite counterexample must pass fresh independent replay before earning
violation credit. A resource stop is never a safety success. See the
[safety-only route contract](../tlc-final-api.md#safety-only-route).

[Pure conditional final checks](../tlc-state-if.md) documents the admitted
state-IF subset, executable CLI/Rust examples, replay and remaining refusals.

The proven direct temporal-IF subset now runs through bounded explicit final/TLC
checking with fresh independent witness replay. Read the
[temporal conditional guide](../tlc-temporal-if.md) for executable CLI/Rust
examples, unchanged bounds and refused forwarding/early-witness combinations.
Ordinary periodic scheduling remains unavailable.

For caller-context forwarding and property names, see the
[scoped final guide](../tlc-final-scopes.md).

The [expanded property identity guide](../tlc-property-origins.md) explains
configured-before-split names, expanded original Action names and derived role
labels exposed by completed bounded final TLC results. These metadata fields
do not replace independent witness replay or add ordinary scheduling support.

Stored scalar and composite action/constant captures are described in the
[concrete capture guide](../material-capture.md), with runnable CLI/Rust examples,
recursive admission limits and deferred/provider refusals.

Pure user operator formals in bounded explicit final TLC mode retain local and
INSTANCE scopes; see [operator arguments](../temporal-operators.md). Native,
configured-origin-ambiguous, recursive and opaque targets remain named refusals.

Bounded direct `Len`, `Head`, and `Cardinality` operator actuals are described in
[Pure native temporal operator arguments](../native-temporal-operators.md), with
executable CLI/Rust examples and retained refusal boundaries.

See [Stored successor values](../stored-successor-material.md) for concrete set/function admission, namespace/order requirements and provider refusals.

[Fixed-arity Append operator arguments](../native-sequence-operators.md) retain
both operand scopes; Tail/SubSeq runtime-name admission remains pending.

The [partial SCC diagnostic guide](../partial-temporal-checks.md) describes the
separate Rust checkpoint API. A partial check can return a candidate requiring
fresh independent replay, but never temporal Holds or an ordinary schedule
certificate. Original graph/partial inspection bounds remain shared.

Rust backend authors can inspect source-owned graph growth and explicit partial stopping with the [periodic backend diagnostic](../periodic-backend.md). This API does not enable ordinary clock scheduling, Java queue suspension, partial Holds or a model-checking verdict.


The [native lifecycle diagnostic](../native-periodic-lifecycle.md) connects internally sampled clocks, real dispatch pauses and actual EOF final checking. Its commit-boundary polling does not certify TLC’s asynchronous ordinary schedule.

[Pure supplied periodic expressions](../periodic-hook.md) can be composed diagnostically at real native pauses. Ordinary configured hook binding, checkpoints and scheduler verdicts remain refused; no TRUE hook result means temporal Holds.

The [configuration-owned snapshot diagnostic](../configured-periodic-snapshot.md) selects the actual pure `_PERIODIC` body before constant caching and preserves its configured name and original definition. This separate Rust API does not enable ordinary CLI scheduling or temporal proof.

The [producer-owned request diagnostic](../producer-progress-requests.md) adds real expansion claim/pause receipts to the configured snapshot. Completed uncommitted output is not a temporal graph write; ordinary periodic checking remains unsupported. Its runnable Cargo example documents the separate request/controller force policies.

[Committed request prefixes](../committed-periodic-prefix.md) publish actual returned contiguous work under the existing owned pause. The Cargo example and typed receipts distinguish expansion, graph publication and physical frontier append; ordinary temporal checking remains unsupported.
