# 0107: Run budgets instead of fixed counts

- **Status:** accepted (owner, 2026-10-20, including the half-of-memory
  default). The default time-limit value is provisional until the benchmark
  campaign sets it.
- **Date:** 2026-10-20

## Context

dive's temporal path refuses on 17 fixed counts: properties, formula and
tableau nodes, expansion steps, captured states and edges, atom cells,
product vertices, edges and steps, acceptance bytes, planning terms and
quantifier bindings. They were all set between 2026-10-09 and 2026-10-10, and
none came from measurement; ADR 0098 calls its own value a "policy baseline".
Several of them refuse models that TLC checks in seconds. For example,
EWD998ChanID has 14 states and is refused by the 4096 predicate-binding cap.
dive had no time or memory budget at all. A run that was not refused either
finished, or was killed by the harness's process timeout without a result.

The pinned TLC (tlaplus `94d0c507`) has no liveness count caps. Its only
enforced liveness limit is the disjunctive-normal-form overflow at about 2^31
disjuncts (`LNConj.java:205`, error 2213). `-maxSetSize` (10^6) bounds set
enumeration in general. TLC keeps its behavior graph on disk and is otherwise
bounded by its JVM heap. Running out of memory is `SYSTEM_OUT_OF_MEMORY`
(1001–1003), with exit status 1 during liveness checking and 255 otherwise.
Its only wall-clock stop is the developer property `-Dtlc2.TLC.stopAfter`
(`AbstractChecker.java:648`), which the source describes as a "developer only
feature… not a regular TLC parameter".

PR #334 proposed lifting the counts under `--compat=tlc` (ADR 0106, proposed,
unmerged). Lifting them showed that they were hiding algorithmic gaps rather
than real blowups:

| Model | dive with counts lifted | TLC |
|---|---|---|
| MCParReach | about 1,530 s | 30 s |
| MCQuicksort | about 600 s | 5 s |
| ElevatorLivenessMedium (4,122 states) | over 77 GB in product construction | 39 s |

Unbounded runs with no other stop turn a refusal into a runaway job. The
owner ruled out both outcomes: refusals that leave dive with less capacity
than TLC, and 30-minute runaway jobs in any regular loop.

## Decision

1. **Goal: no arbitrary count limits.** dive's capacity must be at least
   TLC's. A fixed count is removed once the algorithmic gap behind it is
   closed, one count per PR, with measured evidence. The only count that stays
   is TLC's own normal-form overflow, which TLC also refuses. This ADR changes
   no count; ADR 0106's profile split is superseded by this policy.
2. **Runaway guards are budgets, not counts.** They apply in both profiles.
   The clock starts when the check starts, after the specification and
   configuration are loaded. The budget is polled during safety search (each
   initial state and each expanded state), graph capture and temporal
   analysis. Binding, `ASSUME` evaluation and any single long evaluation (an
   `Init` whose domain must be built first, one huge successor set) are not
   interrupted; the stop comes at the next poll point.
   - **`--max-memory BYTES|auto|none`** is a ceiling on the memory of the whole
     process: resident plus swapped pages on Linux, resident pages on macOS.
     `auto`, the default in both profiles (owner, 2026-10-20), is half of
     physical memory. TLC's default footprint is its JVM heap (a quarter of
     physical memory) plus off-heap fingerprint storage (`FPSetConfiguration`,
     default ratio 0.25) and its disk queue. dive keeps all of that resident,
     so half keeps dive's capacity at least TLC's while still stopping a
     runaway run before it exhausts the machine. Like the host-access grants
     of D-017, this is a host-resource exception to `--compat=tlc`: the
     ceiling applies in compat mode too. Its TLC counterpart is `-Xmx`, which
     `dive tlc` accepts, so an explicit ceiling is not a compatibility
     override. Where the platform cannot measure the process, no ceiling is
     claimed (`max_memory_source: "unavailable"`).
   - **`--time-limit DURATION|none`** limits wall-clock time. Its TLC setting
     is `none`, which `--compat=tlc` selects. The dive profile ships a default
     (`dive_check::budget::DEFAULT_TIME_LIMIT`, provisionally 30 minutes) that
     is a single release-time constant, chosen from the benchmark campaign
     before release. This is D-026. TLC has no time limit, so an explicit one
     under `--compat=tlc` labels the profile `tlc+overrides`.
3. **Exhaustion is a structured stop, never a verdict.** A stopped run reports
   `verdict: "stopped"` and a `stopped` block with the resource, the limit,
   the observed value, the time from arming to the stop (`elapsed_ms`, at or
   after the trip), the phase (`safety-search`, `graph-capture` or
   `temporal-analysis`) and a continuation hint. Statistics at the stop are
   reported as progress, with `reported_by_tlc: false`. Only the budget's
   own limit becomes a stop; every other failure keeps its class. Its exit
   status is 5 under both exit conventions,
   next to 3 for refusals and 4 for internal errors. TLC has no counterpart:
   it dies with `SYSTEM_OUT_OF_MEMORY`, and its developer-only time stop is
   not a TLC-compatible observable. A stop is not relabelled as
   `unsupported`.
