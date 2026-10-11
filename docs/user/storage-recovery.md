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
dive check docs/user/examples/CheckpointCounter.tla --compat=tlc --seed 7 --fp 0 --workers 4 --checkpoint counter.cp --checkpoint-after-levels 2 --output json
dive check docs/user/examples/CheckpointCounter.tla --compat=tlc --workers 8 --recover counter.cp --output json
```

The pure example pauses after two completed levels with three discovered states
and one queued state, then recovers to five distinct and six generated states.
An unused temporal definition can also trigger the conservative purity refusal;
the original `Counter.tla` checkpoint attempt is retained in the example observations.
The pause returns an incomplete resource-stop result; it is not a safety proof.
A committed archive restores saved discoveries, predecessor records and the
ordered frontier instead of replaying Init to recreate them. Pure admitted models run the level engine at the requested worker width;
`--sequential` selects inline execution. A boundary-zero pause stops after Init
before starting the pool and honestly reports one actual worker. Counter reset follows TLC by
default; `--checkpoint-counters retain` preserves uninterrupted counts and is an
explicit mixed setting.

Current archives are trusted-local native files, not Java-interoperable archives
or independently checked model certificates. Source/configuration/profile/model
identity and structural bounds are checked. Pure supported VIEW and safety symmetry
are admitted: recovery recomputes the configured reduced keys while preserving
original concrete discoveries and witnesses. Liveness, runtime effects or
checker reads and unsupported deferred values remain explicit refusals. Frontier and
predecessor budgets can be used independently or together for pause/recovery;
recovery may change budgets or worker width. Native archive work remains outside
both transient budgets. The [runnable spill recovery example](../native-checkpoint.md#recovery-with-transient-disk-storage)
includes CLI/Rust requests and cleanup/failure evidence. Filesystem/recovery failures never report success.

See [native checkpoint details](../native-checkpoint.md) for exact limits,
no-overwrite commits, default recovery settings, API requests and evidence.


For a runnable VIEW and symmetry example, see [configured-key recovery](../native-checkpoint.md).
`CpSymView` pauses with 4 generated states, 2 distinct states and one queued state,
then resumes to the same concrete invariant violation as uninterrupted checking.
With `--checkpoint-counters retain`, its final count is 5 generated states; TLC's
default reset reports 3 generated states and 2 initial states instead. Both reach
3 distinct states at depth 3. Positive completed-level pauses and continued
recovery report the requested pool width. Pause at four workers and recover at
eight if useful; the deterministic result and saved starting level are unchanged.
Pool width is execution metadata, not a speedup guarantee.

Reduced-key equality is neither an Init/Next proof nor an archive authenticity
check. Use trusted local archives and independently validate returned concrete
witnesses. A valid permutation declaration does not prove that the specification
is symmetric. Effectful configuration aliases are refused before binding output
or archive creation, including public Rust IR; pure aliases remain supported.
The [source and raw study](../../conformance/probes/native-checkpoint-keys/README.md)
separates historical evidence from the current integrated implementation.

Current source-bound spill/recovery and executable CLI/Rust example evidence is
in the [combined recovery receipt](../../conformance/probes/native-checkpoint-spill/current-shipping/delivery.json).
It proves the listed finite-safety selection, not total-memory bounds, archive
authenticity or complete TLC recovery interoperability.

The [earlier main integrated verification](../native-checkpoint.md#earlier-parallel-integrated-verification)
records worker-width, counter-prefix and independently checked concrete-witness
controls on the shipped feature combination. These controls certify their
declared fields; they do not certify temporal recovery or a complete archive
feature family.


Pure string constructors also support native pause/recovery. The
[string example and exact worker/counter contract](../native-checkpoint.md#pure-strings-and-recovery-encounter-order)
include a runnable `ToString` case with both zero-byte transient budgets. TLC
encounter order uses an explicit one-worker inline fallback; canonical default
order may use the pool. The saved ordered name table restores execution context,
not independent proof: TLC witness validation still explicitly refuses runtime
name history. Keep that separate from the archive checksum and recovery result.

The [current pure-name publication checks](../native-checkpoint.md#current-pure-name-recovery-publication)
rerun pause/recovery, counter choices, spill cleanup and the executable Rust
example on the combined runtime. They preserve the TLC runtime-name validator's
explicit refusal and do not turn a native archive into an independent proof.

Current native snapshots use structural format 2 (`DIVECP02`); earlier
value-bearing archives refuse rather than guessing lost tuple/function identity.
Recreate them with the current executable. Transient frontier data/index likewise
use version 2; predecessor-only ID/ordinal files keep their existing format.
The [migration example](../native-checkpoint.md#structural-format-migration)
combines zero-byte queue/log budgets with opt-in stored trace transport and
independent witness checking. This migration changes no mathematical store key
or fingerprint and grants no additional archive or total-memory proof.
## Additive disk membership library

[`dive_store::seen::DiskSeen`](disk-seen-library.md) retains full fingerprint or
canonical exact keys on disk with fallible operations and explicit owned cleanup.
This library is not yet a `dive check` option. Ordinary seen sets still remain
resident; frontier/predecessor flags do not imply a seen-set or total RAM budget.

