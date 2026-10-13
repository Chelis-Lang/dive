# 0089: Verified inputs for the PlusCal measurement tool

- **Status:** accepted
- **Date:** 2026-10-12

## Context

The PlusCal corpus tool used the first JRE found anywhere in the cache and every
cached TLAPS/CommunityModules revision. Extra cache entries could change the
experiment while its report called those inputs pinned. The corrected scorer
still compares native checks of two translations; it is not a TLC model oracle.

## Decision

Select the default gating oracle, host JRE archive and module libraries from
`conformance/oracles.toml`, and corpus revisions from `conformance/corpora.toml`.
Verify artifact SHA-256 and checkout commit/cleanliness before invocation, using
the harness's cache layout and ignored-file policy. Missing or altered pins fail
before selection; unreadable corpus files never silently shrink the denominator.

`xtask` gains direct `toml` and `sha2` dependencies at existing workspace versions.
TOML parsing makes pin field order immaterial; streaming hashing verifies large
artifacts without loading them into memory. Keeping this in the developer tool
avoids pulling the complete conformance/backend graph into every repository hook.
These add no compiler/backend or independent-validator dependency edges.

## Consequences

Old runtime/library revisions in the shared cache cannot substitute for pins.
Reports name the selected Java and library paths. As in the harness, verification
of the JRE archive identifies its unpacked tree; it does not attest every unpacked
file. Prior runs retain their actual original input/binary identity and are not
retroactively credited as runs of this revision. Model parity still requires
`pcal.trans` plus TLC versus native translation/check and independent trace replay.

Native comparison additionally requires an explicit statistics field: legal
startup nulls remain distinct from omission. Present counters must be unsigned
and generated must be at least distinct. This is report integrity, not an
additional model proof or a new policy for depth/counter floors.
