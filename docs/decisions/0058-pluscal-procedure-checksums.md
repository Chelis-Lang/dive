# 0058: Bounded sequential procedure header AST serialization

- **Status:** accepted
- **Date:** 2026-10-11

## Context

Full editing previously refused procedures that already had native generation.
The declared 132-row C/P selection retains the original 84 procedure cases and
48 new ordinary/default/call-goto/repeated/layout/negative controls. The pinned
`PCalTLAGenerator.translate` generates from a separate exploded `xast`; `trans`
hashes the original AST after translation. Shared nested vectors nevertheless
mutate the original branch bodies. Initial wrong-stage and generation-mutation
inferences, six actual first-run byte mismatches and all primary failures are
retained rather than rewritten as successful evidence.

## Decision

Serialize one disjoint explicitly labelled sequential procedure's original
header AST, parameters/locals, source calls/returns and adjacent CallGoto folds.
Match `ParseAlgorithm.GetProcedure/GetCall`'s mutable `proceduresCalled` alias,
including main calls and repeated-name deduplication. Admit transfer context and
namespace before writing bounded CRC bytes. Retain nested transfers, recursion,
multiple/process-owned procedures, DEFINE/macro combinations, automatic labels,
modifiers and unproved renaming as named checksum refusals. Preserve the existing
raw generator and independent witness guard. No crate edges or defaults change.

## Consequences

Exact original/header AST bytes, separate expanded ownership, generated CST,
SANY admission, Validator outcomes and finite execution/witnesses are separate
proof obligations. Invalid primary artifacts are uncredited. Existing global
work/output ceilings apply; no synthetic expansion or translation-time unrolling
is introduced. Fair-header CRC equality remains D-020's qualified behavior.
F-034/F-035 and issue #200 remain partial beyond this bounded increment.
