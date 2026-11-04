# dive

**D.I.V.E. (Deterministic Invariant Verification Engine)** is a ground-up Rust implementation of the TLA+ toolchain: parsing and semantic analysis equivalent to SANY, explicit-state model checking equivalent to TLC, simulation, trace validation, PlusCal, a library API and a language server. It has no JVM dependency.

The standard is an **unimpeachably sound port**. Every result is verified continuously against the canonical Java tools. Where `dive` cannot match TLC exactly, it refuses with an explicit `unsupported` result instead of approximating.

> **Status: early development.** Safety checking, bounded temporal checking,
> finite simulation/generation/DFID, independent trace validation and bounded
> PlusCal translation are implemented. Reference remains the default engine.
> The optional Core engine adds compiled execution, exact and disk membership,
> spill and owned safety checkpoint/recovery. Features outside the proved scope
> return named refusals. The family inventory is **8 implemented / 30
> partial / 0 missing**; M1/M2, broad performance and RC acceptance remain open.
> See [current status](docs/developer/core/m1-status.md), the
> [roadmap](docs/ROADMAP.md) and the
> [Phase 7 record](conformance/reports/core/2026-11-03-phase7-checkpoint.json).

Compiled safety simulation and DFID are available with `--engine core` in both
profiles, using one actual worker. `--engine auto` chooses a whole backend before
execution and never retries a reached native refusal. Both backends share
resource controls and independent trace validation. Simulation samples behaviors;
DFID proves safety only with a completion certificate. Configured simulation VIEW
refuses before execution. See [simulation](docs/user/simulation.md) and
[DFID](docs/user/dfid.md).

Core safety BFS supports exact membership, owned candidate payload spill,
frontier/predecessor spill and opt-in disk membership. Opt-in completed-level
`--store-compact-bytes` reclamation releases expanded Core value payloads while
preserving live roots; its byte setting is a trigger, not a hard memory cap.
Completed-level checkpoints preserve owned representations,
construction history and the ordered frontier for identity-checked recovery in a
fresh process. Auto pauses on Reference and recovers with the encoded backend;
disk membership uses one actual worker. Component budgets and archive bounds do
not bound total process memory. Temporal recovery and further spill remain open.
See [storage and recovery](docs/user/storage-recovery.md).

Explicit authenticated Core pure-safety distribution runs real independently
initialized trusted worker processes, with coordinator-owned ordered commit and
complete-unit retry after I/O loss. Both CLI and Rust workflows are available;
broader combinations and durable distributed recovery remain unfinished. The
transport authenticates traffic without encrypting it. See
[distributed safety](docs/user/distributed-safety.md).

