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
