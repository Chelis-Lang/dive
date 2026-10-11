# 0053: Bounded PlusCal global DEFINE helpers

- **Status:** accepted
- **Date:** 2026-10-11

## Context

The selected pinned Examples contain global DEFINE helpers: Echo and FindHighest.
The native translator previously refused every definition block. At pinned
`94d0c5070852b37b801bb8998c436383ab7ee37b`, `GenVarsAndDefs` emits global variables
and `pc`, then the DEFINE expression, then process-local variables, then `vars`.
Opaque helper names remain unchanged after an earlier assignment. The original
reference stage captures, including invalid translated artifacts, are preserved
in `crates/dive-pluscal/define-acceptance`.

## Decision

Emit source-owned ordinary global operator declarations with significant-token
UTF-16/tab-expanded columns, preserving junction grouping and original spans.
Use existing expression serialization for the post-translation AST's `defs`.
Neither profile inlines helpers or changes their state semantics. A bounded
native syntax/name/dependency proof admits no-renaming, nonrecursive ordinary
operators; generated-name conflicts, non-operator declarations, LOCAL/symbolic,
action/temporal helpers, procedure scopes and unproved cycles refuse by name.
The cycle proof is conservative: formal and field spellings can cause refusals.

Both output allocation and graph work use existing fixed ceilings. No dependency
graph or external runtime changes are required. Public CLI/API admission still
checks the generated enclosing module. Exact raw serialization is not an artifact
or model proof. Original checksum-less corpus headers and missing dependency
observations remain distinct from complete fresh header/Validator evidence.

## Consequences

Common pure helpers no longer require a general scoped renamer. Process-local
references are not silently made valid by flattening all variable declarations.
Recursive, function-definition, instance and scope-sensitive declarations remain
explicit boundaries. Full translator formatting, file writes and map files remain
unfinished. The D-020 fair-header weakness remains exactly the reference behavior;
Full CRC equality is not a source-integrity certificate. F-034/F-035 remain partial.
