# Changelog

- Explicit final TLC mode now admits pure state conditional predicates through
  the corrected shared planner and fresh independent witness replay. Scoped
  LET/INSTANCE, entry/closing and error controls retain all earlier refusals;
  resource bounds and ordinary periodic scheduling restrictions stay unchanged.


All notable changes to this project are documented here. The project will follow
[Semantic Versioning](https://semver.org/) once the library API is published.

## Unreleased

- Scoped explicit final temporal checking preserves local LET derived role
  names separately from formal argument forwarding, and exact cardinality-one
  DNF shape. Caller contexts and original witness claims have pinned field and
  fresh replay controls; expanded-action names and ordinary schedules remain
  incomplete. See `docs/tlc-final-scopes.md`.

- Explicit final/TLC checking admits proven direct temporal conditionals with
  pinned shared-guard tags, exact particles and independent counterexample replay.
  Compound guards and INSTANCE attribution have source-bound controls; forwarding,
  accepting-sink early witnesses, ordinary schedules and existing bounds remain
  explicit restrictions.


- Bounded TLC temporal normalization now retains shared IF-guard object tags
  and exact last-visit tag holes, with numbered-particle/edge and independent
  periodic-word controls. This source-level repair does not yet lift the staged
  final temporal-IF refusal or certify ordinary periodic scheduling (#170).
