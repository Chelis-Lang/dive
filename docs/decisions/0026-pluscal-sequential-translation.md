# 0026: Bounded native sequential PlusCal translation

- **Status:** accepted
- **Date:** 2026-10-10

## Context

The source parser (ADR 0022) preserves both current C/P grammars but does not
normalize them into actions. Translator success alone is insufficient evidence:
the pinned translator produces invalid enclosing TLA modules for some variable
and binder names. Significant tokens alone also cannot prove TLA junction layout.

## Decision

Add a native explicitly labelled sequential translation path, with separate
source, normalization, generation and enclosing-artifact admission stages.
Normalization builds bounded action trees, preserving the pinned control-flow
expansion and action order. Generation tracks variables changed within an atomic
action; simultaneous assignments read the pre-group snapshot. Cross-branch
identities retain declaration order. Both profiles use the same proved generation
semantics. Unsupported normalization/options return named errors before an
artifact is admitted; no Java is called at runtime.

`dive-pluscal::translate_at` returns a body and native byte associations.
`dive_api::translate_source` / `translate_file` select a single algorithm in a
module comment, then syntax/name/level-check the complete generated module.
`dive translate` returns that in-memory artifact. Existing translations are
refused; file rewriting, checksums and TLC map formatting remain pending.
Translation acceptance is never model proof or exhaustive exploration.

The pre-existing permitted `dive-api -> dive-pluscal` dependency edge is activated;
no graph edge is added. The validator's independence is unchanged. Generated
models are checked and independently replayed through the existing separate
validator during development controls, not through translator code.

Source, token, node, depth, action and output limits bound work. Normalized depth
is charged even for flat `elsif` chains. Calls join a dedicated stack worker.
Single-line expressions may be substituted only when binder shadowing is absent;
multiline expressions and unproved lexical scopes are explicitly refused.

Evidence retains the complete selection, raw clean translator/SANY outputs,
span/trivia-free operator CST comparisons, generated finite models and corrupted
witness controls. Invalid reference artifacts receive no translation credit.
Details and reproducible commands are in `docs/pluscal-translation.md`.

## Consequences

The next packages can extend this normalization path to procedures, processes,
macros, automatic labels and precise name disambiguation without conflating
source acceptance with generated-artifact acceptance. Formatting/source-map and
historical-option parity need separate matrices. F-034 and F-035 remain partial,
with family certification audit-required. Refusals describe current scope rather
than pretending these constructs are invalid PlusCal.
