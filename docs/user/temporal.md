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
and indexing conventions.

## Final checking in the TLC profile

The explicit command `dive check --compat=tlc --lncheck final --lasso tlc`
uses bounded final-only checking and schema 2, with fresh independent replay of
every reported counterexample. It does not enable TLC's periodic schedule. Only
the separately named `dive-final-tlc` harness mode evaluates this contract.

- Models with no temporal obligations run the ordinary safety engine inside the
  final mode's capture limits. The report names the `bounded-safety-only` route.
  A finite counterexample must pass independent replay, and a resource stop is
  never a safety success.
- Pure state-level and temporal-level `IF` conditions, caller-context forwarding
  of property arguments, and pure user operator formals (with their LET and
  INSTANCE scopes) are supported. So are direct `Len`, `Head`, `Cardinality` and
  fixed-arity `Append` actuals.
- Results name the configured property before any split, and keep derived role
  labels separate from the user's names.
- Stored scalar and composite action and constant captures, and concrete set and
  function successor values, are supported within recursive admission limits.
- Configured native random replacements such as `CONSTANT C <- RandomElement`
  are refused before graph capture. Configuration-origin-ambiguous, recursive,
  opaque and other native operator targets are refused by name.

## Rust diagnostic APIs

`dive-liveness` also exposes diagnostic Rust APIs: ordered temporal execution,
partial SCC checks and periodic-work, backend and terminal diagnostics. They
retain concrete graph and candidate evidence, but their completion is never a
temporal verdict and they do not enable ordinary periodic checking. Under ADR
0086 these TLC-internal projections are not parity obligations, and they may be
simplified or removed.
