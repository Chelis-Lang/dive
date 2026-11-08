# Distributed safety workers

`dive check` can send the evaluation of each source state to separate
`dive worker` processes, on this machine or on other trusted machines. The
checking process (the *coordinator*) keeps the search: it computes the initial
states, commits successors in breadth-first order, owns the seen set, the counts
and the first error, and replays any counterexample locally. Workers only
compute the successors of one source state at a time.

The result is the result of an ordinary one-worker local check. Verdict, counts,
depth, property and counterexample are identical, in both profiles and both
stores, and every counterexample is validated independently as usual. A run that
cannot finish its distributed work never reports success: it stops with a named
error and a non-zero exit code.

Distribution is opt-in. Without `--distributed-worker`, checking is local.

## When to use it

Use workers to move expensive per-state evaluation off the coordinator machine.
Distribution does not make checking faster today: the coordinator sends one
source state per connection and at most one per worker at a time, so a model
with cheap states runs much slower than it does locally. For most models a local
`--workers N` check is the faster choice.

## Supported checks

Workers run pure safety checking on Core: invariants, action properties
(`[][A]_v` in `PROPERTY`), deadlock, `CONSTRAINT` and `ACTION_CONSTRAINT`,
evaluation errors, `--max-states`, `--max-depth`, `--time-limit` and
`--max-memory`, `--compat=tlc`, `--store fp|exact`, and frontier and predecessor
spill on the coordinator.

These refuse by name with exit 3, before any worker is contacted:

| Model or option | Message names |
| --- | --- |
| Temporal properties (liveness, fairness) | `distributed safety with temporal properties is not implemented` |
| `SYMMETRY` | `distributed safety with SYMMETRY is not implemented` |
| `VIEW` | `distributed safety with VIEW is not implemented` |
| `Print`, `PrintT`, `RandomElement`, `TLCGet`, `TLCSet` | `distributed safety with runtime effects (...) is not implemented` |
| No `--seed` or no `--fp` | `distributed safety without an explicit --seed` (or `--fp`) |
| `--sequential`, `--checkpoint`/`--recover`, `--core-sample`, `--allow-io` grants, disk seen, candidate spill, Store reclamation, coverage, `dive debug` | `distributed safety with <option> is not implemented` (for `dive debug`: `compiled debugger with distributed safety checking is not implemented`) |

A `dive worker` refuses the same models with `worker preparation with ... is
not implemented` and exits 3 before it listens. A worker whose own start-up
ends in a verdict, such as a false or failing `ASSUME`, exits 3 with `worker
preparation stopped with <verdict> before serving units` and the message
`dive check` would print. Workers do not take run budgets
(`--max-states`, `--time-limit` and so on) or `--workers N`; give those to the
coordinator.

## Set up a run

Every participant must use:

- the same dive executable build (its SHA-256 is part of the session identity);
- the same module files and the same configuration file bytes (a comment
  counts);
- the same `--seed`, `--fp`, `--compat`, `--store`, `-deadlock`,
  `--action-composition`, `--symmetry-liveness` and `DIVE_CORE_OPT` settings;
- the same 32-byte key file.

Each process reads its own files. A worker never receives source, programs,
paths or host grants over the network, and work messages cannot stop it.

### 1. Make a key

The key file must contain exactly 32 random bytes. Only the path appears on the
command line; the bytes never appear in arguments, reports or logs. Copy the file
to every participant over a channel you trust and keep it readable only by the
user that runs dive.

```sh
cargo build -p dive-cli --locked
umask 077
head -c 32 /dev/urandom > worker.key
```

### 2. Start the workers

Start at least two workers, each in its own terminal or service, with the same
model options as the coordinator:

```sh
target/debug/dive worker docs/user/examples/Counter.tla --seed 0 --fp 0 \
  --worker-key worker.key --listen 127.0.0.1:7801 --output json
```

```sh
target/debug/dive worker docs/user/examples/Counter.tla --seed 0 --fp 0 \
  --worker-key worker.key --listen 127.0.0.1:7802 --output json
```

Each worker loads and compiles the model, then prints one line when it is ready:

```json
{"address":"127.0.0.1:7801","command":"worker","model_proved":false,"process_id":4242,"status":"ready"}
```

`--listen 127.0.0.1:0` (the default) picks a free port and prints it.
A port already in use is a usage error (exit 2). A worker serves until it is
killed; `--worker-max-units N` makes it exit after N completed units and print a
`"status":"stopped"` line with its own work facts. Neither line proves anything
about the model.

### 3. Run the coordinator

```sh
target/debug/dive check docs/user/examples/Counter.tla --seed 0 --fp 0 \
  --distributed-worker 127.0.0.1:7801 --distributed-worker 127.0.0.1:7802 \
  --worker-key worker.key --output json
```

The result is success with 5 distinct states, 6 generated states and depth 5,
the same as `dive check docs/user/examples/Counter.tla --seed 0 --fp 0`. The
coordinator connects to every endpoint and authenticates it before the search
starts. Exit codes keep their ordinary meanings.

Add `--compat=tlc` or `--store exact` to all three commands for those modes.
Stop the workers yourself when the check is done.

## Options

