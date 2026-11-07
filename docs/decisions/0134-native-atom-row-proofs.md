# 0134: Reuse positive native atom input proofs within one graph analysis

- **Status:** accepted; input-row admission amended by [0154](0154-native-atom-rows-by-materialization.md)
- **Date:** 2026-11-01

## Context

ADR 0129 and ADR 0130 require actual set-free closed input rows before optional
native atom evaluation. The native reader repeats the same bounded structural
check for every atom demand, including a source shared by several action targets.
All twelve original ABA atoms already qualify and are consumed without fallback;
expanding descriptor eligibility does not address that repeated work.

A nested success-only equality shape proof would affect TLC comparison guards.
Default equality and membership return before those guards. Set normalization
can use shape proofs in both profiles, but no current original ABA measurement
establishes that consumer as the default atom bottleneck. Neither the older
TLC-only profile nor the observed default/W8 capacity loss on different VMs
establishes a current causal attribution or a promised capacity improvement.

## Decision

A private native Reader owns positive row bits for its borrowed immutable
NativeGraph. Its constructor creates fresh proof state for each actual periodic
or final analysis invocation. The borrow fixes the exact row namespace and
persistent Store owner; no raw graph address, fingerprint, extensional equality,
reference conversion result or atom truth is a proof key. NativeGraph's existing
`closed_rows` remains solely the count of completed CSR sources.

After the existing owned-control and retained-descriptor checks, the first
actual native demand for a row runs the unchanged `closed` validator. Only its
complete success issues a bit. Source and target each retain their independent
4,096-component/string-byte input allowance. Later demands for that exact row
can reuse the positive fact after checking the current Machine Heap poison and
exact Store owner. Store identity cannot be inferred from persistent coordinate
bits. A failed or over-quota validation issues no bit and is scanned again on
later demands. Machine evaluation retains its separate 4,096-unit fuel.

The bit vector is allocated lazily on the first admitted native input demand,
with fallible reservation, checked size arithmetic and at most one MiB requested
and retained capacity. A graph requiring more storage, allocation failure or an
unexpected larger capacity leaves proof storage empty and uses the original
validator on every demand. This optional storage does not produce a resource
failure or replace the run's cooperative time and sampled-memory controls.

Eager conversion before every graph admission and every existing demanded
reference read remain unchanged, including seen originals, names and error
priority. Preserve exact descriptor identity, first physical original target,
source-before-target input checks, Nonstuttering subscript-before-action order,
warning/event checks, reference fallback and owned stop authority. Reader drop
releases its bits and records additive scans, hits, certifications and peak
retained proof bytes; it precedes fresh independent candidate replay. No public
reader trait, CLI switch, backend, dependency edge or divergence is added.

## Consequences

Both profiles can avoid repeated input validation without widening native
semantic admission. Focused consumer controls exercise positive reuse, copied
atoms, set and work refusals, separate source/target quotas, Store ownership,
poison after certification, allocation/cap bypass, semantic fallback and owned
stops. Existing native/reference reports and independent replay remain required.

The additive `core.metrics.liveness.atoms.row_proofs` counters measure consumed
proof reuse, not time saved. Original ABA closure cost and complete dispatch
benefit remain unmeasured by this change. A bounded matched consumer measurement
must justify any performance claim; absent or weak benefit stops this
intervention before broader qualification. Original capacity still requires the
unchanged original workload, normal limits, observable results and independently
validated counterexamples. No 900-second completion or matrix acceptance is
claimed here.
