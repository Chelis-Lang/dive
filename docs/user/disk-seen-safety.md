# Disk seen-state safety checking

Ordinary `dive check` can put fingerprint or exact seen keys on disk. Enable it
explicitly; the default keeps the existing resident reference. The disk route
currently runs inline and reports that fallback even when you request workers.
This bounds disk-membership workspaces, not total search or process memory.

Run the shipped pure finite example:

```sh
cargo run -p dive-cli --release -- check \
  crates/dive-api/examples/seen-safety/Counter.tla \
  --compat=tlc --seed 7 --fp 0 --workers 8 \
  --seen-disk-entries 1000 --output json
```

Use `--store exact` for full canonical keys, or omit `--compat=tlc` for the default
profile. The semantic counterpart is the same command without disk flags: keys,
complete safety outcomes, concrete states and stop counts stay the same within
these supported bounds. Requested workers remain eight, but `options.workers.used`
is one and the reason identifies unproved concurrent disk read views.

A parent directory is optional (`--seen-directory DIR`); it must already exist.
The checker creates a private child and explicitly audits/removes its owned files.
Directories alone, malformed bounds and temporal/recovery combinations reject
before loading the model. Valid ordinary binding and ASSUMEs occur before disk
creation, so an actual creation failure may preserve their output while reporting
zero Init/search workers. Caller files are never overwritten or recursively
removed, including unknown files added to the private child.

The Rust entry point is the same checker request:

```rust
use dive_api::{CheckRequest, SeenOptions, Store, check_file};

let request = CheckRequest {
    file: "crates/dive-api/examples/seen-safety/Counter.tla".into(),
    compat_tlc: true,
    store: Store::Exact,
    seed: Some(7), fp: Some(0), workers: Some(8),
    seen_disk: Some(SeenOptions::default()),
    ..CheckRequest::default()
};
let report = check_file(&request)?;
let output = report.to_json();
assert_eq!(output["options"]["workers"]["used"], 1);
assert_eq!(output["seen_storage"]["after_cleanup"]["cleanup_completed"], true);
# Ok::<(), dive_api::ApiError>(())
```

Compile and execute the complete four profile/store controls:

```sh
cargo run -p dive-api --example disk_seen_safety --locked
```

`SeenOptions::limits` / `SeenLimits` expose the same bounded policy as CLI:

| Option | Default | Meaning |
| --- | ---: | --- |
| `--seen-disk-entries N` | explicit opt-in | Maximum completed physical key records; 1 through 4,294,967,295 |
| `--seen-key-bytes N` | 67,108,864 | One record key workspace, from 8 bytes through 64 MiB |
| `--seen-probe-steps N` | 33,554,432 | Maximum index slots examined per lookup/rehash insertion; positive |
| `--seen-summary-pairs N` | 1,000,000 | Pair comparisons for the bounded actual fingerprint-gap summary; zero is valid |

Duplicates still remain duplicates when the entry limit is full. A new state
beyond the limit returns `resource-limit`, never absence, duplicate or success.
Zero entries/probes or out-of-range key bytes are malformed requests. Exact keys
retain all existing canonical bytes; fingerprints retain all 64 bits. Disk
placement does not change the key scheme or comparison semantics.

`seen_storage` separates `admitted_keys`, physical entries and expected disk
lengths before cleanup, actual cleanup facts, first typed failure/phase and its
generated/predecessor/ordinal position. A later audit/cleanup failure also retains
`deciding_event_before_failure`. Physical writes or key admissions do not certify
that a predecessor/frontier row was published. Partial writes/staging can exceed
expected bytes after a failure; that field is not a measured disk-usage total.

Fingerprint success preserves TLC's optimistic estimate. When its existing
threshold requires the actual-gap estimate, exceeding the summary bound refuses
completion instead of omitting that field. Exact mode reports no fingerprint
collision estimate, matching the resident counterpart.

Explicit cleanup failure prevents completed safety even after the search found
an earlier outcome. A retained counterexample under `resource-limit` is diagnostic
evidence, not full agreement or a completed certificate. First errors are typed
`DIVE-S0700` storage failures; later cleanup does not replace the original cause.

No key/offset collection stays in RAM, but supplied canonical keys and states,
frontiers, predecessors, candidates, trace reconstruction, graphs, file handles
and OS page cache remain outside this policy. You may separately opt into
`--frontier-memory-bytes` and `--predecessor-memory-bytes`, including zero; neither
this combination nor any one flag is a total-memory budget. Inserts do not fsync
data and have no crash durability or restart/import guarantee.

Only successful explicit `INIT`/`NEXT` binding is admitted for search. Missing
or invalid roles preserve ordinary binder ordering and can emit startup output;
they do not carry a blanket no-effects guarantee. Configured `PROPERTY`
and `SPECIFICATION` roles refuse before startup effects, including safety-only
`SPECIFICATION`; callback-free role decomposition remains pending.
Graph/source capture, temporal checking,
checkpoint/recovery and DFID with this store remain explicit unsupported
combinations. Simulation has no disk-seen route. Pure safety VIEW and symmetry
retain the existing concrete state and independent-witness contracts; their
unproved random/liveness combinations remain refused. This increment changes no
Random/VIEW or trace representation admission and grants no family certification.

See [the storage library](disk-seen-library.md),
[ADR0068](../decisions/0068-fallible-disk-seen-safety.md) and the current
[source-bound study](../../conformance/probes/fallible-seen-engine/README.md).


An invariant that raises an evaluation error on a successor currently returns
`unsupported: disk seen-state invariant-error call-stack replay`. TLC resumes
past that successor during call-stack replay; the disk route has no proved
positioned read/replay authority yet. The original error remains in `notes` and
any concrete word is diagnostic evidence, not a completed certificate. Reported
counters describe only admitted keys and the initial stop; they are not TLC's
completed error-replay counters. Use the resident counterpart for this case.

The minimal retained case starts from `x = 0` or `x = 2`, visits `x = 1` before
an already-seen `x = 2`, and divides by zero in its invariant at `x = 1`.
Pinned TLC and the resident reference report 3 generated, 3 distinct and 1 queued.
The preserved failed disk epoch reported 4 distinct and 2 queued. Current disk
checking explicitly refuses that boundary; it never consults an empty resident
set as an answer. [Source and original raw observations](../../conformance/probes/fallible-seen-engine/invariant-replay-before/).
