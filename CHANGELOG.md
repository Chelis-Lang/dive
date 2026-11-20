# Changelog

Notable user-facing changes are recorded here. Pre-release development entries
are in git history.

## Unreleased

### Added

- Core packs ordinary in-memory safety frontiers by variable. Peak process memory fell about 42% on the measured SlateDB and DiskSynod prefixes. [Inputs, counts and wall times](docs/developer/large-models.md#resident-frontier-rows).

- `dive check --partitioned-fingerprints` supports parallel safety search with disk membership beyond four billion entries. The [storage guide](docs/user/disk-seen-safety.md#multi-worker-fingerprint-membership) covers capacity, audit results and the measured throughput cost.

- Ordinary Core safety searches can put [independently compressed frontier chunks](docs/user/storage-recovery.md#independently-stored-frontier-values) on disk and reclaim compound values between worker batches.

- Core reduces the number of sources expanded together in high-fanout parallel safety searches. On BPConProof this cut peak RSS by 59% at one million discoveries. [Measurements](docs/developer/large-models.md#high-fanout-parallel-safety).
- `dive simulate` accepts `SYMMETRY` and `VIEW`, which do not change a
  simulation, checks state-level `PROPERTY` parts on initial states (2108),
  and checks action properties together with constraints. Reports render
  `ALIAS` beside each trace's states and label each step with its action. See
  [simulation](docs/user/simulation.md#what-a-sample-covers).
- `dive_trace::TraceDoc` and `Trace` gain `concrete_closure`
  (`"closure": "concrete"`), which lets a periodic word whose loop closes on
  equal states validate under `VIEW` and `SYMMETRY`. Code that builds these
  structs field by field must set it.

- Core uses one worker for temporal captures with a wide initial frontier and simple actions, and reports why in `options.workers`.
- Core temporal checks use compact successor ordinals while retaining exact trace and saved-graph identities. See the [matched memory comparison](docs/developer/large-models.md#successor-ordinals-in-temporal-graphs).
- Core supports inclusive subsequence predicate searches and folds multikey
  bags in canonical key order. Finite bag results accept materialized record keys with different payload shapes. See [bag fold order](docs/tlc-differences.md#d-031-bag-fold-order).

- Scalar CFG assignments can close input-dependent definitions used through a module instance. See [model input selection](docs/user/checking.md#select-a-closed-model-input-in-the-cfg).
- Temporal predicates can count a filtered set with `Quantify`, sum a function
  over a set, and inspect finite next-state domains in nonrecursive binders.
  See [temporal checking](docs/user/temporal.md).

- Core names the temporal formula planning bound and source location when checking reaches it.

- The Linux release archives include `dive check --strategy symbolic` for
  finite safety checks, validated witnesses and independently verified predicates.

- Symbolic safety searches can [learn the members of wide finite sets](docs/user/symbolic.md#learning-which-members-a-wide-set-needs) with `--refine-domains`.

- Symbolic safety checks support [finite state constraints and ground assumptions](docs/user/symbolic.md#admission-and-reports), including independently validated violations on excluded successors.

- Symbolic safety checks accept [multiple finite initial choices](docs/user/symbolic.md#finite-initial-choices), including duplicate and symmetric witnesses.

- Embedders can qualify a completed native temporal check after retaining its
  first property on unchanged aggregate-state programs. See [retaining a claim](docs/user/rust-api.md#retain-the-first-temporal-property).

- The conformance harness can run selected models without TLC goldens with
  `--observe-missing-goldens` and record their ungraded outcomes. This includes
  the four holdouts excluded because the pinned TLC exceeded its deadline.
- Rust embedders can project an exhausted safety proof onto a smaller
  invariant inventory, including constrained models with aggregate state.
  See [the Rust API guide](docs/user/rust-api.md#drop-an-invariant-from-a-completed-safety-check).

- The optional [finite symbolic checker](docs/user/symbolic.md) searches finite safety models using decision diagrams, rechecks invariant edits and verifies inductive predicates against the current source.
- Rust embedders can capture a finite single-worker Core safety check and
  recheck edited invariants on the original state rows. A qualified
  refutation includes its current independently validated witness. See
  [the Rust API guide](docs/user/rust-api.md#recheck-edited-safety-invariants).

- Rust embedders can also recheck edited temporal properties over a completed
  scalar Core graph. The checker replays ordinary scheduled prefix analyses,
  evaluates fairness on the retained labelled transitions and independently
  validates liveness witnesses. See [temporal property reuse](docs/user/rust-api.md#recheck-temporal-properties).

### Removed

- JSON reports from every command now carry `schema_version: 4`
  (`dive_api::REPORT_SCHEMA_VERSION`). Schema 4 removes the always-zero
  construction-history metrics `core.metrics.store.memory.construction_history_view_bytes`
  and `core.metrics.candidate_storage.{origin_index_bytes, peak_origin_index_bytes, origin_index_bytes_limit_per_worker}`.
  To migrate, accept version 4 and stop requiring those four fields; readers
  of stored reports may keep accepting 3. Trace document versions are
  unchanged. See
  [report compatibility](docs/user/reports-and-errors.md#report-compatibility).
- Core checkpoint archives move to format `DIVECC02`. A `DIVECC01` archive is
  refused with `DIVE-S0400` by name; recreate it with the current executable.
  Distributed workers must run the same dive build as the coordinator.

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

- Symmetry reduction compares nested function and set values without copying their contents. On the [SlateDB WAL comparison](docs/developer/large-models.md#symmetry-comparison-costs) this reduced eight-worker elapsed time by 10%.

- The finite symbolic checker supports string-valued fields and grouped bounded
  quantifiers in state predicates, including the original TCommit safety model.

- Exact symbolic reachability now supports layouts above 52 bits and returns
  integer state and transition counts throughout its 512-bit admission range.

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
