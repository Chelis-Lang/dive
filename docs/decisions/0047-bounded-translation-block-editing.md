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
