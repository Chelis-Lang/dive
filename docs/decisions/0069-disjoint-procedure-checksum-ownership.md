# 0069: Disjoint sequential-procedure checksum ownership

- **Status:** accepted
- **Date:** 2026-10-11

## Context

Pinned ParseAlgorithm.GetProcedure stores its reset `proceduresCalled` vector by reference. With two declarations and main calls Put then Get, the stored original post-generation header has Put's empty vector and Get's `[Put, Get]`. Reverse main calls gives `[Get, Put]`; reversing declarations moves the shared vector to Put. This is distinct from PcalTranslate.ExplodeCall's target-specific saved frame and ExplodeReturn's source-specific restoration slots. The initial 64-row C/P capture and two corrected local declarations retain both raw parse errors. A separate 32-row owner, call-graph, label and command-option study is declared before admission.

## Decision

Index synthetic frame owners explicitly and retain the current procedure index while constructing return mutations. Cache every procedure entry once. All procedures and main use the same conservative work, byte and depth budget; there is no per-owner quota multiplication. Target lookup, complete entry scans, transfer sorting and call-vector encounter-order discovery are charged before owned expansion allocations.

Represent the shared last-declaration main-call vector separately as target indices in first source encounter order, with duplicates removed. Earlier declarations retain their empty vectors in this bounded main-only call scope. Do not infer this field from reachability or from the call frame owner. Preserve existing GenAssign visitation sorting and virtual elsif Then/Else ownership.

Initially admit only globally disjoint explicitly labelled sequential procedures called from main. Procedure-to-procedure calls, recursion, process ownership, scoped renaming, definitions/macros in the procedure-checksum context and unproved modifiers remain named exclusions. Exact header bytes/CRC, public artifact CST/SANY/Validator, finite execution counts and independently checked witnesses remain separate obligations.

## Consequences

There is no DAG, default, checksum scheme or quota change. Fair-header-only CRC detection and strict saved-frame replay #124 remain qualified. Canonical replay cannot count as TLC-mode validation. The current implementation's policy units conservatively bound owned transfer expansion; they do not count exact machine instructions or Rust allocations. Mandatory portable raw/source records retain original failures and current controls without whole-family certification.
