# ADR 0043: Native recursive sequential procedure calls

Status: accepted for the bounded disjoint sequential increment; full parity remains pending.

## Context

ADR 0042 preserves a bounded nonrecursive graph increment. Issue #163 now
extends the same native path to recursive source call graphs while keeping
procedure/global/label names disjoint. A cyclic source graph is not itself an
unsupported translation algorithm: the pinned translator emits stack operations
and recursive program-counter transitions without unrolling the calls.

## Decision

Translate ordinary self and mutual calls using the existing source-owned frame
push/return rules. Same-procedure immediately adjacent call/return uses the
pinned `PcalTranslate.ExplodeCallReturn` special case: no caller-local restore or
stack-head replacement, simultaneous parameter assignment, ordered local
initialization and the original procedure entry. Cross-procedure call/return
retains its distinct head-replacement rule. All formal/local/return owners remain
source-owned; no runtime TLA approximation or translation-time unrolling occurs.

Retain the complete capture-first C/P selection, all translator errors, AST
normalization projections, primary artifacts, SANY rejections and previous native
refusals. Invalid primary output remains diagnostic only. Admission requires
native enclosing-module analysis and independent SANY/operator CST controls.
Unproved scopes/renaming, process-owned frames, procedure-label insertion/modifiers
and procedure/macro interactions remain named refusals.

Translation bounds count source and emitted normalization/actions globally; they
must not scale with a runtime recursive depth. Runtime finite-model controls use
clean pinned TLC. Truly unbounded stack/counter behaviors require explicit
native budget results and must never be credited as complete proofs. A finite
recursive tail self-loop is a complete finite state graph only if the reference
admits its artifact; a safety result is not a termination proof.

Strict procedure-stack replay stays pending where the #124 encounter namespace
cannot be established. Separately qualified default/canonical witness and mutant
checks confer no strict TLC-profile credit. No existing independent guard is
relaxed. Unknown/undecidable returns or exhausted resources remain named results.

## Consequences

No dependency graph changes or external crates. The earlier graph commit,
reports, failed campaigns and copied binaries stay immutable. New executable
CLI/API documentation and a source/evidence receipt accompany this increment.
F-034/F-035 stay partial and audit-required; process/scoped renaming, historical
options/rewrite/map parity and complete strict witness certification remain
separate work.

## Provenance

Pinned `94d0c50`: `pcal.PcalTranslate.ExplodeCall`, `ExplodeReturn`,
`ExplodeCallGoto`, `ExplodeCallReturn`; `PcalTLAGen.GenAssign`, procedure/global
changed-variable rules, declaration/action/Init/wrapper/Next ordering;
`ParseAlgorithm` call/return folding and `PcalSymTab` procedure entries.

## Evidence boundary

All 116 captured C/P source rows remain selected. Native 86 artifact matches are
separately SANY/operator-CST checked; all ten primary-invalid outputs remain
uncredited. The generated-model campaign has 72 strict complete matches and 74
strict witness disagreements/pending namespace checks. Canonical replay (74
valid witnesses/296 rejected mutants) is separately qualified, with no strict
TLC credit. Bounded capture retains 12 primary timeout prefixes and 192 native
nonproof stops, including the existing strict runtime-name refusal. Earlier
failed expectations and raw campaigns remain beside the final disposition.
