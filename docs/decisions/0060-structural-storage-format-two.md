# 0060: Structural storage format 2 preserves tuple functions

- **Status:** accepted
- **Date:** 2026-10-11

## Context

A tuple literal and an integer-domain function can be mathematically equal but
retain different TLC application diagnostics. The old structural codec wrote
both as tag `0x08`, then decoded both as tuple literals. The smallest example is
`Init == n=0 /\ s=[i \in 1..1 |-> 1]` followed by `n'=s["a"]`.
The pinned oracle and memory/predecessor-only native paths report an integer-domain
function application error. Frontier spill and native recovery instead reported
a tuple-index error. Both exited with an evaluation error, so equal exit codes
alone failed to reveal the representation loss. The complete original selection
and clean reference output remain in
`conformance/probes/storage-tuple-function/before/`.

## Decision

The structural value encoding uses `0x08` for literal tuples and `0x0A` for
nonliteral tuple functions, with identical length/item framing. It retains both
in nested stored values and contiguous-domain functions. The canonical encoding
continues to quotient function-equivalent representations using `0x08`; exact-store
keys and FP64 fingerprints do not change. No comparison or construction policy changes.

Persisted value-bearing containers are versioned together: frontier data/index
headers become `DIVEFQ02`/`DIVEFI02`; native snapshots use `DIVECP02`, identity
schema 2 and the `native-checkpoint-0060-structural-v2` semantic identifier.
Old version-1 containers refuse explicitly, including coherently checksummed
schema-1 payloads. We cannot recover a lost literal bit by inference. Bare codec
`0x08` still means a literal tuple, so callers persisting raw values must pin their
own container format. Predecessor ID/ordinal-only files retain `DIVEPR01`.

## Consequences

Existing archives must be recreated with the new executable. This intentionally
breaks old native storage compatibility without claiming Java byte interoperability.
Archive identity and checksums retain their current trusted-local/corruption
contract; they do not prove reachability, completeness or authenticity. Frontier,
predecessor and reconstruction budgets are unchanged and do not bound total RAM.

The representation controls compare structural round trips, literal bits,
canonical bytes and FP64 equality. Paired memory/spill/recovery controls compare
actual diagnostics, EC1000/2103 metadata and counts, while preserving the native
error-site `verified: false`. Independent v2 witness checks are separate from
archive admission and from the error diagnostic.

## Implementation evidence (2026-10-11)

The corrected frozen producer and complete source/runtime readback live in
`conformance/probes/storage-tuple-function/current/`. The full gate passes all
1167 tests, zero skipped. The complete192-case memory/spill/pause/recovery
selection retains both profiles, both stores,1/4/8 workers and chaos controls;
192 original words independently validate and576 Init/middle/final mutants
reject. Twelve actual old CP01 archives refuse before continuation. The Cargo
guide and21+16 conformance models pass at1/4/8. Canonical-key/FP and predecessor
source authority remains distinct from structural format changes.

The first dc0f gate’s seven failures are preserved separately: existing
current-payload fixture builders still emitted CP01 and therefore reached only
version refusal. Their headers were migrated without weakening assertions or
selection. A deliberate header mutation now makes the generic corruption check
fail, proving that it cannot claim payload-validation credit from version-only
rejection. Original version-one migration negatives stay intact. No historical
execution is retagged as the corrected producer.
