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
