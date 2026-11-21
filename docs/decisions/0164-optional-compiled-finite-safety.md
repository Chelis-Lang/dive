# 0164. Optional compiled finite safety

- **Status:** accepted
- **Date:** 2026-11-21

## Context

On bounded configurations of KeyValueStore, generated native guards and
packed exact membership reduce both evaluation and duplicate detection
costs. The finite symbolic checker already binds the original source, proves
layout admission and checks symmetry of the complete action witnesses.
[Packed search results](../internal/survey/verification-workloads.md#compiling-the-finite-relation)

## Decision

Offer `dive check --strategy compiled` in the Linux companion. Generate Rust
from the existing finite relation, then compile in a private temporary
directory using the installed pinned compiler. The supervisor holds the same
time limit through binding, compilation, search and verification. Each call
checks the current source. The public report uses the finite checker format.

Search with exact packed keys in breadth-first layers. Parallel membership
retains each canonical representative once and counts every generated
occurrence. A finding triggers an exact Core search and independent replay in
the remaining budget. Report success after exhausting the search, a named
unsupported feature upon failed admission, and an error upon disagreement.

## Consequences

The first version requires a layout of at most 32 bits and the finite
binder's supported index symmetries. It has no persistent compiled artifacts.
A toolchain and linker are needed for the compiled strategy. The ordinary
Core entry point continues to handle general models, including liveness.
