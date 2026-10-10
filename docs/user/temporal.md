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
and indexing conventions. Pending pinned final-only APIs must be documented and
verified separately before this guide marks them available.

For Rust tool builders, the separate [ordered execution API](../temporal-execution-api.md)
retains concrete graph and deciding-prefix evidence. Its completion is explicitly
not a temporal verdict and it is not exposed as a CLI checking mode.
