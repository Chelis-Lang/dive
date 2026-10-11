# 0077: Recursive procedure header graphs are finite source data

- **Status:** accepted
- **Date:** 2026-10-11

## Context

Issue #223 retains all 130 acyclic-study inputs and adds 32 C/P self/mutual
recursion controls before production edits. Clean translation has 146 SANY-valid
artifacts, 14 reference errors and two generated/SANY-invalid zero-parameter
self-tail artifacts. The first declaration omitted the inner main braces in 16
C inputs; their primary errors and corrected declaration are retained separately.
Original-after-generation stored header ASTs are separate from direct exploded
xast. The prior native binary matches 104 headers, refuses 50, accounts for six
reference rejections and refuses two invalid commands without parity credit.

The procedure body call graph is finite source data. The LAST declaration's
shared main-call alias is checksum bookkeeping, not a body edge. Neither a cycle
nor an unused recursive owner requires following callee bodies to serialize the
stored header. `PcalTranslate.ExplodeCallReturn` distinguishes self calls from
cross-owner tail calls, preserving the existing frame for the former and
restoring caller locals/replacing its head for the latter.

## Decision

Lift the acyclic-only header boundary for already admitted globally disjoint,
explicitly labelled sequential procedures. Preserve source encounter order,
duplicate suppression, declaration order and the distinct LAST alias. Retain
bounded graph discovery, indegree/work-queue reservation and its acyclic-prefix
walk; cyclic entries remain source-owned data, without recursion/unrolling.

Port shared-vector CallReturn mutation separately from raw stored CallReturn
serialization. A self tail call binds target parameters, initializes target
locals and jumps to entry without replacing the frame. A cross-owner tail call
also restores caller locals and replaces the frame head, then performs the same
binding/initialization/entry sequence. Generation-visited simultaneous
assignments are sorted in the pinned order. Charge synthesized wrappers, both
owners' slots and names before allocating their vectors. Existing global node,
byte, depth, action and output ceilings remain unchanged. These conservative
translation work units are not exact allocation/instruction counts.

## Consequences

No crate edge, checksum policy, runtime termination guarantee, default or trace
policy changes. Raw serialized bytes/CRC, public CST/SANY/Validator admission,
finite exhaustive model counts and independent replay remain distinct stages.
The two primary-invalid artifacts have no public success/model credit. Process
ownership, scoped renaming, definitions/macros in procedure checksum scope,
automatic labels and other unproved combinations still refuse explicitly.
Strict saved-frame replay remains pending under #124; canonical-only Valid is
never TLC-profile validation. F-034/F-035 remain partial and audit-required.

The first cycle-removal-only native attempt retained a real nested self-tail
header mismatch and an insufficiently small unused-owner negative. The repaired
controls require exact pinned stored bytes and use a parsed 16-owner cyclic
source with the original 128-node shared expansion bound. Frozen source/runtime,
commands and failures will remain individually qualified in the delivery.

## Closed producer qualification

The frozen `babfdea78` Rust/Cargo producer completed all 54 local commands and
1,203 nextest cases with zero skipped. The fresh 170 raw/340 public selection,
98 finite sources/2,352 settings and 980 replay controls remain independently
qualified, with zero strict saved-frame Valid. Four original runtime commands
failed because historical driver paths were absent; their exact archived drivers
were restored for four separately recorded fresh reruns, without source/binary,
quota or assertion changes. The portable reader requires both command epochs
and rejects 32 fully rehashed semantic forgeries. Post-gate guard/documentation
packaging changes are not retagged as the tested runtime producer.

The original freeze has 5,264 selected inputs, including 451 standard compiled
Rust inputs and 65 literal include targets. The separate reference/tool
supplement records 786 older fixture/primary inputs, 26 runtime-recorded tools
and two restored drivers after closure; it explicitly grants no retroactive
freeze membership, hermetic-environment claim or family certification.
