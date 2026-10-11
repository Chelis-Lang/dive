# Safety checking

```sh
dive check docs/user/examples/Counter.tla --seed 7 --workers auto --output json
dive check docs/user/examples/Counter.tla --compat=tlc --seed 7 --fp 0 --workers 8 --store exact --output json
```

The explicit BFS checker loads a configuration, binds constants and roles,
evaluates assumptions, then explores supported initial and successor states.
It checks invariants, supported action properties, constraints and deadlocks.
`-deadlock` disables deadlock checking, matching TLC's spelling and meaning.
Use the sibling `.cfg` or select one with `--config FILE.cfg`.

Fingerprint storage is the normal mode. `--store exact` keeps canonical concrete
keys for supported values; fingerprint collision qualifications do not become
an exact-state proof. Inspect `statistics`, `collision`, `counterexample`,
`warnings`, `notes` and resolved `options` in the JSON report.

Supported pure parallel searches commit results in deterministic reference
order. Read `options.workers.requested`, `used` and `reason`: checker-register
reads and some modes can require an inline run. A requested pool width is not
a claim that every step used that many active threads. `--sequential` explicitly
selects inline checking. Simulation and checkpoint widths have separate limits.

Pure safety symmetry retains concrete counterexamples while canonicalizing
visited keys. Unsupported representative/startup/value interactions refuse.
VIEW with random operators currently returns `unsupported` before binding,
output or search, even under `--compat=tlc`. The source-wide guard also detects
unused definitions and native aliases, including native replacements selected
only by `CONSTANT C <- RandomElement` or an instantiated context alias. The unfinished native seeding path could
produce different concrete draws despite equal counts; see the preserved
[example and evidence](../../conformance/probes/view-random-seeding/README.md).
Pure VIEW and direct source-call random checking without VIEW remain available.
VIEW normalizes all concrete variables before computing a visited key, even
variables the projection ignores. If a value cannot be normalized on the
proved bounded path, the result is `unsupported`; an unprintable state never
becomes a validated counterexample. See [the measured boundary](../../conformance/probes/view-concrete-normalization/README.md).
Random checking reports its actual inline width separately from requested workers.
Native random replacements selected by `.cfg`, for example
`CONSTANT C <- RandomElement`, now refuse before startup/output/search even
without VIEW. This includes aliased targets, all four native random families,
and mixed source/configured calls. Matching counts previously concealed both
false violations and missed violations. Pure user replacements remain supported.
The callback-free target scan is conservative across matching modules/aliases;
shadowed or malformed targets can receive this refusal before a binding error.
There is currently no compatibility flag that enables this unproved route.
See [the exact reproducer and repair boundary](../../conformance/probes/configured-random-admission/README.md).

Ordinary strict temporal model checking remains unsupported; use the explicit
[final/short mode](temporal.md) for its supported subset. Refused library
operators, unknown finite deadlock covers, resource stops and internal failures
must never be treated as `success`.

See [compatibility](compatibility.md), [storage](storage-recovery.md),
[reports](reports-and-errors.md) and the [complete coverage ledger](../tlc-parity.md).
