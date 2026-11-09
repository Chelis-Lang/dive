# Changelog

Notable user-facing changes are recorded here. Pre-release development entries
are in git history.

## Unreleased

## 0.1.0

Prepared, **not yet published**: no tag or release exists. The
[release gates](docs/RELEASE.md) pass on the release candidate except the tag
itself.

### Added

- Native TLA+ parsing and semantic analysis, supported PlusCal translation,
  and explicit-state model checking with no JVM.
- One checking engine, Core, behind `dive check`, `dive simulate`, `dive dfid`
  and the `dive tlc` shim.
- Safety and temporal checking, including weak and strong fairness, with
  symmetry, VIEW, CONSTRAINT and ACTION_CONSTRAINT support.
- Counterexamples independently validated against their specifications by
  `dive-trace`, which shares no search code with the checker.
- Schema-3 JSON operation reports, per-command help without input files, and
  `--version` naming the release tag or, for an untagged build, the commit.
- Distributed safety checking across worker processes (`dive check
  --distributed-worker`, `dive worker`) over authenticated TCP, with the same
  results as a local run and named failures; see the
  [distributed safety guide](docs/user/distributed-safety.md).
- Run budgets (`--time-limit`, `--max-memory`) that end a run as `stopped`
  (exit 5) with the phase and limit named. There is no default time limit;
  `--max-memory` defaults to half of physical memory.
- Seeded simulation (`dive simulate`) that reports its seed, so `--seed`
  replays the run, and depth-first iterative deepening (`dive dfid`).
- Embedded standard modules, TLAPS and CommunityModules, with local modules
  and explicit search paths taking precedence; ordinary imports need no extra
  library flags.
- Linux x86_64 and aarch64 archives with checksums, both command entrypoints,
  and the bundled libraries' license and provenance notices.

### Results on the tlaplus/Examples corpus

- Of the 176 in-scope models with a TLC result, Core agrees with TLC on 162,
  matches a registered difference on 7 and refuses 7 by name, with no
  disagreements. Seven public real-world specification repositories also show
  no disagreements.
- At one worker Core takes about a fifth of TLC's time on the geometric mean,
  and no model takes more than twice TLC's time, in a single-pass measurement
  with four models run concurrently. The gate measurement under the
  [R2 procedure](docs/developer/performance.md) is pending.

### Differences from TLC

- The default profile uses canonical TLA+ semantics where TLC's behaviour is
  an implementation artifact: unbounded integers, canonical CHOOSE and print
  order, canonical exploration order, and mathematical comparisons of values
  TLC rejects. Each difference is registered in
  [docs/divergences.md](docs/divergences.md) and described in
  [docs/tlc-differences.md](docs/tlc-differences.md).
- `--compat=tlc` selects the inexpensive TLC-compatible switches; exact
  reproduction of TLC implementation artifacts is not promised, and its known
  gaps are listed in docs/tlc-differences.md.

### Limits

- Unimplemented requests refuse by name (verdict `unsupported`, exit 3);
  budget stops are reported as stops, never as success.
- Not in the first release: the debugger, checkpoint and recovery breadth,
  TLC coverage and logical costs, spec I/O beyond explicit grants, simulation
  with a temporal PROPERTY, VIEW with liveness, distributed temporal checking,
  and LSP.
- EWD998ChanID imports resolve, but checking gives its named VIEW/liveness
  refusal.
