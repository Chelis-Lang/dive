# 0032: Bounded single-process PlusCal translation

- **Status:** accepted
- **Date:** 2026-10-10

## Context

The sequential generator does not represent process storage or scheduling.
`PcalTLAGen.GenProcess` distinguishes a constant-ID process from a set of
processes. In both cases the program counter is a function; only the latter
indexes local variables and parameterizes actions by `self`.

## Decision

Admit one unfair process block in current C/P syntax, with a literal nonnegative integer or
string ID, a finite literal nonnegative integer/string set, or an interval with nonnegative integer endpoints. Preserve
the source domain expression, including duplicate entries and an empty domain.
No runtime process-domain evaluation or exhaustive-check claim is made by the
translator. Nonliteral/composite IDs remain named refusals.

Reuse bounded continuation and changed-variable normalization after proving
global/local/generated names do not require renaming. A private storage context
records the original process. Constant-ID locals remain scalar. Membership
locals use function initialization, `v[self]` reads, `v'[self]` reads after an
earlier assignment, and `[v EXCEPT ![self]... = ...]` updates. `pc` initialization,
tests and updates remain indexed for both forms. Preserve the process wrapper,
existential scheduling, empty-domain termination conjunction and program-counter
omission rules from the pinned source.

Process bodies require `-label`/`--label` for every missing label, even when no
source label occurs. This differs from the pinned sequential default. Match
`ParseAlgorithm.getAlgorithm`'s process `LabelFlag` check rather than inheriting
the sequential heuristic. Preserve parser AST spelling; inserted metadata is
separate from original labels. Omitted process-local initializers use the same
`defaultInitValue` constant/configuration contract as globals.

The artifact exposes original process source extents, binding kind and local
storage names. The native API checks the complete generated enclosing module
before returning an accepted artifact. Both profiles use the same admitted
translation. Processes with fairness or label modifiers, multiple blocks,
procedures/macros, name disambiguation and unproved local membership initializer
bindings remain explicit refusals. No new crate dependency edge is introduced.

## Consequences

Retained primary translation, separate SANY validation, development-only
normalization ASTs, token equality and full-module CST equality prove different
stages separately. Finite generated models and independent concrete witnesses
test the resulting state graph at multiple workers and both stores. Unsupported
and invalid primary artifacts remain in the complete selection. F-034/F-035
stay partial and audit-required; full process/fairness/procedure/macro/options
and source-map/checksum parity require later increments.
