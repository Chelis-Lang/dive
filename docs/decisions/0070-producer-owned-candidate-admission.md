# 0070: Independently admit privately produced temporal words

- **Status:** accepted; implementation controls in progress
- **Date:** 2026-10-11

## Context

Issue #208 services genuine requests after actual source/graph/frontier publication.
Its selected candidates are diagnostic, with no model-proof authority. Issue #213
requires a composition that cannot accept caller-created graphs, traces or Valid
flags and that survives fresh binding/name tables.

## Decision

Add `dive_api::periodic_candidate::check` accepting only original IR/configuration
and selected options/limits. It owns snapshot binding and real request execution.
The separate `Snapshot::execute_candidate_source` retains actual successful source
identities/values. An optional Session audit inspects original admitted material
fingerprints before successor deduplication and recursive writes; differing concrete
values with one fingerprint refuse. Legacy `execute_service` resource policy stays
unchanged. Audit entry retention uses the existing capture-state ceiling and Session
material/work meter; source metadata and reconstruction use the shared graph quota.
These are structural/policy bounds, not instruction, heap or comparison-complexity
guarantees.

Select only the real terminal candidate: original request/checkpoint/source facts
or genuinely successful safety EOF/final selection. Preserve product identities,
SCC/anchor/closure fields and pinned contiguous fingerprint packing. Missing source
representatives, unwritten records and ambiguity refuse instead of inventing values.
Attribute complete original configured property claims without relabelling derived
roles. Rehome a lossless private word into fresh binding A for independent memory
validation, then serialize/parse and freshly bind B for independent JSON validation.
Only two actual Valid results construct the private witness.

## Consequences

This certifies a concrete violated behavior only. Partial success and no-candidate
EOF never mean Holds. Ordinary CLI/schedule/count/queue/checkpoint/model-proof
contracts remain unproved/refused; F-023 remains missing. Diagnostic failures and
resource/refused replays preserve the actual run. Existing configured-hook source,
effect, ownership, profile and finite replay restrictions remain in force. See the
[executable user guide](../producer-owned-temporal-candidate.md).
