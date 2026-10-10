# ADR 0042: Native nonrecursive sequential procedure graphs

Status: accepted for the bounded increment; full family certification remains pending.

## Context

ADR 0041 admits one disjoint sequential procedure. The parser already owns full
procedure declarations, calls, returns and source spans, but native translation
refuses multiple procedures and every nested call. Issue #163 extends that native
path without a Java runtime adapter. The pinned `PcalTranslate` distinguishes
ordinary calls, call/goto, returns and immediately adjacent call/return; the last
replaces the stack head and must never be implemented as an approximate push/pop.

## Decision

Admit multiple sequential procedure declarations only with proved disjoint
names and a nonrecursive call graph. Main and each procedure keep their own
source body, action entry and return owner. Parameters, locals and frame records
belong to the source procedure; declaration/action/wrapper ordering follows the
pinned translator. Name rewriting, process-owned stacks, macro/procedure
interactions, automatic procedure labels and unproved fairness are named refusals.
A cycle remains a refusal even in an unused or conditionally unreachable body.

Capture the complete C/P selection before native changes. Keep translator errors,
normalization projection, primary artifacts and SANY dispositions separate. An
invalid primary output is diagnostic evidence, never an admitted artifact match.
Every admitted artifact must pass enclosing-module analysis and operator CST
comparison against the primary, not only significant-token equality.

Normalization owns one private expanded arena. Scope processing borrows that
arena; it must not clone all statements or declarations once per procedure or
once per call. Call/return nodes retain bounded procedure indices. Action and
normalized-node quotas are global and charged before accumulating each next
procedure's output. Generated-byte and source/depth bounds remain effective.

Generated model exploration/count/depth controls use clean pinned TLC and native
runs at 1/4/8 workers in fingerprint/exact stores. The independent strict replay
namespace guard (#124) remains intact: pending witnesses are not fully agreed
TLC violations. Default/canonical replay is separately labelled qualified
positive/mutant evidence and can never certify the strict profile.

## Consequences

No crate dependency graph or external dependency changes. Full F-034/F-035
certification remains pending, including recursion, scopes/renaming, process
storage, options/rewrite/checksum/map contracts and strict namespace-provenance
validation. The executable user guide, raw evidence, failure history and source
receipt are delivered with the implementation.

## Reference

Pinned commit `94d0c50`: `PcalTranslate.ExplodeCall`, `ExplodeReturn`,
`ExplodeCallGoto`, `ExplodeCallReturn`; `ParseAlgorithm` call/return folding;
`PcalSymTab` procedure entries and `PcalTLAGen` declaration/Init/wrapper ordering.

## Retained evidence

All 120 capture-first C/P rows remain: 68 artifacts, 19 matching rejections,
29 named refusals and four invalid-reference diagnostic dispositions. Eight
primary SANY-invalid outputs remain uncredited. All 68 admitted artifacts pass
separate enclosing SANY and operator CST comparison. The 130 clean model rows
reproduce verdict/count/depth at 1/4/8 workers in fp/exact stores (780 controls):
64 complete matches, 62 invariant witnesses and four deadlocks with strict
namespace replay pending. Original strict disagreements are preserved.
Separately qualified default replay validates 62 invariant witnesses and rejects
264 mutants, while four deadlock endings remain unsupported; this confers no
TLC-profile certification. The stopped implicit-fallthrough success expectation
and its primary exit-11 capture remain archived.
