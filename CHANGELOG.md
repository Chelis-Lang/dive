# Changelog

- Explicit final TLC mode now admits pure state conditional predicates through
  the corrected shared planner and fresh independent witness replay. Scoped
  LET/INSTANCE, entry/closing and error controls retain all earlier refusals;
  resource bounds and ordinary periodic scheduling restrictions stay unchanged.


All notable changes to this project are documented here. The project will follow
[Semantic Versioning](https://semver.org/) once the library API is published.

## Unreleased
- Native commit-boundary lifecycle diagnostics use real borrowed dispatch authority, internal clock/controller polling and actual EOF final SCCs. No ordinary temporal verdict is enabled; see `docs/native-periodic-lifecycle.md`.

- Source-owned periodic backend diagnostics retain pinned pointer-bucket growth/check-size snapshots, private joined-wave facts and first-candidate stopping. Original work bounds and independent replay remain required; ordinary periodic checking and partial Holds remain unsupported. See `docs/periodic-backend.md`.

- Bounded partial temporal SCC/source-checkpoint diagnostics retain written-record authority and candidate fields under shared graph limits; successful partial checks never certify temporal completion.
- Fixed-arity `Append` operator arguments in explicit bounded final TLC checking,
  with independent operand and witness proof. Tail/SubSeq runtime-name refusals
  and existing budgets remain unchanged.
- Explicit final TLC state admission accepts stored Enum/Raw sets, including
  normalized sets nested in tuples, records and general functions. Callback-free
  namespace/order checks and unchanged cumulative work/depth bounds precede
  fingerprinting; cached deferred providers still refuse.
- Bounded temporal operator actuals: `Len`, `Head`, and `Cardinality` retain
  scoped constant/state calls with independent word/witness checks and
  callback-free stored-input admission. Configured/opaque/effectful and
  unsupported state-storage/error boundaries remain explicit; final TLC
  scheduling remains separately scoped.


- Explicit bounded final TLC checking supports proved pure user operator formals,
  retained LET/INSTANCE scopes and constant/state/temporal actuals. Independent
  periodic and serialized witness replay retain original attribution. Native,
  recursive/higher-order, configured-origin-ambiguous and effectful callbacks
  still refuse without raising resource limits. See `docs/temporal-operators.md`.

- Explicit bounded final TLC checking admits callback-free stored strings/model
  atoms, tuples, records, functions and Enum/Raw sets in captured scopes and
  configured constants. Actual context resolution discharges potential OPEN
  dependence; opaque/effectful/computed providers and fixed recursive admission
  limits still refuse. Supplied stored sets/records/general functions must match
  the evaluator ordering profile; tuple domains remain profile independent.
  See `docs/material-capture.md`.

- Explicit bounded final TLC results retain configured PROPERTY names, original
  expanded temporal Action names, and derived role labels separately in the
  Rust API and schema-2 JSON. Conjunctions preserve repeated original names;
  original witness claims, ordinary schema 1, scheduling and bounds are unchanged.
  See `docs/tlc-property-origins.md`.

- Scoped explicit final temporal checking preserves local LET derived role
  names separately from formal argument forwarding, and exact cardinality-one
  DNF shape. Caller contexts and original witness claims have pinned field and
  fresh replay controls. The later property-origin increment adds expanded Action
  names separately; ordinary schedules remain incomplete. See `docs/tlc-final-scopes.md`.

- Explicit final/TLC checking admits proven direct temporal conditionals with
  pinned shared-guard tags, exact particles and independent counterexample replay.
  Compound guards and INSTANCE attribution have source-bound controls; forwarding,
  accepting-sink early witnesses, ordinary schedules and existing bounds remain
  explicit restrictions.


- Bounded TLC temporal normalization now retains shared IF-guard object tags
  and exact last-visit tag holes, with numbered-particle/edge and independent
  periodic-word controls. This source-level repair does not yet lift the staged
  final temporal-IF refusal or certify ordinary periodic scheduling (#170).
