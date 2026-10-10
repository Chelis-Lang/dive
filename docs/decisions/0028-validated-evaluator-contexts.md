# 0028: Validated public evaluator contexts

- **Status:** accepted
- **Date:** 2026-10-10

## Context

Rust callers can supply an IR and constant/definition override tables directly,
without the CLI's semantic analysis and binding. On source `8f451ff`, an
out-of-range redirect target panicked during evaluator construction, after
interning a literal. A missing lazy expression ID was also admitted into a
scope. Safe temporal endpoint lookup alone could not repair these earlier
failures. The original source and run are retained in
`crates/dive-api/tests/fixtures/public-context/`.

## Decision

`Evaluator::new` and `Evaluator::with_sink` return
`Result<Evaluator, ContextError>`. Construction validates arena references,
lexical frame/tree structure, binding-table shape, override signatures and
nontrivial redirect cycles before interning literals or executing expressions.
Identity redirects are valid; redirect chains are normalized after admission.
Valid recursive definitions and operator arguments remain supported.

`ContextError` has stable typed categories and codes `DIVE-EVAL-C001` through
`DIVE-EVAL-C007`: unknown ID, arity, redirect cycle, foreign context, malformed
structure, opaque value and admission work limit. Scope operations propagate
these as named evaluator refusals. The binding stage preserves TLC's existing
configuration arity diagnostic through a targeted check before construction;
it does not map unrelated malformed Rust contexts to that TLC code.

Scopes and actions retain their originating IR/name table and a weak identity
of the evaluator binding context. A different IR
is refused even if numeric indices match. Empty and symbolic binding-stage
scopes may transfer between evaluators over the same IR. On a transfer between evaluators, admission inspects stored captured atoms,
cached thunks, substitutions and private native lazy values without forcing
them. Foreign atom tokens, cached thunks from another binding context and lazy
functions tied to another evaluator are refused. This also prevents a cached
constant result from one evaluator from silently overriding another evaluator's
constant binding, even when both share the same IR and name table. All supplied arguments
are checked before a strict argument can execute.

A native deferred set is inspected through Rust's `Any` trait upcast, never a
caller-provided `as_any`, `key`, `member`, `elements` or formatting callback.
Opaque deferred values are refused. Structural value and scope admission each
have a 65,536-item work budget. Bulk function, stored-set, lazy-component,
frame and state-value work lists check room before cloning or enqueueing their
contents; resource refusal does not first allocate an input-sized expansion.
This limits admission work, not total retained
memory. Ordinary value constructors retain their existing behavior: constructing
or hashing a tuple/record may call a deferred key. Admission itself must add
zero calls above the already-created value's captured baseline.

Independent trace, periodic-predicate, enabledness and planning entry points
check model/evaluator ownership and supplied predicate scopes before analysis
or replay setup. `dive-trace` remains standalone without the enumerator.

## Consequences

Unpublished Rust consumers must handle the constructor and scope-operation
results. No infallible compatibility wrapper creates a dummy evaluator or
silently drops malformed overrides. CLI-backed valid models keep their existing
behavior and startup diagnostics.

Bare `ExprId`, `DefId` and other numeric IDs are still relative indices. Range
and signature checks cannot prove where an equal-number index came from; a
caller must use IDs from the admitted IR. Ownership branding establishes scope
and native-capture provenance, not provenance of bare numbers. This increment
also does not certify arbitrary caller-modified semantic annotations, arbitrary
state values, the mathematical correctness of supplied action-split projections,
or general evaluation resource safety. F-035 remains partial/audit-required.
