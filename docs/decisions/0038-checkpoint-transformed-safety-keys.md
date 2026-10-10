# 0038: Restore configured VIEW and symmetry safety keys

- **Status:** accepted
- **Date:** 2026-10-10

## Context

ADR 0033's first native safety checkpoint slice refuses VIEW and SYMMETRY.
The clean pinned captures in `conformance/probes/native-checkpoint-keys/primary/`
precede this extension: CpView ends at 7 generated / 4 distinct / depth 4,
CpSymView at 5 / 3 / 3, and CpSymConcrete at 3 / 3 / 3, all with invariant
violations. Ordinary checking agrees; the frozen pre-extension binary refuses
native checkpointing. CpSymConcrete's concrete owner is p2 although its symmetry
key represents p1. The declaration is intentionally not an audited symmetric
specification; the control isolates concrete retention, not symmetry validity.

Live search already calculates its keys through `engine::state_key_of`:
symmetry canonicalization precedes VIEW and the selected store's key encoding.
Raw `S::key` during recovery loses these configured transformations and can
reject correct archives or reconstruct the wrong visited set.

## Decision

Admit pure finite INIT/NEXT safety models with supported VIEW and safety symmetry.
Restoration decodes each original concrete discovery, recomputes its key through
the actual configured live-search path, and checks the saved key and visited-set
uniqueness. Preserve concrete predecessor IDs, ordinals, names and frontier order.
Key-evaluation failures during import are archive admission failures, not proof
that a model has an evaluation error on a reachable state.

The native semantic identity changes to `native-checkpoint-0038-v1`. Existing
schema fields, limits, exact source/options/executable identity, checked counters,
atomic writes and integrity controls remain. Both profiles and stores use their
existing configured semantics. Recovery remains inline, with actual width one
and requested width retained explicitly. The TLC reset remains the default;
retaining original counters is still an explicit override.

Archives are trusted-local inputs, not authenticated reachability certificates.
A coherent malicious rewrite can preserve transformed keys while changing
concrete states; key equivalence alone never validates Init, Next or a closing
trace edge. Independent concrete witness positives and changed equivalent/Init/
closing-state negatives remain separate from archive integrity controls. Restore
does not regenerate Init or semantically replay every stored discovery.

Pre-binding purity checks conservatively root every non-native user body and
every non-native top-level zero-arity startup body, including standard modules, plus
every possible role/config-substitution
target, resolved through all matching module-context aliases as well as definition
names. Structural IR admission does not certify caller-supplied semantic metadata;
a selected local alias must be inspected before constant precomputation. The lowered `Def.std` marker is the native override authority, matching
`Tables::lookup_def` and `Binder::constant_defns_of`; imported native bodies do
not run at startup. Reached native calls are inspected directly, while user
same-name fallbacks retain their real bodies. No namespace match is guessed to
admit an override. Effectful VIEW/symmetry, local fallbacks, constant/definition
redirects to PrintT, runtime constructors and unused startup effects refuse
before output or checkpoint-file creation. Pure native Permutations, a pure
constant replacement and a pure local same-name body have positive controls.

## Consequences

Pure VIEW and symmetry safety recovery gain coverage without another key path
or checker-to-validator dependency. No dependency graph changes. Temporal/graph
recovery, runtime names/effects/random streams/checker reads, deferred state
values, transient spills and parallel recovery remain named exclusions. Symmetry
liveness and spec audit remain unimplemented; valid permutation declarations
alone do not prove that the specification is symmetric. F-032 remains partial.


The resolved-alias admission control fixes [issue #164](https://github.com/Chelis-Lang/dive/issues/164).
The complete pre-repair 737-test source/matrix and failing effect control remain
historical evidence; the current 738-test source passes the same refusal plus a
pure alias positive, with fresh actual recovery and independent witness records.
