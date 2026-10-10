# Safety frontier storage

`dive check Spec.tla --frontier-memory-bytes 1048576 --output json` limits each
encoded active safety frontier to 1 MiB before spilling it to owned transient
files. `--frontier-memory-bytes 0` forces disk immediately. Add
`--frontier-directory DIR` to choose an existing parent directory. Omit these
options to use the original in-memory reference queue.
Simulation and DFID explicitly refuse these safety-BFS storage flags before
executing a model; they do not silently ignore the request.
Rust callers must also pair `CheckRequest::frontier_directory` with
`frontier_memory_bytes`. A directory without a budget returns `DIVE-A0100`
before opening the model or configuration.

This works with `--compat=tlc`, the default profile, `--store fp` and
`--store exact`. It preserves ordered search, partial-stop counts and concrete
counterexamples. Disk storage keeps a fixed-size index on disk and does not
retain a state vector or a growing RAM offsets array for the spilled queue.

The JSON `frontier_storage` object reports the per-queue budget, peak combined
resident encoded capacities, retained bytes after cleanup and
`committed_spill_bytes`. The byte total counts completed writes including
headers/indexes. `write_accounting_complete: false` means a filesystem failure
may have left additional partial bytes; it is not a completed-file size claim.
Read, write, corruption and cleanup failures are explicit resource-limit
results, with execution evidence retained. `search_event_before_storage_failure`
retains the deciding search event when a later filesystem failure invalidates it. Transient files are not checkpoints.

The budget applies only to the current and next encoded frontiers. Codec
workspaces are capped separately at 64 MiB per record and 65,536 visited nodes
per value. Decoded values, candidate/winner buffers, visited/exact stores,
predecessor logs and behavior graphs remain resident and outside this budget.
This does not bound total process memory. Frontier spill with temporal graph
capture currently refuses by name. Evaluator-driven deferred values refuse
before encoding, without calling their evaluator; callback-free lazy
representations remain supported.

Reproducible controls:

```sh
cargo test -p dive-store --lib spill::tests
cargo test -p dive-check --test frontier_spill
cargo test -p dive-api --test frontier_spill
cargo test -p dive-value --test codec frontier
```

The safety controls cover positive completion, invariant violation, deadlock,
mid-level evaluation error, duplicate successors, implied-action checks,
constraints and empty initial frontiers. Each compares the memory path against
0/64/1024-byte budgets, both profiles/stores, inline and 1/4/8 workers with chaos.
Symmetry counterexamples also pass independent validation and corrupted
witness controls. Deferred VIEW admission and filesystem failure controls must
refuse rather than execute extra effects or certify an incomplete result.
