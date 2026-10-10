# Search storage and native recovery

Safety BFS has independent opt-in frontier and predecessor budgets:

```sh
dive check docs/user/examples/Counter.tla --compat=tlc --seed 7 --fp 0 --workers 8 --frontier-memory-bytes 0 --output json
dive check docs/user/examples/Counter.tla --compat=tlc --seed 7 --fp 0 --workers 8 --predecessor-memory-bytes 0 --output json
```

Zero forces disk storage from the start. The frontier budget counts encoded
current/next queue bytes; the predecessor budget counts resident parent/ordinal
vector capacity. Neither limits total process memory, visited states, candidate
vectors, temporal graphs or reconstructed paths. Optional directory flags require
a matching budget and an existing parent directory. These files are transient
search storage, not restart archives. Read [frontier storage](../frontier-storage.md)
and [predecessor storage](../predecessor-storage.md) for accounting and cleanup.

Native checkpoints are a separate bounded pure safety feature:

```sh
dive check docs/user/examples/CheckpointCounter.tla --compat=tlc --seed 7 --fp 0 --checkpoint counter.cp --checkpoint-after-levels 2 --output json
dive check docs/user/examples/CheckpointCounter.tla --compat=tlc --recover counter.cp --output json
```

The pure example pauses after two completed levels with three discovered states
and one queued state, then recovers to five distinct and six generated states.
An unused temporal definition can also trigger the conservative purity refusal;
the original `Counter.tla` checkpoint attempt is retained in the example observations.
The pause returns an incomplete resource-stop result; it is not a safety proof.
A committed archive restores saved discoveries, predecessor records and the
ordered frontier instead of replaying Init to recreate them. Actual execution
is inline even when a wider width was requested. Counter reset follows TLC by
default; `--checkpoint-counters retain` preserves uninterrupted counts and is an
explicit mixed setting.

Current archives are trusted-local native files, not Java-interoperable archives
or independently checked model certificates. Source/configuration/profile/model
identity and structural bounds are checked. Pure supported VIEW and safety symmetry
are admitted: recovery recomputes the configured reduced keys while preserving
original concrete discoveries and witnesses. Liveness, runtime effects/names or
checker reads, unsupported deferred values, transient storage budgets and parallel
recovery remain explicit refusals. Filesystem/recovery failures never report success.

See [native checkpoint details](../native-checkpoint.md) for exact limits,
no-overwrite commits, default recovery settings, API requests and evidence.


For a runnable VIEW and symmetry example, see [configured-key recovery](../native-checkpoint.md).
`CpSymView` pauses with 4 generated states, 2 distinct states and one queued state,
then resumes to the same concrete invariant violation as uninterrupted checking.
With `--checkpoint-counters retain`, its final count is 5 generated states; TLC's
default reset reports 3 generated states and 2 initial states instead. Both reach
3 distinct states at depth 3. A wider worker request is recorded, but this native
recovery slice runs inline with one actual worker.

Reduced-key equality is neither an Init/Next proof nor an archive authenticity
check. Use trusted local archives and independently validate returned concrete
witnesses. A valid permutation declaration does not prove that the specification
is symmetric. Effectful configuration aliases are refused before binding output
or archive creation, including public Rust IR; pure aliases remain supported.
The [source and raw study](../../conformance/probes/native-checkpoint-keys/README.md)
separates historical evidence from the current integrated implementation.
