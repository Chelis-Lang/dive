# 0084: PlusCal parsing and translation

- **Status:** accepted
- **Date:** 2026-10-12

## Context

TLC ships the PlusCal translator (`pcal.trans`). dive translates PlusCal natively
in `dive-pluscal`, so translation needs no JVM. More than twenty increment ADRs
widened the admitted subset one shape at a time: sequential code, labels,
processes, fairness, macros, procedures, definitions, checksums and block
editing. This ADR records the decisions that remain, without that history.
User-facing detail is in `docs/pluscal-translation.md`,
`docs/pluscal-parser.md` and `docs/user/pluscal.md`.

## Decision

**Pipeline.** Translation runs in separate stages: source parsing, macro
expansion, label and control-flow normalization, generation, and admission of the
enclosing module. `parse`/`parse_at` produce an unnormalized source AST that
keeps the original bytes and half-open byte ranges. Each public call runs on its
own bounded-stack worker, with explicit byte, token, node, depth, action and
output limits. A limit gives a named resource result. A worker failure gives
an internal diagnostic. No partially accepted output is ever returned.

**Admission.** A translation is returned only after the complete generated
module passes dive's native syntax, name and level analysis. Translation never
claims a model proof or exhaustive exploration (`model_proved=false`,
`exhaustive=false`). Any construct that is not supported yet is a named refusal,
never an approximation. Correctness is judged against the pinned translator by
comparing output, together with generated-model checking and independent trace
replay. The comparison uses significant tokens and the operator CST. A primary
artifact that SANY rejects earns no agreement.

**Semantics followed.** dive matches the pinned translator's observable output
for the admitted subset:
- label insertion rules: the sequential default, `-label`, `-labelRoot` and
  `-reportLabels`, and the process rule that every missing label needs `-label`;
- `defaultInitValue` for omitted initializers, returned as required
  configuration data;
- simultaneous-assignment snapshots and the program-counter omission rules;
- finite processes with disjoint ID domains and names;
- fairness declarations and label modifiers, with option precedence for `-wf`,
  `-sf`, `-wfNext` and `-nof`;
- macro substitution in source order;
- sequential, nested, recursive and process-owned procedure calls, using stack
  frames rather than unrolling;
- global `define` operators and functions, including LOCAL.

Name rewriting and disambiguation, unproved multiline substitution and other
unproved scopes remain refusals. Both profiles use the same translation.

**Editing and checksums.** `dive translate --rewrite` returns an in-memory
replacement plan for the translation block and never writes the input file. The
default `Full` checksum policy computes the algorithm AST checksum and the
generated-body checksum natively, for every shape whose serialization is proved.
Other shapes refuse rather than emit a guessed checksum. `--checksums tla`
selects body-only checksums, and a TLC-mode run that uses it is labelled
`tlc+overrides`. Replacing a modified, missing or disabled old body checksum
requires an explicit grant. Checksums are a kept functional feature. The goal is
interoperability with TLC's translation-change detection.

**Dependencies.** `dive-api` depends on `dive-pluscal`, and `dive-pluscal`
depends on `dive-syntax`. dive does not call Java at runtime. `dive-trace` stays
independent of the translator.

## Consequences

- New PlusCal support extends this pipeline and `docs/pluscal-translation.md`.
  It does not need a new ADR unless it changes a decision above.
- The translator's internal reference vectors and conservative reservation units
  are implementation details and may be simplified.
- Disk writes, the TLC map file format, exact pretty-printing widths and
  historical options remain open work, listed in the translation guide.

Supersedes: 0022, 0026, 0030, 0032, 0035, 0037, 0039, 0041, 0042, 0043, 0044,
0045, 0046, 0047, 0049, 0053, 0056, 0058, 0061, 0066, 0069, 0073, 0077, 0079.
