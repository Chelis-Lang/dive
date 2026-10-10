# 0030: Bounded sequential label and initialization normalization

- **Status:** accepted
- **Date:** 2026-10-10

## Context

ADR 0026 admitted explicitly labelled sequential algorithms. The pinned translator
also inserts labels and synthesizes initial values. Those are semantic stages:
where labels are placed determines atomicity and the generated state graph.
Option names alone do not establish their defaults.

## Decision

Port `ParseAlgorithm.InnerAddLabels` for the admitted sequential C/P subset. Track
assigned variables across alternatives, clear them at action boundaries, require
labels at loop/control-transfer boundaries and retain the reference `with` input
set rule. Traverse source statements on a private parsed value; source AST APIs
remain unchanged. Generated names carry separate original statement associations
and reference display positions rather than fabricated source spelling.

Match the measured default: a sequential algorithm without any explicit labels
gets inserted labels. With an explicit label present, missing labels are rejected
unless `insert_missing_labels` / `--label` / `-label` is selected. The
`label_root` / `--label-root` / `-labelRoot` prefix defaults to `Lbl_` and does not
enable insertion. `--report-labels` / `-reportLabels` enables insertion and exposes
label metadata in native JSON; historical human message formatting is pending.
Root syntax follows the primary ASCII-letter/letter-digit-underscore contract.
A separate 256-byte root resource limit is reported as a resource refusal.
Inserted labels and generated actions are bounded before retention.

An omitted global initializer becomes `defaultInitValue`, declared as a constant.
The artifact returns `CONSTANT defaultInitValue = defaultInitValue` as required
configuration data. It does not write a `.cfg` file. Unproved name collisions are
refused; complete enclosing-module analysis still precedes artifact acceptance.

Sequential `L:+` and `L:-` modifiers force retention of the program counter,
matching the measured primary translation. They do not introduce process-label
fairness formulas in a sequential algorithm. A `--fair algorithm` retains
`WF_vars(Next)` as before. Process fairness, global fairness option spelling and
branch-internal labelled expansion remain separate pending contracts.

Default and TLC profiles use the same reference normalization settings. No new
result-changing default or dependency edge is introduced. Generated models still
use the independent evaluator/trace validator during development evidence;
normalization and translation never assert a model proof.

## Consequences

The complete 110-row normalization matrix retains clean primary translation and
SANY results, plus a clearly labelled development-only normalized-AST adapter.
Inserted names, locations and initialization status are compared separately from
artifact tokens/CST, module validity and finite model/witness behavior. All prior
translation inputs remain retained. Refused branch/name/process/procedure/macro
cases remain in denominators. F-034 and F-035 stay partial and audit-required.
Full translation options, pretty-width, file rewriting/checksums and upstream
source-map formatting require later independent matrices.
