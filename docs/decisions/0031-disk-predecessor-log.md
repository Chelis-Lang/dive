# 0031: Separately budgeted transient predecessor logs

- **Status:** accepted
- **Date:** 2026-10-10

## Context

Safety BFS retains a parent ID and successor ordinal for every discovered state.
Spilling an ordered frontier does not bound this growing predecessor log. Concrete
counterexamples must still replay the original representative, including symmetry
and VIEW, rather than recover a state by matching a fingerprint.

## Decision

Keep the memory log as the default/reference. Opt-in
`--predecessor-memory-bytes N` bounds committed parent/ordinal vector capacities
separately from all other storage. Zero goes straight to disk. A private transient
subdirectory lives under `--predecessor-directory DIR`, or the system temporary
directory if no parent is supplied. A parent without a budget is rejected before
input/model effects. The TLC-equivalent setting is the existing no-budget memory
path; disk storage changes neither enumeration nor stop selection.

Use one fixed-record file, with implicit discovery IDs and no resident offset
index. Its 24-byte header is `DIVEPR01`, little-endian version 1, record width 16,
and eight reserved zero bytes. Each record is parent u64 (all ones for roots),
ordinal u32, and FNV-1a u32 over little-endian ID/parent/ordinal. Checksums detect
accidental corruption, not hostile replacement. Parents must be strictly earlier.
The reader checks header, exact total length, checksum and parent order. Closing
scans every record, including records outside the selected witness, and removes
owned files before certification. All file/allocation/corruption/cleanup failures
invalidate completion while retaining counts, actual workers and the preceding
search event. Confirmed write bytes exclude partial failed writes.

Disk reads use one fixed 16-byte record. Opt-in reconstructed paths admit at most
1,000,000 steps with fallible vector allocation. Paths, decoded states, allocator
workspaces, frontiers, seen stores, per-source candidates and graphs are outside
the predecessor budget. In particular this is not a total RAM bound.

Ordinary safety BFS is the admitted mode. DFID, simulation and behavior-graph
capture reject this setting explicitly until separately implemented and checked.
These files are not restartable checkpoints or a recovery API.

## Consequences

Both stores and value profiles can retain predecessor records on disk without
changing concrete replay. Disk writes and close-time validation cost I/O. Long
witnesses can hit the named reconstruction bound rather than allocate without
limit. Seen/candidate/graph/SCC spilling and native checkpoints remain separate
work. No crate dependency edge changes.

Evidence: `crates/dive-store/src/predecessor.rs` corruption and failure controls;
`crates/dive-api/tests/predecessor_spill.rs` zero/tiny-budget equality and independent
symmetry witness controls; `docs/predecessor-storage.md` records the delivery checks.
