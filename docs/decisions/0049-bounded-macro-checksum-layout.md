# 0049: Bounded macro checksum token layout

- **Status:** accepted
- **Date:** 2026-10-11

## Context

Pinned `ParseAlgorithm` expands macro calls before the first captured AST.
`AST.toString` omits macro declarations and serializes expanded expressions.
Native provenance spans identify argument and definition text, but those byte
positions do not describe the logical token lines that the Java substitution
constructs. Reusing physical source positions produced false serialization
line breaks for otherwise supported single-line actuals.

## Decision

Keep provenance spans unchanged and use a separate logical single-line layout
for substituted expressions. Support only shapes with exact pinned AST-byte
and CRC evidence. Multiline actuals and substituted junction columns remain
named refusals; unchanged expression layout retains its existing serialization
contract. The embedded selector uses a typed bounded grammar-owned end, matching ordinary
trailing prose and ASCII-star decoration inside the selected comment without
loosening standalone EOF. Malformed quote/comment lexical state is still a
deciding error. Raw low-level byte selection retains its error-stage contract
when no complete algorithm can be proved.

Raw serialization is independent from generated module admission and model
checking. Invalid primary artifacts stay uncredited. Retain original arbitrary trailing
prose false rejections before the proved ownership fix, and retain the
fair-header-only CRC weakness.
The public enclosing-module admission places generated declarations after the
complete algorithm closing line, matching the pinned ordering before user
definitions. The original `Lock` forward-reference rejection and paired C/P
positive/unknown-name controls remain evidence; module analysis is not relaxed.
Caller-supplied invalid selection spans produce UTF-8-bounded diagnostics.

No new default semantic or integrity correction is introduced.

## Consequences

The existing bounded macro translator can derive Full checksums for proved
single-line shapes without confusing source provenance with logical layout.
The retained selection and public API controls must accompany further scope
extensions. Full PlusCal, exact formatter/file-writing and general scoped
renaming remain pending. Dependency edges and resource quotas are unchanged.

The source wrapper also retains the measured same-line tail contract: Java
`String.trim()` removes characters through ASCII 0x20; any remaining tail must
begin `\*`. Source outside that contract rejects before artifact generation.
Standalone and Full-edit APIs share this reference boundary; full parsing and
module analysis are retained separately.


## Marker ownership follow-up

Source5 passed its complete 316-row raw and 632-row public selection plus the
1,093-test gate, then outside-selection review found an existing main selector
wrong answer. Whole-comment unfair-marker precedence could translate a later
algorithm instead of an earlier-line fair one. Native ownership now ports
`trans.java`'s line-by-line selection and same-line unfair priority, with 32
additional primary controls. The original secondary stage adapter had the
opposite same-line priority; a separately named adapter corrects it for these
new rows. Primary bytes, earlier source5 results and all failed native controls
stay separate. No corrected checksum default or broader family certification
follows from this grammar-boundary repair.

Six no-tail fair-marker controls ensure discovery and ownership use the same
literal line-scoped marker rule. The intermediate source6 named refusal is kept
with its exact binary/source identity; no full gate or broad runtime campaign
was claimed for that intermediate release build.
