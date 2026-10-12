# 0073: Acyclic procedure-body checksum ownership

- **Status:** accepted
- **Date:** 2026-10-11

## Context

The declared 130-row C/P selection retains the previous 98 cases and adds 32
procedure-body call shapes. Clean pinned parser/name-fixed/original-after-generation
stages are separate from direct exploded xast. The first capture failed during
manifest writing with ENOSPC; its raw files remain failed evidence. The completed
retry has 116 SANY-valid artifacts, 14 reference errors and all 32 new rows valid.
The old native producer gave 68 exact header matches, 54 named refusals, six
accounted source rejections and two invalid command selections without credit.

`ParseAlgorithm.GetProcedure` resets and stores `proceduresCalled` by reference;
`GetCall` appends distinct targets in source encounter order. With declarations
Outer, Inner and calls Outer-to-Inner / main-to-Outer, the original header stores
Outer=[Inner], Inner=[Outer]. Reversing declarations gives Inner=[],
Outer=[Inner,Outer]. A three-owner runtime chain similarly appears cyclic if the
last header's main alias is treated as a call edge. This field is checksum
bookkeeping, not runtime reachability. Saved target slots and current return
slots still follow the separate `PcalTranslate.ExplodeCall/ExplodeReturn` view.

## Decision

Represent each procedure's source-owned call vector separately from main's call
vector. Serialize the former for every owner, appending distinct main targets
only to the last declaration. Preserve first source encounter ordering and
reference duplicate suppression. Admit only actual acyclic procedure-body call
graphs, including forward and backward declarations. Use an iterative indegree
walk over actual edges, never over the last-header alias. Calls remain runtime
calls; checksum derivation does not unroll them.

Reserve source scanning, target lookup, source sorting, owner lookup, duplicate
comparisons, adjacency visits, indegrees and the complete work queue before their
owned allocations. The existing shared conservative expansion and output limits
apply across all owners; there is no per-procedure multiplication or quota change.
These units bound work/owned expansion, not exact instructions or Rust allocations.
Real cycles, process ownership, scoped renaming and other unproved checksum
contexts remain named refusals before CRC/replacement output.

## Consequences

No crate edge, default behavior, checksum scheme or witness policy changes.
Original-header bytes/CRC, public whole-module CST/SANY/Validator admission,
finite model results and independent replay are separate evidence stages.
Strict saved-frame replay remains qualified under #124; canonical-only validation
cannot become TLC-profile credit. Fair-header-only equality is the measured weak
reference detection policy, not an integrity certificate. F-034/F-035 stay partial.

## Qualification

Frozen source `64964b539a207e5727d8a537e03887bcaea5f197` closed all 53
gate stages, including 1,197 tests with no skips. Full 130 raw / 260 public
selections retain every error/refusal/invalid-command disposition. The declared
finite models produced 1,488 settings; independent replay has zero strict
saved-frame Valid and 62 separately qualified canonical Valid. Portable readback
preserves actual stages, initial ENOSPC and six failed commands plus explicit
repairs. Selected source identity is distinct from a hermetic environment:
15,944 original inputs, 43 separate literal fixtures and 208 external reference
inputs keep their own authorities. Post-gate verification/documentation does not
retag source/runtime. See the executable guide and acceptance delivery.
