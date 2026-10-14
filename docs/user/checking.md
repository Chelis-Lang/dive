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

A pure `SYMMETRY` declaration is prepared before assumptions and initial states.
Division by zero in that declaration and incompatible values encountered while
normalizing its set report `model-error`, TLC `GENERAL` (1000), with no search
statistics or counterexample. The TLC profile exits 255; the default profile
uses its existing model-error exit 1. The default comparison policy may produce
a different explanatory message for a mixed set, while retaining this class.
For example, this fixture fails before exploring any state:

```sh
dive check crates/dive-model/tests/fixtures/symmetry-group/specs/SyDivZero/SyDivZero.tla --compat=tlc --seed 0 --fp 0 --workers 1 --output json
```

Other unimplemented startup exception shapes remain named refusals. Effectful,
state-dependent and parameterized declarations still refuse; these diagnostics
do not expand supported symmetry modes or resource bounds.

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
Ordinary `RandomElement` checking supports a closed pure VIEW and pure startup/Init.
Expansion draws use the VIEW value's TLC fingerprint, while counterexamples retain
concrete variable values. Direct calls and proved configured native replacements
use the same authority. The runnable paired example
shows the reported seed, polynomial, requested width and actual inline width.
Unproved VIEW/startup contexts still return `unsupported` before binding, output
or search. The conservative source walk includes unused definitions and native
aliases, so importing unused Randomization bodies can also refuse this slice.
Concrete values and the VIEW value must fit 16,384 stored-value visits and depth
64: atoms, literal tuples/records, general functions with homogeneous atomic
domains, and explicit homogeneous atomic sets. Compound/mixed sets, symbolic or
deferred sets and nonliteral tuple/record representations remain named refusals;
these extra shape limits do not widen the unresolved mixed-word certificate
boundary in #190. No provider callback or comparison is used to grant admission.
Pure VIEW and direct source-call random checking without VIEW remain available.
VIEW normalizes all concrete variables before computing a visited key, even
variables the projection ignores. If a value cannot be normalized on the
proved bounded path, the result is `unsupported`; an unprintable state never
becomes a validated counterexample. See the measured boundary.
Random checking reports its actual inline width separately from requested workers.
Ordinary safety checking admits the configured `RandomElement` route, for example
`CONSTANT C <- RandomElement`, using the actual bound native operator for startup
and predecessor-stream selection. Native definition redirects and admitted typed
context aliases follow the same authority. These runs execute inline and disclose
requested and used worker counts separately. Pure user replacements still use the
parallel engine where admitted.

The committed configured-operator example is runnable directly:

```sh
dive check docs/user/examples/Stream.tla --compat=tlc --seed 7 --fp 0 --workers 8 --output json
```

It reports an invariant violation with generated/distinct/depth `4/4/4` and
concrete values `0,80,94,100`, matching clean pinned TLC. Although eight workers
were requested, the report discloses the actual inline width of one.

A random violation on the first successor of an initial state
currently returns `unsupported` with its observed stopping-prefix statistics.
TLC's initial-predecessor printing path can keep an already-consumed stream and
omit the final state; dive has not proved that replay contract. The report contains
no counterexample certificate. The conservative source-wide random detector can
also refuse this boundary when an unused definition contains a random call.
This boundary applies in both profiles;
a new full-witness override has not been adopted. Startup draws before random Init can likewise
prevent reconstruction; a seed alone never creates an independent certificate.
The standalone validator also refuses unproved draw positions for multiple draws
or conditionally discarded random branches. A matching concrete value does not
establish where the generator consumed its stream. A checker violation whose
witness is missing or independently `unsupported` never counts as full agreement.
Configured `Randomization` replacements, SYMMETRY with randomness, graph
capture, temporal random checking and seeded native recovery remain explicitly
refused. The callback-free configured-target scan remains conservative across
matching modules and aliases.
See the original wrong-answer repair
and bound-stream controls and remaining replay limits.
The corrected standalone campaign independently validates 144 of 180 original
witness controls, refuses 36 unproved draw positions, and rejects all 180
changed-Init controls. The remaining refusals cover two draws, a discarded random
branch and conditional random Init. These counts belong to the corrected
producer; the original failed campaign's 48 refusals remain historical evidence.

Ordinary strict temporal model checking remains unsupported; use the explicit
[final/short mode](temporal.md) for its supported subset. Refused library
operators, unknown finite deadlock covers, resource stops and internal failures
must never be treated as `success`.

See [compatibility](compatibility.md), [storage](storage-recovery.md),
[reports](reports-and-errors.md) and the [complete coverage ledger](../scoreboard.md).

A lazy filter ignored by VIEW does not require evaluating its predicate. The
checker normalizes its stored bound set only. `x = {i \in Nat : Print(i,
TRUE)}`, `Next == UNCHANGED x`, and `VIEW 0` finish with no Print output.
Spilling or archiving that callback-bearing value still refuses explicitly;
this support is limited to memory checking. Other unproved lazy representations
remain unsupported.

The current D-009 comparison improvements do not make every mixed structure
normalizable. A reversed heterogeneous set may refuse even in the default
profile. A forward heterogeneous set can produce a checker report whose
canonical structured trace then fails decoder shape admission in both profiles.
Treat that report as an unvalidated witness; checking counts or a displayed
trace do not confer independent validation. See the measured selection and
refusals and
[follow-up #190](https://github.com/Chelis-Lang/dive/issues/190).


For automation, a default invariant violation exits 10; with `--compat=tlc`
it uses TLC's physical status (normally 12 for a safety invariant). Conformance
compares these numeric statuses exactly. Known default differences count as
ExpectedDivergence only for declared exact pairs and valid independent witnesses,
never as Agree. See [reports and errors](reports-and-errors.md).

The conformance harness requires whole counterexamples for safety, action-property,
liveness and deadlock verdicts at every width. Missing or metadata-only words
receive no agreement credit; present words must validate independently. This
also holds when a worker-specific comparison omits witness shape.