4. **Zero overhead.** A watchdog thread checks elapsed time every 50 ms and
   memory every 100 ms on Linux (`/proc/self/status`) or every 500 ms on
   macOS. ADR 0109 supersedes the original delayed-only `ps` sampler with
   safe native initial/terminal observations plus these periodic samples.
   Short runs are observed, unavailable measurements fail closed, and physical
   RAM is still read once per process. Stopping the watchdog does not wait for
   a sample in flight. When either limit is
   exceeded it sets a flag. Engines read the flag at points they already
   pass:
   - each initial state and each expanded state in the inline engine;
   - each claimed block in the parallel engine. An event found before the
     budget ran out is still published when every block that starts at or
     before it was expanded;
   - each state labelled, each tableau expansion and each product inspection
     in temporal analysis.

   Each read is one relaxed atomic load. The acceptance bar is at most 1%
   more instructions (cachegrind) on the ADR 0104 models, with identical
   counts. A single evaluation, such as one huge `Init`, is not interrupted;
   the harness's kill backstop covers it.
5. **Early termination only on proof.** A future projected-size check may
   stop a run early only when a proven lower bound exceeds the memory
   ceiling, or when a passive projection from counters the checker already
   keeps crosses the time budget. In both cases it reports `stopped` with the
   projection. It adds no exploration pass and no per-state work. It is not
   part of this change.
6. **Development loops use explicit budgets derived from TLC.** With
   `--tlc-budgets`, a model with a golden gets a harness budget of
   `max(60 s, 10 × the golden's TLC time)`, hard-capped by `--budget-cap` (default
   900 s). The harness passes the budget as
   `--time-limit` in both profiles (an explicit host budget, labelled
   `tlc+overrides` under compat) and `--xmx` as `--max-memory`, and kills the
   process only 30 s later. A run over either budget is a timeout finding (a
   performance issue), never agreement. Long TLC reference times do not bypass
   this development cap; an explicitly requested long campaign can select
   a larger `--budget-cap` or override it with `--timeout`. Without
   `--tlc-budgets`, the harness
   keeps its historical timeouts and runs `dive check` with
   `--time-limit none --max-memory none`. The harness and the gate never
   depend on the default time limit; library tests that keep the default
   finish far below it.

## Consequences

- An agent or a loop gets a bounded, machine-readable answer ("stopped: time,
  safety-search, N distinct states") instead of a hung process or a kill.
- Library callers of `dive_check::check`, `dive_liveness::analyze` or
  `dive_api` get the same budget. `dive_api` resolves the profile defaults.
  `CheckOptions::budget` is unlimited unless a caller sets it, and a caller
  that runs several stages arms the budget once
  (`dive_check::budget::arm`).
- The JSON change is additive within schema versions 1 and 2: a new verdict
  value `stopped`, a `stopped` block, and `options.budget`. Consumers that
  pin the verdict vocabulary, such as Oyster's pilot, see `stopped` only when
  a budget is exhausted.
- The gate, smoke/cm1 scripts and nightly model checks select
  `--tlc-budgets --budget-cap 900` explicitly (owner, 2026-10-20). Each model
  uses its recorded TLC time, independent of the release-time default. A
  budget-only compatibility override preserves semantic comparison after the
  harness verifies the requested limit and TLC settings; other overrides
  remain ineligible for strict temporal transport.
- Not covered yet: `dive dfid` and `dive simulate`, the `dive tlc` adapter's
  `-Dtlc2.TLC.stopAfter` property, environment-variable or configuration
  spellings of these options, and `dive-bench`, which still runs `dive check`
  with the defaults (it also drives binaries from before this change).
- Each fixed count still in force is removed by its own follow-up (lane L,
  R2 onward): predicate bindings first, then TLC-style tableau decomposition,
  then product and atom-cell representation, then a disk-backed behavior
  graph.

### Watchdog startup failure

A finite budget that cannot start its watchdog is an explicit runtime failure
(`DIVE-R0001`), never an unlimited run or a model verdict. Arming is fallible in
the library and every existing budget-arming call site propagates that failure. The CLI aborts
with status 4 before checking; an explicit request disabling both budgets needs
no watchdog. A deterministic injected-spawn-failure control covers this path
without exhausting threads on the shared host.

### Explicit discovery budgets

`--max-states N|none` and `--max-depth N|none` are opt-in search budgets in
both profiles; their defaults and TLC-equivalent settings are unset. Initial
states have BFS depth one, and state identities use the existing in-model
identity policy. The first new identity over either ceiling stops before
admission, while exact-bound exhaustion remains possible. A checkpoint whose
whole imported graph exceeds a ceiling stops before importing its keys.

Both resources use the existing stopped outcome, with names `states` and
`depth`, units `states` and `levels`, actual admitted progress and exit 5.
They never become unsupported features or successful checks. Parallel
generation keeps its pool; explicit-limit publication uses ordered candidates
so no key beyond the budget prefix is inserted. The unset resolver remains
unchanged, and the normal path does no extra membership lookup or counter
sampling. Cold limit metadata is allocated only when a limit is requested.
The instruction-overhead acceptance bar remains at most 1% in both profiles.
