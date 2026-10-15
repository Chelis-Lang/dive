# Reports, errors and automation

Request `--output json` and retain the whole document, source/configuration,
resolved options and replay seed. Every operation has its own versioned schema.
Use its result fields; do not interpret all commands through safety-check keys.

| Operation | What to inspect |
| --- | --- |
| Parse/analyze | `accepted`, `diagnostics`; no property check |
| Translate | Schema 2: `result`, `accepted`, `stage`, artifact/replacement, complete-module analysis and effective options |
| Safety check | `verdict`, counts, options, warnings, collision and counterexample |
| Final/short check | Schema 2, complete `properties`, execution and validated closing edge |
| Simulation | `result`, `exhaustive: false`, `model_proved: false`, observed/pending validation |
| DFID | `termination`, `model_proved`, iterations and validated counterexample |
| Trace validation | `validation`; valid/invalid/refused is separate from search verdicts |
| Checkpoint pause | `checkpoint.status: paused-incomplete`; incomplete execution |

`unsupported` names a feature that was not checked. A resource limit, timeout,
incomplete bound, failed storage or internal error supplies no successful model
certificate. Missing counters/worker evidence remain absent instead of invented.
A valid finite implementation trace says that trace satisfies the checked
relation; it does not establish every possible behavior of the implementation.

CLI parse/analysis/translation use their own exit conventions. `dive check`
uses TLC's deciding physical status in both semantic profiles: the included
invariant example exits 12 and a cyclic final/short liveness violation exits 13.
Both conventions keep 0 for success, 2 for usage/I/O, 3 for unsupported/resource
limits and 4 for internal failure. `--exit-codes native` selects the historical
statuses: 1 for rejected input, 10 for property violation, 11 for deadlock,
12 for assumption failure and 13 for evaluation error. `--compat=tlc` also
selects TLC evaluation; it is independent of the chosen exit convention.
DFID and simulation require their report qualifications even on exit zero.

Stable diagnostic codes and spans belong to the report; display text is intended
for people. Full TLC deciding-error-class coverage is unfinished and tracked in
the status. Generated harness reports in
[conformance](../conformance.md) are development evidence, not the schema for
user commands. Keep profile and schema version in any automation protocol.

Conformance reports compare the physical numeric exit exactly, even when a
worker-specific comparison omits counts. Native 10 versus TLC 12 is a real
observable difference. The harness can label only an explicitly registered
exact TLC/native pair as `expected-divergence`, conditioned on its declared
verdict, permitted other fields and independent trace validation. It never
counts that result as `agree`; absent, signaled or unrelated exits fail.
See [D-023](../tlc-differences.md#d-023-default-process-exits-are-distinct-conformance-observables)
for the historical accounting correction and current CLI decision. Historical
Rust `CheckReport::exit_code()` remains profile-based; select a convention with
`exit_code_for`.

A safety, action-property, liveness or deadlock verdict needs a whole independently
validated counterexample to earn conformance agreement, including in
worker-specific comparisons that omit witness shape. Missing or metadata-only
words cannot be replaced by a claimed validation result. A selected oracle error
witness also remains required when verdicts match and its payload is unavailable; the metadata
provides no certification and cannot waive an observed word. Legitimate success, assumption, frontend and
evaluation outcomes whose contract carries no witness need none. Numeric exit
pairs cannot excuse absent, invalid, unconfirmed or refused required words.
