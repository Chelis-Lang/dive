# Fallible disk seen-state library

This study covers the additive `dive_store::seen` library (issue #205,
[ADR 0063](../../../docs/decisions/0063-fallible-disk-seen-state-library.md)).
It does not expose a checker storage option, change the infallible `StateSet`
trait, or establish an ordered semantic discovery prefix. The ordinary engine,
checkpoint and total-memory boundaries remain unchanged.

The current source owns fingerprint/full canonical-byte identity, bounded
on-disk open-addressed indexing and sequential rehash, first-error poisoning,
audited owned cleanup and a bounded fallible minimum-gap summary. Tests are
`crates/dive-store/src/seen/tests.rs`; the actual Cargo guide is
`cargo run -p dive-store --example disk_seen --locked`.

Controls retain memory/disk equality, complete u64 identity, exact full-hash
collisions, duplicates, partial data/index writes, late rehash failure, injected
read/seek/write/storage-full/allocation/cleanup failures, corrupt headers,
short/trailing/oversized records, coherent invalid offsets, duplicate index
entries, probe/record/entry/summary bounds and caller-file retention. Error
injection is not a claim that the host disk was filled. Public successful close
requires full index/record reachability audit and explicit cleanup.

`before-current-map/` retains parent current-source metadata and changed original
source bytes. `before-current-map-preflight/` preserves failed source-hash checks
before the additive module export/ledger change was reflected in current maps.
Historical runtime captures remain attached to their original producers. A
current hash refresh alone is not fresh runtime conformance evidence.

The current library producer is `0be524349e4b09f452ee399edc7005653a4825d5`.
`current/` retains its exact 449-input archive, eight-stage full gate (1,179
passed, zero skipped), fresh twelve-control run and compiled Cargo example.
The unchanged 21 CM1 and 16 ValidationTestSuite models fully agree at 1/4/8
workers; these are resident-engine regression controls, not disk engine proof.
`current-source.json` is a separate receiving-source map; metadata integration
does not retag the original producer or executable hashes. The portable verifier
is `python3 -I scripts/check-fallible-seen-store.py`; optional `--binary-root`
checks retained executable bytes too. Hashes verify consistency, not authenticity.

The actual local two-thread full gate took 771.91 seconds, including 446.29
seconds of nextest; the unchanged CI per-test bounds were used. These timings do
not predict Linux completion. `before-verifier-report-order/` retains a metadata
helper's mistaken completion-order assertion: the parallel VTS harness printed
model 2 before model 1. The corrected helper requires every selected index once,
all agreement statuses and unchanged raw output, without imposing print order.

The first library retains zero key/offset collections in RAM, with at most two
record-sized workspaces. It does not bound OS page cache, paths/handles, caller
keys or ordinary resident seen/candidate/trace/graph collections. The quadratic
summary has an explicit pair-policy refusal. Physical insertions are not
semantic committed discovery counts; staged write bytes may exceed confirmed
accounting after failure. See the [user guide](../../../docs/user/disk-seen-library.md).
