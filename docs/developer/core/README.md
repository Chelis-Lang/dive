# Core execution engine

Core is the product engine under [ADR 0153](../../decisions/0153-core-is-the-product.md).
The [release gates](../../RELEASE.md) define the work and acceptance criteria;
the [scoreboard](../../scoreboard.md) holds the current measurements.
The Reference checking runtime is retained until Core meets R1. Its only
permitted changes are fixes for wrong user-visible results; shared startup and
independent trace validation remain supported.

## How to resume

1. Read [AGENTS.md](../../../AGENTS.md), ADR 0153 and the release gates.
2. Select an existing issue in the `v0.1: first value` milestone. Rank Core
   coverage work by the measured refusal-reason histogram.
3. Work in an owned worktree and build directory, with at most three live
   worktrees across the team. Remove each tree after its PR merges.
4. Use focused checks, then required CI. Full qualification runs only for a
   release candidate or an engine-cutover candidate.

The default profile uses canonical sets (D-028). Do not extend TLC
normalization history, receiver identity or construction authority.
Compat is nominal; inexpensive existing switches and a documented gap list
are its scope. Frozen modes, storage, distribution, debugging and LSP work are
listed in ADR 0153.

## Design documents

These documents explain the implementation. Their older workstream plans,
performance targets and compatibility proposals do not override ADR 0153 or
[RELEASE.md](../../RELEASE.md). Update behavior descriptions with the code;
keep historical measurements in git and their existing reports.

| Document | Contents |
|---|---|
| [Semantic contract](02-semantic-contract.md) | Observable results and semantic boundaries |
| [Value model](03-value-model.md) | Handles, store, arenas, identity and hashing |
| [Plan compiler](04-plan-compiler.md) | Binding, compilation and evaluation |
| [Successor generation](05-successor-generation.md) | Actions, state generation and deduplication |
| [Memoization](06-memoization.md) | Pure evaluation keys and cache soundness |
| [Engine](07-engine.md) | Search, stores, determinism, traces and runtime names |
| [Correctness](08-correctness.md) | Golden comparison, mutations and independent validation |
| [Performance method](09-performance-method.md) | Benchmarking, profiling and ablations |
| [Plan](10-plan.md) | Current release stages |
| [Status](m1-status.md) | Links to current measurements and release work |
| [Liveness](11-liveness.md) | Capture, temporal analysis and budgets |
| [Motivation](01-motivation.md) | Original performance diagnosis |
| [Spike results](m0-results.md) | Historical Core prototype measurements |
| [TLC baseline](tlc-baseline.md) | Pinned TLC behavior and implementation survey |
