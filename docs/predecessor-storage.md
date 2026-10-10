# Predecessor storage

Safety checking can move its parent/ordinal log to disk while preserving discovery
IDs and concrete counterexample replay:

```sh
dive check Model.tla --compat=tlc --seed 7 --fp 0 \
  --predecessor-memory-bytes 0 --workers 8 --output json
```

Use `--predecessor-directory DIR` to select an existing parent directory. Without
it, dive uses a private subdirectory of the system temporary directory. A directory
alone is rejected before reading the spec. The Rust counterpart is
`CheckRequest.predecessor_memory_bytes` with `predecessor_directory` (and the same
fields on `CheckOptions`). Omit both to keep the memory reference path.

The budget counts committed resident parent/ordinal vector capacity only. It is
separate from the encoded frontier budget. It does not bound the seen set,
frontiers, candidate vectors, temporal graphs, decoded values, reconstructed
paths or allocator workspace. Once spilled, the log retains no growing RAM index.
Each disk read uses a fixed 16-byte record. Reconstructed paths in opt-in mode
have a separately reported 1,000,000-step limit and fallible allocation.

JSON `predecessor_storage` reports capacity before cleanup, peak committed
capacity, complete records and confirmed complete header/record write bytes.
Failed writes may leave additional partial bytes; `write_accounting_complete`
then becomes false. It also records failures, cleanup completion and the search
event reached before a failure prevented certification. Counts and actual worker
metadata remain available. A storage failure produces `resource-limit`, clears
certified counterexamples/collision claims, and never reports success.

The reader checks format/version, exact length, checksum and earlier-parent
ordering. Before a completed result is returned, every record is checked and
owned files are removed. Best-effort Drop cleanup is only a fallback. The files
are transient native storage, not a checkpoint/restart format.

Only ordinary safety BFS admits this setting. Simulation, DFID and bounded
behavior-graph capture name the unsupported combination. No random replay,
namespace history, canonical representative or semantic resource default changes.

## Evidence

`crates/dive-api/tests/predecessor_spill.rs` compares memory with zero, 24 and
256 bytes on successes, violations, deadlocks, mid-level errors, multiple roots,
duplicates, constraints, action properties and symmetry at 1/4/8 workers, with
and without chaos, in both profiles and both stores. Separate independent
symmetry witness controls require valid originals and rejection of corruptions.
`crates/dive-store/src/predecessor.rs` covers spill transitions, truncation, extra
bytes, bad headers/checksums, parent cycles, missing files, failed writes,
unvisited-record corruption and cleanup failure.

The fixed source passed 713/713 workspace tests and the full local gate.
Required release CM1 21/21 and ValidationTestSuite 16/16 agree at 1/4/8 workers;
both also agree when every check is forced to a zero-byte predecessor budget.
The complete 936-run native matrix preserves 312 memory references and 624
strictly equal bounded runs. Binary/source hashes, raw outputs, gate logs and
initial/interrupted-run history live in
`conformance/probes/predecessor-spill/`. These are staged functional controls;
F-031 remains partial and every family certification stays audit-required.
