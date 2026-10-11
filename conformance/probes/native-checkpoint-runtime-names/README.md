# Native recovery and pure runtime names

The immutable `before/` study uses runtime source `7872bf687ad0b40fd90f7d186f963ec05e621070`, separately packaged in delivery `425474bc61210d3a178f5f373d332c72a01d8d1c`.
`RuntimeStrings.tla` creates `ToString(n + 1)` along a finite safety path. Ordinary native checking under both profiles at requested widths 1, 4 and 8 finds the invariant violation with 4 generated, 4 distinct states and depth 4. Clean pinned Java TLC at one worker, seed 7 and fingerprint polynomial 0 reports the same numbers and verdict. Native checkpoint requests after one completed level refuse before work in every setting. Raw commands, stdout/stderr, input bytes, executable/oracle hashes and the original source bytes are preserved; the receipt hashes every file.

This is a native recovery admission limitation, not a TLC bug. A recovery admission expansion must restore the complete ordered name table, preserve actual inline fallback for TLC encounter order, keep runtime effects and checker/register reads refused, and retain the independent TLC witness replay refusal for history-dependent name creation. Snapshot consistency is not an independent reachability certificate.

## Current admission and evidence

The increment removes only the pure native constructor refusal. The native
`impure()` and checker/register guards remain intact; mixed models refuse before
model work/output/archive creation. Names-only models preserve the existing typed
sequential fallback in TLC value/exploration order, including requested width 1
with chaos. Canonical order uses the requested pool. This does not approve a new
counter default, change the archive schema, or broaden the archive trust claim.

The five complete inputs exercise plain `ToString`, history-sensitive `CHOOSE`,
string `Tail`/`SubSeq`/concatenation with VIEW, concrete symmetry plus VIEW, and
duplicate successors. `tlc-primary-final/` contains all five clean primary runs
at seeds 0, 7 and 14, polynomial 0 and one worker. They all exit 12. Plain,
sequence and symmetry cases each report 4 generated/4 distinct states; CHOOSE
reports 37/37; duplicate successors report 6/4. Original four-case primary bytes
remain separately preserved.

The Rust matrix has ten case/profile wrappers, each spanning both stores,
requested widths 1/4/8, ordinary/chaos scheduling, completed-level boundaries
0/1/2 and cross-width recovery. Its transient storage family cycles through
memory/frontier/log/both; that selection is not an exhaustive family crossproduct.
Every retained run compares the entire concrete report after a documented
metadata normalization. Across stores, only `options.store` and
`options.fingerprint` differ: each exact documented value is asserted before
normalization, and incomplete collision estimates must be null. All remaining
fields, including diagnostics/output/notes/counterexamples/counts, remain equal.

Fresh executable CLI capture uses the complete four-family crossproduct, keeping
all raw outputs, actual worker evidence and archive identity. The reproducer is
`capture_recovery.py DIVE FRESH_OUTPUT`. Canonical witnesses must independently
validate, with an altered Init rejected. TLC witnesses must still yield the exact
named runtime-encounter-order `unsupported`, not an agreement or certificate.
Effects controls and coherent-checksum name-table corruption controls retain
pre-effect refusal or truthful validated-import prefix evidence.

The user guide includes executable CLI and Rust requests. Queue/log budgets
retain their separate accounting scopes; candidate buffers, names, visited
states and archive/reconstruction work remain outside them. Historical source
maps are archived before current integrity maps are refreshed. Fresh source
integrity is not recertification of old runtime captures or a complete recovery
family claim.

## Frozen completed verification (2026-10-11)

[The delivery receipt](delivery.json) separates runtime producer
`d1102e5d6eb66898259f061949ab031ab6cfa334` from verification source
`2e079670d9c232f83304fe1d38b146972c442d2b`. All 626 Rust/Cargo/toolchain
inputs and all 254 recorded production inputs are byte-identical across those
commits; metadata packaging does not retag the executions. The immutable native
executable SHA-256 is
`bcbd0c2fd63008f142929f00bb93fb2006a79cacd574bc5ba25dce0931277b2b`.
Exact producer bytes and historical authorities are archived separately.

The complete CLI selection passed all 1,440 groups and 7,920 executions: five
inputs, four transient storage families, both profiles and stores, widths 1/4/8,
ordinary/chaos scheduling and boundaries 0/1/2, with retained/reset recovery and
independent positive/negative witness controls. Original regression selection
remains 576 groups/4,032 executions plus 96 failure controls. All 15 clean pinned
TLC primary runs and all five concrete native/primary trace comparisons passed.
The exact CLI and Rust guide consumers passed. CM1's 21 models and VTS's 16 models
fully agreed at widths 1/4/8. Reproduce raw capture with `capture_recovery.py`,
primary correspondence with `compare_primary.py`, and verify the retained raw
bytes with `python3 -I conformance/probes/native-checkpoint-runtime-names/verify.py`.

The full local gate passed 928/928 tests with zero skipped and every remaining
gate stage. At two build/test threads, nextest took 2,140 seconds and the full
gate took 2,911 seconds under unrelated host virtualization and swap pressure;
no process was changed. New case/profile wrappers took 112–132 seconds, below
the unchanged 180-second per-test bound. This is correctness evidence, not a
performance claim or a measured Linux CI duration. The four CI test partitions
select the complete disjoint inventory (242/226/244/216); no check, case or timeout
was dropped. Initial failed test expectations remain in `development/`.

F-032 stays partial. Random state, other runtime effects, checker/register reads,
temporal recovery, Java checkpoint byte interchange and independent TLC
runtime-name witness replay are not admitted by this increment. Encoded queue
and resident predecessor budgets remain separate from names, visited/candidate
state and archive/reconstruction allocations; no aggregate memory bound, archive
authentication or complete recovery-family certificate is claimed.
