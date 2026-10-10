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

## Implementation addendum: deterministic parallel recovery (2026-10-10)

The inline-only restriction above describes the historical first configured-key
slice. The continuation admits its same pure finite safety models through the
existing deterministic three-phase parallel engine. No temporal/effect/spill
admission changes and no dependency graph changes are made.

Resume the archive's actual next level. Each winner retains its already-resolved
configured store key and original concrete state. In ordered discovery position,
record that key/state/parent/ordinal in the native session before invariant
checks. At a completed-level barrier the frontier, predecessor log, seen set
and native discovery sequence describe the same committed prefix; only then
can a pause write its archive. No key is recomputed during parallel commit.
Speculatively retained records beyond a later deciding event cannot become an
archive: a stopped/incomplete level has no checkpoint barrier. Pure admission
prevents that speculative record encoding from introducing model effects.

Native record failure has its own event phase between key insertion and depth
publication, preserving inline distinct/depth behavior. MAX_COUNTER decisions
use the logical ordered generated prefix. Local speculative enumerations cap
work at the remaining native count, but worker scheduling never selects the
reported cutoff. Inline and pool stopping-prefix controls include duplicate
keys, multiple actions, multiple frontier states and chaos scheduling.

Worker/chaos/explicit-inline choices are excluded from semantic identity as
before, so one executable can recover at a different pool width. Boundary-zero
pauses or restored-state admission failures remain actual-inline-one with a
reported reason; continued admitted pool execution reports its actual width.
Wire schema and configured-key semantics remain0038-v1; exact executable SHA
already refuses imports made by a different implementation. The checksum is
still corruption detection, not authenticity or independent reachability proof.

The original inline-only raw/source receipts stay frozen. Separate current
controls cover both profiles/stores, 1/4/8 widths, chaos, cross-width restoration,
actual later-level recovery, counters and concrete independent witnesses. The
archive 64 MiB/state 65,536 bounds remain; parallel candidates/keys/scratch and
ordinary retention are not a total-memory budget. Full F-032 remains partial.

## Implementation addendum: transient spill recovery (2026-10-10)

The previous addendum's spill exclusion is removed for the same pure finite
safety slice. No semantic key/counter/default or dependency graph changes.
Restoration decodes discoveries in original order, inserts configured keys,
rebuilds the configured fallible predecessor log and streams only the saved
frontier suffix directly into the configured queue. No second full decoded
frontier or resident disk-offset index is introduced. A completed-level snapshot
still retains native discovery payloads/parents independently of both budgets.

Pause/recovery may use either budget, both or neither, and recovery may change
budgets/directories/width. These are storage/execution choices excluded from
semantic identity; executable/source/profile/store/config/key identity and all
archive admission bounds remain unchanged. Queue/log failure prevents a model
certificate, retains the original deciding event and preserves actual archive
bytes if commit preceded a later cleanup failure. The additive receipt field
`transient_storage_cleanup_completed` is computed from typed finalized storage
evidence; no diagnostic substring changes behavior.

The original complete parallel recovery source/raw receipts remain frozen.
Current controls retain the original memory selection and run frontier-only,
predecessor-only and both-zero storage across both profiles/stores, 1/4/8 widths,
chaos and both completed-level boundaries. Counter reset/Retain, concrete
independent witnesses, tiny/change-budget controls, corruption and blocked
storage directories are separate obligations. Runtime/effect/temporal/deferred
recovery and full F-032 certification remain pending. Native archive workspace,
visited sets/candidates/worker scratch and reconstructed paths remain outside
both transient budgets; no aggregate-memory or Java byte-interop claim.