Constructive generation is available through `simulate --generate` and the finite
TLC `-generate num=N` transport. Retained samples require independent Init/Next,
constraint and safety-property proof. Sampling proves neither exhaustive safety
nor liveness. Reference simulation also reconstructs bounded finite random-source
relations across startup, Init and Next. General randomization and broader
effects remain open.
The `dive tlc` and installed `tlc` adapters accept common checking flags, finite
simulation/generation options and module/configuration basenames. See the
[checking guide](docs/user/checking.md#tlc-checking-entrypoints) and
[simulation guide](docs/user/simulation.md).

Native PlusCal translation supports the measured debug-header, original-AST,
procedure/macro, termination, wrapping and label-reporting subsets. Independent
pinned-translator/TLC versus native-translation/dive model comparison uses the
Reference checker. Missing configurations, refusals and timeouts remain explicit;
explicit module/cfg/backup publication and AST-only file writes are available.
Complete translation and mapping/specification-backed workflows remain unfinished. See the
[translator guide](docs/pluscal-translation.md).

Opt-in `check --source-coverage` reports native producer expression and generator
work at original source locations through one Reference worker. These counters
are separate from TLC logical costs and model proof. Final `--coverage 0` and
TLC `-coverage 0` additionally report admitted Reference logical action costs,
including multiple scalar variables, helper reuse and lazy quantifier demand.
Complete coverage, Core source maps and debugging remain unfinished. See
[checking and coverage](docs/user/checking.md#tlc-logical-coverage).

Phase 7's eight implementation PRs are merged at `20ee7f75`, tree `b6625ae5`.
The November 4 replacement passes the full gate (3,246 tests and 201 doctests);
its source-bound archive is downloaded and verified member by member before
worker termination. The retained census has unchanged outcomes, and one
previous fallback agreement now executes on Core. The first attempt remains
rejected for evidence preservation after an archive failure and early worker
termination ([#763](https://github.com/Chelis-Lang/dive/issues/763)); the
replacement is a separately dated run and does not reconstruct that lost bundle.
Explicit category grants enable bounded startup environment/text inputs and
offline replay through Reference; runtime reads, writes, exec and broader
backend combinations remain refused. See [host inputs](docs/user/modules.md).
The comparison-summary prototype remains unadopted after missing its performance
screen; no code, flag, default change or rescue ships. See
[ADR 0147](docs/decisions/0147-demanded-comparison-summaries.md).
Remaining mandatory v1 work includes broader language/modes/effects, real
coverage/Core source maps and full DAP, accepted Store/Arena and temporal
paging/recovery, broader distributed safety/recovery, full PlusCal/public APIs and
Linux/deep/performance/RC acceptance before the macOS final mile.

## Documentation

Start with the [user manual](docs/user/README.md) and its
[runnable examples](docs/user/examples/README.md). The
[developer documentation map](docs/developer/README.md) connects design, known
issues, provenance and conformance evidence.

- [`docs/CHARTER.md`](docs/CHARTER.md): mission, scope, soundness contract and architecture.
- [`AGENTS.md`](AGENTS.md): how to work in this repo, for humans and agents.
- [`docs/ROADMAP.md`](docs/ROADMAP.md): milestones.
- [`docs/scoreboard.md`](docs/scoreboard.md): corpus results and the roadmap feature checklist.
- [`docs/tlc-differences.md`](docs/tlc-differences.md): where dive differs from TLC and why, with `--compat=tlc` restoring TLC's exact behaviour.
- [`docs/temporal-api.md`](docs/temporal-api.md): final temporal checking for Rust callers.
- `docs/tuple-only-trace-replay.md`: strict independent replay of proved tuple sequence calls and remaining namespace limits.
- [`crates/dive-sim/README.md`](crates/dive-sim/README.md): seeded finite simulation and its current limits.
- [`docs/pluscal-rewriting.md`](docs/pluscal-translation.md): bounded in-memory block replacement and explicit translation-body checksums.
- [`docs/decisions/`](docs/decisions/): architecture decision records.
- [`docs/survey/`](docs/survey/): oracles, corpora and the competitive analysis.
- [`docs/conformance.md`](docs/conformance.md): latest conformance summary (generated).

## Building

```sh
cargo xtask install-hooks   # once per clone
cargo build --workspace
cargo nextest run --workspace
```

The toolchain is pinned in `rust-toolchain.toml`, and rustup installs it automatically.

For the supported bounded temporal subset, ordinary checking uses final analysis
with an independently validated short lasso, both in the default profile
(`dive check Model.tla --output json`) and with
`dive check Model.tla --compat=tlc --fp 0 --seed 7 --output json`.
Eligible ordinary native-profile requests use deterministic completed-level
checks with independently validated prefix violations. Holding requires actual
exhaustion. Explicit final selection remains available in either profile with
`dive check Model.tla --lncheck final --lasso short --output json`.
See [temporal checking](docs/temporal-api.md#command-line-access) for scope,
lossless traces and compatibility limits.

The pure safety DFID slice is available with
`dive dfid Model.tla --max-level 8 --seed 7 --output json` and
`dive_api::dfid_file`. Reaching the depth bound reports incomplete exploration;
only a separate completion certificate permits `model_proved: true`. See
[DFID controls and limits](crates/dive-check/tests/fixtures/dfid/README.md).
