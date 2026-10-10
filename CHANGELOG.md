# Changelog

All notable changes to this project are documented here. The project will follow
[Semantic Versioning](https://semver.org/) once the library API is published.

## Unreleased

- TLC-order simulation refuses runtime-name history that cannot yet be
  independently validated; canonical default sampling remains supported.

- Preserve TLC runtime string encounter order by using reported inline checking
  for reachable string/name constructors. Eagerly intern checker configuration
  values before assumptions, including the seed and fingerprint (issue #122).
- Independent trace validation explicitly refuses TLC runtime-name history
  that a witness prefix cannot establish, preventing a forged trace from
  choosing its own string order (issue #124). Canonical validation remains
  decisive; supplied seed/fingerprint metadata does not bypass this refusal.
  Caller-supplied temporal/enabledness predicates receive the same scoped
  checks; structurally proven tuple-only sequence calls remain supported.

- Offline parity-ledger checks reject absent or escaping evidence, changed
  stable IDs, unsupported certification claims and Markdown census drift.

- Pure final/short temporal graph capture uses ordered parallel source waves,
  retaining all duplicate/seen edges, original labels/ordinals, roots and
  implicit stutters. Inline/1/4/8/chaos controls cover both profiles and stores.
  Existing logical graph limits are unchanged; a separately named per-source
  scratch admission bound returns an explicit resource refusal. VIEW, symmetry,
  randomness, stateful graph effects and strict TLC temporal timing still refuse.

- Native bounded `dive dfid` and `dfid_file` implement pure safety iterative
  visitation with TLC's discovery/status bits, eager vector counting and actual
  worker selection seeds. Depth-bound stops never claim model safety, and every
  claimed violation is independently certified. Remaining mode/output contracts
  refuse by name; TLC's stale cross-root witness is retained as reference evidence.

- Final/short temporal failures retain their execution stage, actual capture
  workers, completed statistics, output and collision qualifications. Unknown
  thread execution is reported as unknown rather than zero. Existing library
  error interfaces remain available alongside evidence-preserving entry points.

- Auditable TLC parity ledger and delivery plan covering native checking modes,
  distribution/recovery and PlusCal, with pinned source-surface inventories
  and separate accounting for strict and mixed-profile evidence.
- Conformance harness resolves relative executable paths before model/parse
  subprocesses change directory, preserving repository-relative provenance.

- Final/short temporal quantifiers and finite predicate constructors accept
  bounded structural function, record, product and subset domains. Preflight
  bounds intermediate materialization before enumeration and preserves TLC's
  operand normalization order; deferred and infinite domains remain explicit
  refusals. Eight pinned models match final truth/counts in both profiles and
  stores, with independently validated violations.

- `dive simulate` and Rust `simulate_file` sample finite pure safety traces, with
  TLC-exact seeded single-worker selection and default trace-identity replay.
  Every violation is independently validated; sampling never claims a proof.
  Unimplemented simulation modes and effects remain named refusals.
