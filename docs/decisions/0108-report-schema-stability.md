# 0108: One report version and explicit check families

- **Status:** accepted
- **Date:** 2026-10-21

## Context

The approved agent-ergonomics plan requires JSON schema 2 for every operation
and a stability guarantee for reports and native traces. Safety reports used
version 1 while temporal reports used version 2, coupling transport version to
execution strategy. The harness treated every version-2 check as temporal and
study-final rewrote safety evidence to version 1 before decoding it. Oyster's
pilot accepts version-2 checks but its replay reader accepts validation report
version 1 only. A producer-only bump would break independent replay there.

## Decision

All public operation report writers use `dive_api::REPORT_SCHEMA_VERSION = 2`.
Existing public per-operation constants remain aliases. Check reports add the
closed `check_kind` discriminator `safety` or `temporal`, identifying the existing
request/report family. It never certifies that an engine ran or capture completed.
The existing temporal-family predicate and strict execution checks stay in place;
final/short requests with safety failures retain their incomplete-capture facts.

Readers retain legacy report-version-1 safety and frontend input support and
legacy version-2 temporal input support. New safety version 2 must explicitly
identify its family and must not carry temporal options, execution or properties.
Unknown versions/kinds fail closed. Study-final shares the ordinary safety mapper
only after validating its final/short mode and options wrapper, without rewriting wire data.
Native `dive-trace` remains version 1. Conformance reports, baselines, goldens and
checkpoint formats are independent and do not change.

The maintained policy in `docs/user/reports-and-errors.md` freezes existing names,
types, presence and meanings within a major version. Optional fields and open
diagnostic/resource/phase identifiers may grow; closed outcomes/tags may not.
Breaking changes require a new version, CHANGELOG migration note and advance
notice to known consumers. Schema 2 was announced in Oyster issue 61 before the
writer change lands; its pin must move only after its readers and actual pilot
requalification pass. Rust library API stability remains a separate milestone.

## Consequences

Consumers distinguish transport version from proof scope and can cache reports
without guessing the execution strategy. Old immutable pins remain usable with
the harness. Current writers no longer have a version-1 mode; consumers migrate
before updating their pins. Tests retain strict temporal validation and genuine
plus forged trace replay, and cover each operation writer's report envelope.
