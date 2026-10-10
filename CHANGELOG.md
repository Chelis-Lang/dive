# Changelog

- Explicit final TLC mode now admits pure state conditional predicates through
  the corrected shared planner and fresh independent witness replay. Scoped
  LET/INSTANCE, entry/closing and error controls retain all earlier refusals;
  resource bounds and ordinary periodic scheduling restrictions stay unchanged.


All notable changes to this project are documented here. The project will follow
[Semantic Versioning](https://semver.org/) once the library API is published.

## Unreleased


- Explicit final/TLC mode routes freshly bound models without implied temporal
  obligations through bounded ordinary safety checking. Reports retain actual
  counts, limits and workers, name the safety route and claim no temporal product
  certificate; finite witnesses require fresh independent memory/JSON replay.
  Constraint/property replay boundaries remain named refusals.
