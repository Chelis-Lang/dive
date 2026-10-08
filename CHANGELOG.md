# Changelog

All notable changes to this project are documented here. The project will follow
[Semantic Versioning](https://semver.org/) once the library API is published.

## Unreleased

### Added
- Cargo workspace with one crate per charter component, workspace lints,
  pinned toolchain, crate layering enforcement and lean CI.
- AI authorship protection: commit-msg hook, CI check and agent configuration.
- `dive-syntax`:
  - a TLA+ lexer and parser with a lossless rowan CST, including the proof language;
  - parse acceptance matching the pinned SANY parser;
  - stable `DIVE-S` diagnostic codes and error recovery;
  - typed AST views (ADR 0009).
- `dive parse FILE [--output human|json] [--cst]`, with a versioned JSON report
  (`schema_version` 1), through `dive_api::parse_file`.
