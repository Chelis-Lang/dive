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
identity and structural bounds are checked. The current pure slice refuses
VIEW, symmetry, liveness, effects, unsupported deferred values and transient
storage budgets; later key-interaction work is pending until its guide and
controls land. Filesystem/recovery failures never report success.

See [native checkpoint details](../native-checkpoint.md) for exact limits,
no-overwrite commits, default recovery settings, API requests and evidence.
