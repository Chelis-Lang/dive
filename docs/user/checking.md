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
Pure VIEW and random checking without VIEW remain available. Random checking
reports its actual inline width separately from requested workers. Configured-only
native RNG replacements (`CONSTANT C <- RandomElement`) still have a separate
stream-classifier defect even without VIEW; avoid them until that repair is
proved. The preserved clean/native example is linked from the evidence guide.

Ordinary strict temporal model checking remains unsupported; use the explicit
[final/short mode](temporal.md) for its supported subset. Refused library
operators, unknown finite deadlock covers, resource stops and internal failures
must never be treated as `success`.

See [compatibility](compatibility.md), [storage](storage-recovery.md),
[reports](reports-and-errors.md) and the [complete coverage ledger](../tlc-parity.md).
