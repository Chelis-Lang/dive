# Changelog

Notable user-facing changes are recorded here. Pre-release development entries
are in git history.

## Unreleased

### Added

- The optional [finite symbolic checker](docs/user/symbolic.md) searches finite safety models using decision diagrams, rechecks invariant edits and verifies inductive predicates against the current source.

### Fixed

- Core grows the in-memory fingerprint table one shard at a time, packs trace
  predecessors, and bounds parallel safety candidate batches. On KVS Large
  the eight-worker checker finished within its 24 GiB memory limit.

- `dive simulate` no longer holds every sampled trace as a JSON tree before
  printing its report. Each trace is kept as its serialized entry and its
  concrete states are released once that entry is written, so memory per trace
  falls about tenfold: bosco at 1,000 traces x depth 100 peaks at 0.36 GB
  instead of 4.0 GB. Reports are byte-for-byte unchanged.
- Library users: `SimulateReport::write_json` writes the report text without
  building the JSON tree that `to_json` returns.
- A temporal check whose model has effects or run-time names (for example
  `SpanTreeRandom.cfg` and `MCKVsnap.cfg`) no longer refuses at `--workers`
  above 1 with "concrete pool capture with effects or runtime names". It
  builds the behavior graph with one worker and gives the one-worker result;
  the report's `options.workers.reason` and the human header say so.

### Performance

- Parallel Core temporal checks keep one worker team across graph-capture
  batches. In the CoffeeCan3000Beans eight-worker comparison, elapsed time
  fell from 107.3 s to 49.4 s.

### Documentation

- The user guides build as an mdBook: `mdbook build book`, then
  `python3 scripts/check-book-links.py book/book` checks its links. A new
  [install](docs/user/install.md) page covers crates.io, the Linux release
  archives and source builds.
- The user guides read more plainly: shorter sentences, fewer repeated
  caveats and no maintainer process notes. Stale statements about the removed
  Reference engine, host-input support and final/short temporal checking are
  corrected.
- [Getting started](docs/user/getting-started.md) and
  [temporal checking](docs/user/temporal.md) write the counter example inline,
  so their commands run in an empty directory after a crates.io or archive
  install.
- [Install](docs/user/install.md) describes the `dive --version` output of a
  crates.io build, which names the published commit's first 12 hex digits.
- `skills/dive/SKILL.md` describes how to run dive from an AI coding tool: the
  check command, exit codes, refusals, temporal checking and trace validation.

## 0.1.0

The first release.

### Compatibility

- Library users: `dive_api::TemporalMode` no longer has a `FinalShort`
  variant, which Core never implemented, and is `#[non_exhaustive]`. The CLI
  is unchanged: `--lncheck final --lasso short` (and `dive tlc -lncheck final`)
  still refuses temporal properties by name with exit 3, and a model without
  temporal properties is checked exactly as without the flags.
- JSON reports from every command carry `schema_version: 3`. Pre-release
  builds wrote schema 2; schema 3 replaces the single
  `options.temporal_bounds.atom_cells` limit with the separate
  `cell_limits.graph_labels`, `cell_limits.attribution_rows` and
  `cell_limits.word_truth` limits. Readers that pinned schema 2 must move to 3.
- Counterexample documents use `"format": "dive-trace"` with a version that
  depends on how the trace ends: 1 for the ordinary endings, 2 for an error
  while evaluating a named invariant, 3 for a Next step that leaves variables
  unassigned, 4 for an error while checking a generated candidate state, 5 for
  an error in a temporal property's source row and 6 for an error while
  evaluating VIEW. Readers accept all six and reject unknown versions. See
  [trace validation](docs/user/traces.md).
- Library users: the public result, error and kind enums of `dive-api`,
  `dive-runtime`, `dive-trace`, `dive-sema` and `dive-ir` are
  `#[non_exhaustive]`, so a `match` on them needs a wildcard arm. Later
  releases can add variants without a breaking change.

### Added

- Native TLA+ parsing and semantic analysis, supported PlusCal translation,
  and explicit-state model checking with no JVM.
- One checking engine, Core, behind `dive check`, `dive simulate`, `dive dfid`
  and the `dive tlc` shim.
- Safety and temporal checking, including weak and strong fairness, with
  symmetry, VIEW, CONSTRAINT and ACTION_CONSTRAINT support.
- Counterexamples independently validated against their specifications by
  `dive-trace`, which shares no search code with the checker.
- Schema-3 JSON operation reports, per-command help without input files, a
  one-line description of each command in `dive --help`, and `--version`
  naming the release tag or, for an untagged build, the commit.
- Distributed safety checking across worker processes (`dive check
  --distributed-worker`, `dive worker`) over authenticated TCP on Linux and
  macOS, with the same results as a local run and named failures; see the
  [distributed safety guide](docs/user/distributed-safety.md).
- Exact state storage (`--store exact`) for safety and temporal checks. The
  behavior graph keeps complete state identities, so states whose 64-bit
  fingerprints collide stay distinct. The human temporal report names the
  store, and a fingerprint run says that no collision probability was computed.
- Run budgets (`--time-limit`, `--max-memory`) that end a run as `stopped`
  (exit 5) with the phase and limit named. The opt-in `--max-states` and
  `--max-depth` ceilings apply to safety and temporal checks; a temporal
  capture stopped by a ceiling reports no counterexample and no holding
  result. There is no default time limit;
  `--max-memory` defaults to half of physical memory.
- Seeded simulation (`dive simulate`) that reports its seed, so `--seed`
  replays the run, and depth-first iterative deepening (`dive dfid`).
- Embedded standard modules, TLAPS and CommunityModules, with local modules
  and explicit search paths taking precedence; ordinary imports need no extra
  library flags.
- Experimental: `dive debug`, a Debug Adapter Protocol adapter over stdio or
  loopback TCP for Core safety checking in both profiles, with source and
  exception breakpoints, forward steps and bounded scalar inspection; see
  [debugging](docs/user/debugging.md).
- Linux x86_64 and aarch64 archives with checksums, both command entrypoints,
  and the bundled libraries' license and provenance notices.
- Published on crates.io: `cargo install dive-cli --locked` installs the
  `dive` and `tlc` commands, and `dive-api` with its library crates is
  available to Rust callers ([Rust API guide](docs/user/rust-api.md)).

### Results on the tlaplus/Examples corpus

- Of the 176 in-scope models with a TLC result, Core agrees with TLC on 162,
  matches a registered difference on 7 and refuses 7 by name, with no
  disagreements. Seven public real-world specification repositories also show
  no disagreements.
- At one worker, end to end including TLC's JVM start-up, Core takes about a
  fifth of TLC's time on the geometric mean (0.195x over 169 models), and no
  model takes more than twice TLC's time. The
  [measurement procedure](docs/developer/performance.md#r2-measurement-procedure)
  runs one checker at a time on quiet machines.

### Differences from TLC

- The default profile uses canonical TLA+ semantics where TLC's behavior is
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
- Not in the first release: checkpoint and recovery breadth,
  TLC coverage and logical costs, spec I/O beyond explicit grants, simulation
  with a temporal PROPERTY, VIEW with liveness, distributed temporal checking,
  and LSP.
- EWD998ChanID imports resolve, but checking refuses it by name because it
  combines VIEW with a liveness property.

### Known issues

- With temporal properties, extra workers may not help and can make a check
  slower: on some models a check at 8 workers took up to twice as long as at
  1 worker, and the report does not say why. Results are the same at every
  worker count. Use `--workers 1` for specifications with temporal properties
  until this is fixed.
