# 0047: Bounded translation-block editing and body checksum interoperability

- **Status:** accepted
- **Date:** 2026-10-11

## Context

The pinned `pcal.trans.performTranslation` replaces a translation section in a
module, normalizes line endings on Unix, and records CRCs for the normalized,
name-fixed algorithm AST and generated TLA. `Validator.checksum(Vector)` feeds
concatenated UTF-8 lines into CRC32 without newline separators. Algorithm CRC
requires its exact AST serializer after `removeNameConflicts` and the generator's
`PcalTranslate.Explode` mutation (explicitly recorded in `Validator.java`);
significant-token equality does not prove it.
The capture-first study preserves all 258 selected rows, including original
malformed C controls and corrected follow-ups, callback behavior and invalid
upstream artifacts. Java adapters are development-only reference evidence.

## Decision

Expose a bounded, pure source replacement plan and an admitted native-generation
API. The default `Full` checksum policy refuses until normalized/name-fixed AST
serialization is proved. Explicit `TranslationOnly` (`--checksums tla`) disables
algorithm checking with the upstream `\\in STRING` sentinel and records the
proved translation-body CRC. Compatibility requests with that policy are
reported as `tlc+overrides`. An existing full algorithm checksum is retained as
unverified selection evidence and disabled in the new header, never claimed valid.

A modified, missing or disabled old body checksum requires an explicit replacement
grant. Preserve unchanged source/newlines in the default profile; compatibility
normalizes LF as measured. Selection rejects ambiguous/non-comment marker tokens,
multiple algorithms/sections and unproved placement. Source, generated body and
output byte budgets are enforced before construction. Public generation checks
both the generated artifact and resulting enclosing module. Pure editor checks
on supplied primary bodies earn no generation, artifact-admission or model credit.

The CLI returns an in-memory plan and never writes the input. File writes, exact
primary generator pretty bytes, mapping format and full AST checksum remain
pending. There is no dependency-graph change and no new runtime Java dependency.

## Consequences

Callers can review byte intervals and save an admitted module explicitly. Body
integrity, algorithm integrity, TLA admission and model proof remain separate.
The six fair-algorithm controls expose an upstream algorithm-CRC collision (D-020);
this increment does not fix it or adopt a new result-changing default. A later
increment must prove the full serializer independently before offering full CRC.

## Addendum: proved post-translation AST checksum subset

Full requests now serialize the pinned `AST.toString()` after name-fix and
translation/Explode for the admitted bounded sequential context. Exact native
bytes are compared to each corresponding primary capture, separately from
parsed/name-fixed-stage observations. The serializer retains reference newlines,
expression-token encoding and C block flattening; unproved name fixes, processes,
procedures, macros, definitions, multiline/junction tokens, elsif, multiple with
binders and branch-internal labels refuse without producing a CRC. Existing
normalization admission and budgets remain required. No dependency edge changes.

`algorithm_checksum_verified` means the reference CRC derivation has been proved
for the requested shape. It is not integrity certification or source identity.
`algorithm_matched` compares the recorded reference CRC only. D-020 is deliberately
reproduced: a fair header hashes only `fair`, with `fair_header_crc_only=true`. No
stronger default is adopted. Old algorithm CRC absence/disablement/mismatch needs
the same explicit replacement grant as a modified body. Body-only requests remain
explicit overrides and broader generation remains separate from full serialization.

The AST study retains original failed controls, all source-stage rejections and
name/default/label-context refusals. API/CLI output is independently checked for
nonproof flags, exact spans and CRCs, then admitted rewritten artifacts are checked
with SANY and the pinned Validator, including corrupted header/body/source cases.
The supplied-primary-body editor comparison earns exact editing-byte evidence
only. Broader AST shapes, exact generator formatting, file writes and mapping
format remain pending; no feature-family certification is inferred.

## Addendum: nesting and expression-layout serialization

The next bounded increment follows `ParseAlgorithm.GetIf` and
`InnerGetWith`: `elsif` becomes nested else-if records and multiple `with`
binders become nested records in source order. Native serialization opens and
closes those records iteratively, so long flat binder/branch vectors do not add
serializer recursion. Parsed nesting, node and output quotas still apply.

`TLAExpr.normalize` and `toString` retain significant-token line vectors,
intervening empty/comment lines and normalized ASCII junction columns. A single
source pass records UTF-16 columns with eight-column tab stops; expression
lookup does not repeatedly rescan the whole source. LF/CRLF are proved in the
retained controls, including supplementary Unicode in comments. Bare CR,
unproved substituted token positions and proof-step-shaped lexical tokens
remain named refusals. Source-level name fixes, macros, processes, procedures,
definitions and branch-internal labels are not broadened by this increment.

The clean reference accepts a trailing separator before a C `with` close
parenthesis. Native parsing now accepts that bounded case while retaining
empty/missing-first/doubled-separator rejection controls. Both original
disagreements and the complete initial C separator failures are preserved in
the study; no raw selection or primary invalid artifacts are removed.

Serialization credit requires native post-translation bytes equal to each
primary capture. Full rewrite admission additionally requires enclosing SANY,
operator-CST equality and independently invoked Validator checks. Finite model
and strict witness controls remain separate. The fair-header CRC weakness and
nonproof/integrity qualifications are unchanged; no default correction or
crate dependency change is adopted.

The retained empty-Else CST failure exposed a generator fidelity gap: reference
`PcalTLAGen.GenIf` emits `TRUE` before branch-identity updates. The narrow fix
retains that conjunct and admits explicit P empty Else after a nonempty Then.
Empty Then parses but generation remains a named refusal; C empty bodies and P
both-empty conditionals retain measured rejection outcomes without crash
emulation. Multiline AST derivation is proved separately, while native multiline
layout/substitution generation remains unsupported. No raw checksum match grants
translation admission.

A separate terminal-empty-elsif capture demonstrates four initially admitted
raw ASTs that the pinned parser rejects: `GetIf` resolves its final statement
from the innermost Then/Else pair, not from any earlier branch. Native P parsing
now rejects that empty terminal pair. All 16 supplemental sources and four
original disagreements remain retained; the active old gate was interrupted
and the old 304-row campaign completed before creating the corrected producer.

## Addendum: bounded non-junction multiline generation

`PcalTLAGen.AddSubscriptsToExpr` retains lexical changed-variable substitution;
`addExprToTLA` transports normalized lines and indentation. The next generator
slice admits sequential expressions without junction-list tokens: significant
token order and existing lexical substitutions define the same operator tree
when whitespace is normalized. Multiline `/\`, `\/`, Unicode/alias junctions,
binders, macro declarations and process/procedure contexts remain named refusals.
Neither layout-sensitive grouping nor substitution ownership is guessed.

Complete primary and original-refusal captures precede admission. Significant
whole-module CST equality, pinned SANY acceptance, Full/body CRC and Validator
corruptions qualify artifacts separately from finite model/witness controls.
Invalid primary output is not admitted or counted as successful conformance.
Global source/token/node/depth/action/output/document budgets are unchanged.
No new dependency edge, formatter contract, checksum default or D-020 correction
is introduced. The executable user contract is `docs/pluscal-multiline.md`.
