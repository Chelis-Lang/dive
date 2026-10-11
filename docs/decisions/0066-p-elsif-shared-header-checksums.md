# 0066: P elsif shared-header ownership

Status: Accepted for the bounded sequential-procedure representation

## Context

The pinned parser represents P `elsif` as nested `AST.If` objects. The native
parser retains a flat branch list. Those representations agree for ordinary
serialization, but do not identify the same shared Else vectors when the
translator expands a final procedure call or return. The declared 32-row C/P
study preserves the old producer's 17 exact matches and 15 named refusals.

The header CRC hashes the original AST after generation has mutated shared
vectors. Direct `Explode` produces a separate ownership study and is not the
header checksum authority. The previous 180 rows and 12 loop studies remain
unchanged; no old producer is retagged.

## Decision

Use a typed `IfBranch { id, index }` view of each virtual reference conditional.
Work backwards through the flat source branches, keeping each Then and Else
vector distinct. The transfer flag propagates exactly as in the pinned
`CopyAndExplodeLastStmt`: an appended UpdatePC belongs after the complete
remaining Else subtree, not independently inside each source branch. The writer
starts at that virtual branch and preserves the pinned nested record/vector
indentation and ordering.

The existing transfer-span index, procedure-entry reservation and generation
visit ordering remain in use. Virtual nesting charges the existing depth bound;
reserve eight conservative expansion-policy units per branch before constructing
its containers, flags, entries and view nodes. Source scans, flattened child
vectors, transfer frames/slots/name strings and final output retain their own
existing reservations. These are conservative policy units, not a measurement of
allocator calls or evaluator instructions. No quota increases or dependency
edges are introduced.

## Consequences

This lifts only proved flat P-elsif shared-header cases in one disjoint explicitly
labelled sequential procedure. Public generation must independently pass exact
whole-module CST, SANY and Validator checks. CRC equality is neither source
identity nor a model proof. Recursive, multiple, process-owned and scoped-name
procedure checksums, unrelated macro/DEFINE combinations and unproved multiline
procedure generation remain named refusals.

Strict saved-frame trace replay remains qualified by #124. Separately labelled
canonical replay cannot certify a TLC-profile witness. The algorithm/parser and
API parity families remain partial.

## Evidence

See `crates/dive-pluscal/p-elsif-checksum-acceptance/` for the declared selection,
clean parsed/name-fixed/after-generation header stages, separate direct-Explode
stages and original native refusal campaign. `p_elsif_checksum.rs` asserts exact
stored-header bytes and CRC for all 32 cases plus bounded resource controls.
Public/model/forgery campaigns have separate source-bound reports and must finish
before delivery credit. User contract: `docs/pluscal-p-elsif-checksums.md`.