| Option | Role | Meaning |
| --- | --- | --- |
| `--distributed-worker IP:port` | `check` | A worker endpoint. Repeat it: 2 to 64 distinct endpoints, which must reach at least two different worker processes. |
| `--worker-key FILE` | both | Path to the 32-byte shared key. Required in both roles. |
| `--worker-timeout-ms N` | both | Deadline in milliseconds, 1 to 60000, default 5000. On the coordinator it bounds each connection, handshake, message and unit; on a worker it bounds each unit it evaluates. |
| `--distributed-retries N` | `check` | How many more times a source unit is sent after a lost connection or timeout, 0 to 8, default 2. Each retry goes to the next endpoint in the list. |
| `--listen IP:port` | `worker` | Endpoint to listen on, default `127.0.0.1:0`. |
| `--worker-max-units N` | `worker` | Exit after N completed units. |

Out-of-range values, fewer than two endpoints, a repeated endpoint, and the
key/timeout/retry options without `--distributed-worker` are usage errors
(exit 2), reported before the model is loaded.

## What happens when something fails

The coordinator only commits a source state's successors after that unit's full
result has arrived and authenticated. Units from a dropped connection never
contribute successors, so a retry can never count a state twice. When a unit
cannot be completed, the run stops with verdict `resource-limit` and exit 3. A
stopped run reports the progress it had committed, the same as a local run that
reaches a budget, but no verdict, property or counterexample.

| Situation | Outcome |
| --- | --- |
| A worker dies or exits during the run | The lost unit is retried on the next endpoint; the result equals the local result. If the retries run out: `resource-limit`, exit 3, `DIVE-S0502: distributed I/O: ...`. |
| A worker restarts on the same endpoint | Later units can use it, if its build, model and key match. |
| An endpoint has no listener | `resource-limit`, exit 3, `Connection refused`. |
| An endpoint accepts but never answers, or answers too slowly | After `--worker-timeout-ms`: `... no reply within the --worker-timeout-ms deadline`. During the search the unit is retried first. |
| A unit takes longer than the worker's own `--worker-timeout-ms` | The worker reports it: `worker refused incomplete unit: the unit did not finish within the worker's --worker-timeout-ms of N ms`. Not retried. Raise the worker's timeout. |
| Coordinator and worker keys differ, or the worker is stalled | `worker ... closed or did not answer the authenticated handshake (check that every role reads the same --worker-key file and that the worker is running and not stalled ...)`. A different build is reported as an identity mismatch instead (below). |
| Key file missing or not exactly 32 bytes | The coordinator stops with `worker key file PATH: ...`; a worker exits 3 before it listens. |
| `--distributed-worker` without `--worker-key` | Usage error, exit 2. |
| Different build, source, configuration, seed, fingerprint, profile, store or `-deadlock` | `worker source/config/executable/policy/startup identity mismatch`, before the search. |
| Two endpoints reach the same worker process | `two independently initialized workers required`. |
| Corrupted, replayed, stale or oversized messages | Rejected by authentication or admission checks; the unit fails and the run stops. |
| The coordinator is killed | The run is lost. There is no distributed checkpoint; start again. |

Only connection loss and timeouts are retried. Authentication, identity and
admission failures stop the run at once.

## Security

Traffic is authenticated with HMAC-SHA256 over fresh, identity-bound sessions.
It is **not encrypted**. Use a trusted private network, or an encrypted tunnel
such as SSH port forwarding or WireGuard, when the model or its states are
confidential. Anyone holding the key is trusted to compute successors correctly;
process IDs and worker IDs in the report are not remote attestation. dive does
not discover workers, distribute keys, start remote processes or admit untrusted
workers.

## Reports

`core.metrics.distributed` in the JSON report records what happened:
`configured_workers`, the authenticated `workers` (ID, process ID, units sent to
and results received from each), `assigned_units`, `completed_units`,
`retries`, `epoch` (largest completed level), `membership_queries`,
`duplicate_frames`, `failed_units` and `failures`. Logical counts include only
committed states. A returned result that was never committed, such as work after
the first error, counts in `result_units` but not in the model counts. These are
work facts, not a speed measurement or a proof of completion.

## Limits

Each message is at most 64 MiB, each value graph 32 MiB with 128 levels, each
membership key 1 MiB, and each source state may have at most 16,384 successors;
crossing a limit stops the run by name. String names are fixed before work
begins, so evaluation that needs a new name refuses explicitly. These limits
apply only to distributed checking. `--max-memory` measures the coordinator
process only, not the workers.

## Rust callers

An embedding program can use `prepare_core_worker` and `CoreWorker::serve` for
the worker role and `check_file_engine` with `DistributedOptions` for the
coordinator. Use the same executable for every role, because the executable
digest is part of the session identity.

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

The caller supplies the worker's listener and stop callback; `serve` returns
work facts, never a model verdict. Through the API, endpoint policy errors are
reported as a `resource-limit` result rather than a usage error.

## Tests

`crates/dive-cli/tests/distributed.rs` runs real worker processes: two and four
workers against local results for success, invariant and action-property
violations, deadlock, constraints and evaluation errors, with validated
counterexamples and repeated runs; worker exit with and without retries;
exhausted retries; endpoints that refuse, never answer or stall; worker unit
deadlines; wrong, short and missing keys; another build; endpoint and listener
policy; and the refused models and options. `crates/dive-api/tests/distributed.rs`
covers identity mismatches, restarts and the Rust API, and
`crates/dive-explore/src/distributed/fault_tests.rs` covers corrupted, stale,
duplicate and oversized frames. The design is recorded in
[ADR 0151](../decisions/0151-authenticated-native-safety-workers.md).
