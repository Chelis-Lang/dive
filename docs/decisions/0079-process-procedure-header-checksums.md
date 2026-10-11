# 0079: Process header calls have their own source owner

- **Status:** accepted
- **Date:** 2026-10-11

## Context

Issue #225 declares all 170 recursive predecessor inputs plus 72 C/P finite
process candidates/boundaries before native edits. Clean original generation,
parse/name-fixed/post-generation stored AST and separate direct exploded xast
are captured individually. Fifteen C statement separators were initially missing;
the corrected 242-row capture preserves every initial error and source. The new
72 have 66 generated outputs, 64 SANY-valid artifacts and six primary errors.
Both scoped-name capture artifacts generate but fail SANY, without positive credit.

Pinned GetProcess resets proceduresCalled independently from GetProcedure.
The procedure's vector remains source-owned; a process owns its own calls rather
than aliasing LAST as sequential main does. GenTLA separately indexes runtime
state. Stored original header slots and direct exploded slots are unindexed in
these measured controls. A CRC over xast or textually indexed slots would be
wrong even if the generated runtime state happened to be correct.

## Decision

Admit only one finite explicitly labelled unfair process and one disjoint
procedure in the existing Full checksum path. Keep sequential LAST/main handling
unchanged. Allocate separate bounded process called vectors; discover source calls
in encounter order, charge owner/target searches, comparisons and storage before
allocation. Reuse the proved original-header shared-vector transfer overlay and
existing runtime emitter without unrolling callees or indexing header fields.

Existing namespace/domain/arity/control normalization runs before successful
serialization. Declared/none options retain the unfair scope; process/algorithm
fairness, other fairness settings, modifiers, definitions/macros, automatic
labels, mixed/multiple owners and unproved scoped/multiline forms refuse explicitly.
Global quotas remain unchanged; every owner debits one conservative reservation,
not a per-process allowance or an exact machine-instruction/allocation count.

## Consequences

No dependency edge, default, checksum policy or witness policy changes. Full
CRC bytes, public CST/SANY/Validator artifacts, finite model search and independent
replay remain distinct evidence. Strict saved-frame #124 remains uncredited;
canonical-only validation never becomes strict agreement. Reference-invalid
scoped artifacts and original failures remain in the declared census. F-034 and
F-035 stay partial. The same increment includes executable CLI/Cargo user docs,
source-qualified captures and complete gate/runtime receipts once closed.
