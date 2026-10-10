# Changelog

All notable changes to this project are documented here. The project will follow
[Semantic Versioning](https://semver.org/) once the library API is published.

## Unreleased


- Native recursive sequential PlusCal procedures: self/mutual call stacks,
  frame-preserving self-tail calls and source-bounded normalization. Both syntaxes
  retain pinned artifacts/errors without unrolling. Invalid upstream output is
  rejected; finite/count, strict pending and canonical witness evidence remain
  separate. Process-owned/scoped renaming and strict namespace provenance are
  still pending. See `docs/pluscal-recursive-procedures.md`.

