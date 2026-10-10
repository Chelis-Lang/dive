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
