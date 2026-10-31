# 0131: Owned set construction history across state handoff

- **Status:** accepted
- **Date:** 2026-10-31

## Context

C1–C5 and D3 in `../developer/core/02-semantic-contract.md` separate extensional
identity from actual construction and demand. Sailfish assigns a union of a
predecessor set and a mapped set of model-value tuples, then queries that
successor before commitment. A canonical set handle proves neither its raw
order nor that TLC normalized that particular object. Independent equal sets
must not acquire each other's mutable phase. The initial #623 local literal
lane retained these distinctions only within an expansion.

The `construction-set-history` microcorpus was pinned through the official
harness with recorded reasons. In particular, a typed-model early hit succeeds,
its reversed incompatible prefix errors, and an excluded successor retains raw
membership behavior. These facts do not license native error classification
without E7 source provenance.

## Decision

Extend the existing sparse arena history for actual literal and MAP constructors
whose elements are closed booleans, inline integers, owned strings/model values,
or literal tuples recursively containing those leaves. Unknown function receiver
classes, set-valued elements and unknown imported histories keep existing guards.
No source/configuration/model-name whitelist supplies authority.

A fresh constructor owns its raw array, duplicate positions and mutable phase.
Successful normalization runs TLC binary insertion and retains the first equal
representative before projecting to the run's canonical order. Raw membership
uses receiver equality; normalized membership uses query comparison and the
actual pivot sequence. D-009 remains the default comparison policy; construction
normalization and union's own membership demands retain TLC's rules.

An origin is allocated only when a construction view escapes its arena. The
Store's private sparse origin table registers exact retained snapshot handles;
flags, `Kind`, a slot number, persistence and extensional equality alone are not
certificates. Normalized immutable snapshots can cross a state handoff. Raw
persistent snapshots remain refused by consumers without a live mutable owner.
The existing exact interner preserves those raw representations without claiming
they are normalized. This is not general import or raw-provider authority.

Canonical state keys and replay expectations remain separate. Only affected
frontier rows retain actual view roots, including nested function/EXCEPT parents.
Unaffected rows use the existing canonical frontier without an extra retained
row. Parallel expansion overlays only the affected chunk; first discovery still
wins. Replay carries its own actual source views while reporting canonical
states. A rollback retains the logical normalization array and can rebuild a
freed arena projection; reset discards expansion-local history.

The ordinary file routes pass their existing armed Budget through an additive
controlled explorer entrypoint, preserving existing entrypoints and explicit
caller limits. New bulk construction/normalization/search work uses that owner;
owned stops cannot become successful outcomes. New vectors reserve fallibly,
and origin-vector capacity is included in Store byte reporting. Sampled RSS
remains necessary; this supplies no allocation-time hard memory guarantee.

## Consequences

The scalar/model-value/tuple literal–MAP–union path is consumed through actual
successor queries and normalized state handoff. This is partial #623 coverage,
not the generic S5 index or a membership-result cache. Broader set/record/function
history, raw persistent mutable ownership and E7 diagnostic source replay remain
separate. Full unchanged Sailfish and the performance panel still require
source-bound measurement; minimals and admission-stop diagnostics establish no
whole-model performance benefit or parity percentage.
