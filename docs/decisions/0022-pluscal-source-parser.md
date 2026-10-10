# 0022: Native PlusCal source AST before translation

- **Status:** accepted
- **Date:** 2026-10-10

## Context

Pinned `ParseAlgorithm.getAlgorithm` combines grammar with macro expansion,
label insertion and control-flow normalization. C block labels move to their
first statement, elsif/binder lists become nested nodes, and call tails fold.
`GetMacro` expands earlier macro calls. Translation needs original source first.

The original expression scanner counts delimiter depth without checking kinds:
primary and secondary probes accept `(1]`, `[1)`, `<<1}` and `[1, end]`. A grammar
AST must not imply valid TLA expressions. A stress test also exposed debug-parser
stack overflow on a normal 2 MiB caller stack before a 256-level resource bound.

## Decision

`parse`/`parse_at` produce source ASTs for current C/P syntax, with ordered
statement arena references. Explicit grouping, macro calls, elsif chains, binder
lists, initializers and label modifiers remain unnormalized. Original UTF-8 bytes
and tokens are retained; counted expression slices await syntax/semantic checks.

Each public call starts and joins a scoped 16 MiB parser worker. The recursive
core runs there, with hard byte/depth bounds 4 MiB/256, defaults 1 MiB/128 and
token/node limits. Resources return `DIVE-P0040`; unexpected worker failure is
an implementation diagnostic `DIVE-P0050`. Statement destruction is flat. No
unsafe code or runtime JVM dependency is introduced.

Ranges are half-open original bytes. Optional display positions count Unicode
scalars, eight-column tabs and CRLF. `parse_at` requires an explicit region and
rejects trailing content; invalid caller ranges never become trusted slices.

Macro substitution, synthetic labels, name/control-flow checking, historical
version grammars, translator options/checksums and translated output are pending.
A source AST never implies translation or a proved model. The existing CLI
translation refusal remains. The DAG is unchanged: the already permitted
`dive-pluscal -> dive-syntax` edge, workspace serde and development serde_json.

## Consequences

Clean primary normalized results and explicitly secondary public grammar-method
results are retained separately. Comparison unfolds documented representation
differences only; unknown nodes fail. Macro expansion differences remain pending,
never AST agreements. Normalization rejects, oracle timeouts and selection errors
remain visible. Offline tests and corruption controls verify source ownership,
operands, declarations, fairness and refusal of false translation claims.

[Evidence](../pluscal-parser.md) accounts for 222 inputs/markers. F-034 remains
partial and uncertified until translations, options, checksums, source maps and
translated-model behavior pass their acceptance matrix. This AST is the seam
for native normalization and translation next.
