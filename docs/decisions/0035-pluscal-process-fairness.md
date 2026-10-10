# 0035: Bounded PlusCal process fairness and label modifiers

- **Status:** accepted
- **Date:** 2026-10-10

## Context

ADR 0032 admits scalar constant-ID and indexed membership-process storage.
The same action normalization can generate fairness without changing the state
graph, but fairness conditions must preserve label guards, quantifier scope and
the distinction between weak and strong fairness.

## Decision

Admit weak/strong declarations and `+`/`-` label modifiers for the existing
single-block source and naming subset. Match the pinned `ParseAlgorithm`
declaration/option precedence and `PcalTLAGen.GenSpec` formulas. `-wf`/`-sf`
supply fairness only for a process without an explicit declaration. `-wfNext`
adds fairness of `Next`; `-nof` suppresses process and global fairness in
multiprocess syntax. A sequential `--fair algorithm` overrides `-nof`/`-sf`
with weak fairness of `Next`, as the pinned parser does.

For a fair process, minus labels restrict its fairness action by `pc[id]` or
`pc[self]`. Weak process fairness adds strong fairness of each plus-labelled
action; a strong declaration does not add redundant plus-label conjuncts.
Membership processes quantify the complete process formula over their original
domain. Constant-ID processes keep unparameterized action names. Modifiers keep
the program counter even in an otherwise optimized `while TRUE` body.

Preserve the reference conjunction grouping decisions, including its documented
`ProcessFairness.singleLineWidth` omission of a sole prefix line and conjunction
separator. Full operator CST comparison supplements significant-token equality.
Pretty-width, file rewriting and map/checksum parity remain pending. Fair domain
expressions containing multiline/comment/tab normalization remain named refusals
until their prefix/width transformation is proved. No runtime oracle dependency,
new dependency edge or result-changing default is introduced.

Expose the fairness setting through `TranslationOptions`, the native API and
the four pinned CLI spellings. The default preserves source declarations in both
profiles. Accepted translation artifacts still report no exhaustive exploration
and no model proof. Translation support does not certify native liveness
checking or counterexample selection.

## Consequences

Retain complete malformed-label and corrected positive selections separately,
all original process/sequential matrices, and wrapped-formula boundary controls.
Separate SANY and full-module CST checks validate every admitted artifact;
finite model and independent witness controls check generated action behavior.
Comparator mutation controls forbid proof, exhaustive, disposition and metadata
forgeries. Full PlusCal and API families remain partial and audit-required.
Multiple blocks, procedures/macros, renaming and historical generation modes
require subsequent increments.
