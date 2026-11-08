# Native distributed safety

Explicit Core safety checking can send real source-state evaluation to two or
more independently initialized trusted worker processes. The coordinator retains
ordered BFS commit, counts, IDs and first errors. Both value profiles and fp/exact
membership are admitted. Distribution is opt-in: omit all distributed flags, or
leave `CheckRequest.distributed = None`, for the TLC-equivalent local setting.
This is a partial family; it does not provide distributed temporal or durable
coordinator recovery.

## Run a model

Use the same executable build, model/module sources, configuration bytes,
seed/fingerprint, store, semantic options and `DIVE_CORE_OPT` switches everywhere.
Each process loads its own files before listening or connecting. Work cannot
request another executable, load a module or grant spec host access. Provision a
shared secret file containing exactly 32 random bytes on each participant, with
access restricted to trusted participants. Only the file path appears in flags;
key bytes are never command arguments or result/debug fields.

From the repository root, these commands check the existing finite Counter:

```sh
cargo build -p dive-cli --locked
umask 077
python3 -c 'import secrets; from pathlib import Path; Path("worker.key").write_bytes(secrets.token_bytes(32))'
```

Start two workers in separate terminals:

```sh
target/debug/dive worker docs/user/examples/Counter.tla --seed 0 --fp 0 \
  --worker-key worker.key --listen 127.0.0.1:7801 --output json
```

```sh
target/debug/dive worker docs/user/examples/Counter.tla --seed 0 --fp 0 \
  --worker-key worker.key --listen 127.0.0.1:7802 --output json
```

Run the coordinator:

```sh
target/debug/dive check docs/user/examples/Counter.tla --seed 0 --fp 0 \
  --distributed-worker 127.0.0.1:7801 --distributed-worker 127.0.0.1:7802 \
  --worker-key worker.key --output json
```

The result is success with 5 distinct states, 6 generated states and depth 5.
Every source unit is actually evaluated remotely. Init and selected witness/error
replay use the existing local engines. Add `--compat=tlc` to all three commands
for TLC value semantics, or `--store exact` to all three for exact membership.
Ordinary checking process statuses retain their documented meanings. A worker's
ready/stopped report has `model_proved: false` and never establishes safety.
Stop workers locally after the check; work messages have no shutdown grant.
`--worker-max-units N` provides an optional local finite lifetime, counting
completed units, not startup pings. A run may stop before using that many units.

TCP traffic is authenticated with HMAC-SHA256 and fresh identity-bound sessions.
It is **not encrypted**. Use a trusted private network or an independently
provided encrypted tunnel when confidentiality is required. All key holders
are trusted to execute the bound model correctly; UUID/PID facts are not remote
attestation. The coordinator requires 2..64 distinct explicit IP:port endpoints
and at least two authenticated worker instances. Discovery, key distribution,
remote shell/process management and public untrusted worker admission are absent.

## Retry, limits and result facts

`--worker-timeout-ms N` defaults to 5000 on both roles and admits 1..60000.
It bounds network operations and active units. `--distributed-retries N` defaults
to 2 and admits 0..8. After I/O loss the coordinator retries the entire uncommitted
source on the configured endpoint ring with a new attempt/session. Results from
dropped attempts never contribute logical successors. A restarted worker at the
same endpoint can continue work if the model/build/key still match. Coordinator
loss does not produce a resumable distributed checkpoint.

Authenticated duplicate frames are bounded and suppressed. Stale session/epoch,
identity, integrity, malformed and oversize results refuse; exhausted retries
produce a resource stop. Cancellation may wait for the current bounded socket
operation. A stopped or incomplete prefix never certifies Holds, even if all
received states satisfied the invariants. State/depth budgets are coordinator
admission limits and preserve ordinary stopped-result semantics.

`core.metrics.distributed` reports configured endpoints, authenticated worker
instances/PIDs and their sent work/returned RESULT-frame counts, assigned source
units, admitted complete results, retries, largest
admitted epoch, membership queries, duplicate frames and explicit failures.
Assigned/completed units exclude startup pings; retries are counted separately.
Per-instance `sent_units` includes sent retries; `result_units` counts complete
authenticated RESULT frames before structural admission, so it is not proof.
Returned RESULT frames can include speculative work after the ultimately selected
first model error; owned import and admission occur only at source-ordered
consumption. `failed_units` counts units left incomplete after retries, including
unused later work. `failures` names required failures that prevent certification;
an unused later failure cannot replace an earlier selected model error. Logical
generated/distinct counts include ordered commit only.
After a restart, the contacted-worker list can exceed simultaneous endpoint width.
These are work facts, not speedup measurements or a completion certificate.
They are coordinator observations; workers return their own session facts from
`serve` or a stopped worker report.

Private protocol limits are 64 MiB per frame, 32 MiB per owned DAG or fixed
compiled startup, 128 DAG levels, 1 MiB per canonical membership key and 16,384
emissions per source. Startup names use the existing 65,536-name image admission.
Names are sealed before work in both profiles, so evaluation requiring a new name
refuses explicitly. These limits apply only to distributed admission, never
silently to ordinary local checking.

Coordinator resident Seen, accepted Store/history, imported speculative batch
values, ordinary frontier/predecessor owners, startup/plan roots, Arena/memo/index
storage, codec/JSON/key scratch, socket buffers and allocator overhead are outside
these protocol limits. Workers release their per-unit Store and evaluator roots
after each result; their fixed compiled startup remains live. `--max-memory`
observes the coordinator process, not aggregate worker RSS. No total RSS or
performance claim follows from distribution.

The initial route refuses temporal properties/capture, host/runtime effects,
VIEW/symmetry, sampling, coverage/debugging, sequential/chaos execution, disk Seen,
candidate payload spill, Store reclamation/remapping and checkpoint/pause/recovery.
Both coordinator checking and `prepare_core_worker` reject Store reclamation and
TLC logical coverage before loading source/configuration or key files. Existing
frontier/predecessor spill can be enabled on the coordinator. Pure source/config
admission remains conservative. Local native recovery and temporal paging have
their separate contracts in [storage and recovery](storage-recovery.md).

## Rust callers

An embedding executable can use `prepare_core_worker` plus `CoreWorker::serve`
for its worker role, and `check_file_engine` with `DistributedOptions` for its
coordinator role. Use the same executable for every role because the executable
digest is part of session identity. Each process prepares independently; received
work does not call a user-supplied evaluator or execute a received program.

```rust,no_run
use dive_api::{CheckRequest, DistributedOptions, Engine, check_file_engine};
let request = CheckRequest {
    file: "docs/user/examples/Counter.tla".into(), seed: Some(0), fp: Some(0),
    distributed: Some(DistributedOptions::new(
        vec!["127.0.0.1:7801".parse()?, "127.0.0.1:7802".parse()?],
        "worker.key".into(),
    )),
    ..CheckRequest::default()
};
let report = check_file_engine(&request, Engine::Core)?;
println!("{}", report.to_json());
# Ok::<(), Box<dyn std::error::Error>>(())
```

Worker preparation and serving use the evaluator's recommended stack. The
caller supplies the listener and stop callback; `serve` returns process/work
facts, never a model verdict. Worker preparation admits source/safety options
and the separately supplied unit deadline; explicit search/time/memory budgets
or multiple local workers refuse. Global run budgets belong to the coordinator.
Public API rustdoc includes the corresponding
worker role. Focused process tests compare actual local/distributed results,
receiver/alias history and independently valid/mutated words in both profiles
and stores; fault controls cover partial results, restart, duplicate/stale frames,
identity/key/corruption and private admission. They do not establish broad corpus,
performance, full distributed recovery or release qualification.
