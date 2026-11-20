# dive-conformance

`compare-models --oracle-model FILE --model FILE --config FILE --dive-bin BIN
--out DIR [--module-path DIR] [--model-id CORPUS_PATH] [--timeout SECS]` checks
staged PlusCal translations using pinned TLC and dive in both semantic profiles.
The configuration bytes must match the oracle module's same-named configuration.
`--out` must be fresh and retains raw child output and physical status receipts.
`--overall-timeout SECS` bounds the complete comparison (default four child
timeouts plus 60 seconds), including source binding and independent replay.
The deadline cancels and drains registered children before process termination.
Results use ordinary conformance classification and independently replay native
counterexamples; exit 1 retains any non-agreeing observation. This command does
not write goldens or certify a whole translator corpus.

The differential conformance harness. It:
- fetches and verifies the pinned TLC/SANY oracles, the Java runtime and the corpora;
- generates golden results from TLC and SANY;
- runs an implementation against those goldens: TLC itself, `dive` (its front end today: `dive analyze` and `dive parse` against the SANY goldens), and external tools later;
- writes per-model reports.

Read with ADRs [0002](../../docs/decisions/0002-oracle-pinning.md) (pins), [0003](../../docs/decisions/0003-golden-format-and-provenance.md) (goldens, invocation, classification), [0004](../../docs/decisions/0004-worker-count-and-depth-semantics.md) (workers and depth) and [0005](../../docs/decisions/0005-reuse-examples-manifests.md) (Examples manifests).

The generated `validation-smoke` corpus covers 16 pinned ValidationTestSuite
models and 20 modules. See `docs/conformance-corpora.md` for its complete
selected-case census, scope, licence and reproduction procedure.

## Design

```text
conformance/oracles.toml, corpora.toml   pins (config.rs)
        │
fetch.rs ── curl/git ──► $DIVE_CACHE_DIR (cache.rs), SHA-256 verified (hash.rs)
        │
oracle.rs   OracleEnv: verified jar + classpath + module libraries + Java + probed capabilities
        │      runs TLC / SANY through process.rs (timeout, process-group kill)
        ▼
tlc/messages.rs   -tool stream parser ──► tlc/result.rs  OracleResult + classification
sany.rs           SANY accept/reject
        │
golden.rs   goldens (payload + provenance)        corpus.rs   manifests, tiers, scope
compare.rs  outcomes, field diffs, worker invariance, manifest drift
implementation.rs   Implementation trait: TLC, dive, external adapter (stub)
dive.rs             dive: runs the `dive` binary (analyze/parse JSON), module search path
pipeline.rs / report.rs   commands, JSON report, docs/conformance.md
```

**The harness never guesses.**
- Every TLC run is classified from its `-tool` message stream, then cross-checked against the exit code.
- An exit code that the messages do not explain is `oracle-error`, with both recorded.
  One source-verified exception is TLC’s finite temporal-prefix path: it prints
  temporal code 2116 but stores invariant code 2110, exiting 12. The harness
  requires the finite counterexample header, consecutive nonempty state messages,
  completion/statistics and no other errors or cycle markers. It retains both
  the temporal verdict and actual exit 12; cyclic cases still require exit 13.
- A narrowly verified Init-exception route keeps `other-error / GENERAL1000`
  with physical exit 75: the original GENERAL owns a complete known NONE cause
  (2215, or 2154 wrapping2178), and later Init diagnostic replay reports2103.
  Parser-owned containment, exact placement/templates and the complete Init
  lifecycle are required; no state/trace or completed-Init route is admitted.
  The cause is evidence only, not a replacement error class. Public flat
  transcript classification stays conservative; unknown GENERAL/context-only
  errors, including bare interval exception text, remain unclassified.
- An unknown exit code or message code, or a malformed stream, is `unclassified`, with the raw data kept.
- JVM resource exhaustion (out of memory, stack overflow) is `resource-limit`: never an oracle fact, never a golden.
- Golden generation refuses oracle errors, unclassified outputs, timeouts and
  resource exhaustion before creating or replacing a model golden. SANY writes
  only classified acceptance, parse rejection or semantic rejection. A refused
  result counts as a failed generation even with `--reason`; existing files stay
  intact. Classified specification errors remain valid oracle facts.
- Unexpected stderr, or a success without statistics, is `oracle-error`.
- An implementation that cannot do something must answer `unsupported`.

**Each model gets exactly one outcome:**
- `agree`;
- `agree-pending-trace-validation`: every compared field agrees, but the harness's independent trace validator (charter rule 2, ADR 0016) could not confirm the counterexample: it refused part of the claim by name, or there was no document. It is never counted as `agree` and is not a finding (the PR conformance gate fails on it);
- `disagree`, with a field-by-field diff;
- `unsupported`;
- `oracle-error`;
- `timeout`;
- `out-of-scope`;
- `divergence-mode` and `expected-divergence`: for `dive` and adapters, see "Profiles, options and expected divergences" below;
- `known-upstream-change`: for TLC oracles only, see "Oracle roles and known upstream changes" below.

None of the last three ever counts as `agree`, and none is a finding.

## Oracle roles and known upstream changes

**Roles.** Every `[[tlc]]` in `conformance/oracles.toml` has a required `role` (ADR 0002):
- `gating`: its goldens and runs decide conformance. The default oracle must be gating. Today the only gating oracle is `tlc-2026.10.06.014338`.
- `reference`: kept for comparison only. Today this is `tlc-1.7.4`. A reference oracle never gates:
  - the harness uses it only when it is named: `oracle fetch|verify --oracle <id>`, `golden generate --oracle <id>`, `run --impl tlc:<id>` or `run --golden-oracle <id>`. Without `--oracle`, `oracle fetch` and `oracle verify` cover gating oracles only;
  - a `run` in which it takes part, on either side, exits 0 even with findings. It prints how many there were;
  - the report records `golden_oracle_role`, `implementation_oracle_role` and `reference: true`, and the Markdown summary says "Reference run".

**Known upstream changes.** `conformance/oracle-changes.toml` is the machine-readable side of `docs/internal/survey/oracle-history.md`. When a TLC oracle runs against another oracle's goldens, a difference that registered changes fully explain is reported as `known-upstream-change`, with the change ids and upstream commits in the detail. A difference that no entry explains stays `disagree`.

An entry applies when all of these hold:
- the two oracles are one from `absent_in` and one from `present_in`, in either role;
- the model or module path matches its `models` or `modules` globs;
- its optional `absent_result` and `present_result` conditions hold (verdict, error class, unresolved modules, distinct-state count).

A difference is explained only if every differing field matches the `fields` globs of an applicable entry. Unresolved modules must be covered by the applicable entries together.

The registry never applies to `dive` or to external adapters, and it never turns a difference into `agree`. Entries are validated when loaded:
- every oracle id must be pinned;
- every field glob must match a compared field;
- the upstream commit must be a SHA or a `<from>..<to>` range.

The seeded entries explain the v1.7.4-vs-pin report: CommunityModules, `TLCExt`/`Json`, `ALIAS`, and depth 1 for constant-only models. Two differences there are not attributed to a commit and stay `disagree`; the file's header lists them.

**Comparison rules** (ADR 0004 and its amendment, with ADR 0086's observable contract):
- Every implementation is compared with the single-worker golden on:
  - the verdict class and the error class (the `EC` name of the deciding message);
  - the physical numeric process exit, including an absent code;
  - the violated property, when the golden names one, subject only to the independently validated liveness attribution rule below;
  - distinct counts and depth at every worker width, and generated counts at one worker;
  - finite counterexample length and ending, retaining the one-worker shortest-BFS contract.
  Queue population and arbitrary periodic lasso choice/length/ending are not
  parity criteria. Raw queue values remain recorded; a final temporal producer
  must still establish its own exhausted-capture queue consistency. Every
  periodic word still requires independent validation.
- Liveness names are relative to the actual reported word (ADR 0126). A different
  name list from TLC is accepted only after fresh independent validation of the
  whole native word and its complete false-property set, with the reported list
  exactly matching that word's names in TLC display format. Default result
  comparison without this proof stays strict. Missing, invalid, forged, refused,
  unknown/duplicate/omitted/false or misreported claims never gain an exception;
  generic successful validation callbacks do not establish attribution. All
  other fields, and every non-liveness property name, retain their prior rules.
- The counterexample's content is never compared with TLC's. Instead, for an implementation that is not TLC, the harness validates the counterexample itself (`Implementation::trace_validated`, `src/trace_check.rs`; ADR 0016 §3). `dive`'s counterexample document (in `OracleResult.trace.json`, dive's trace format) is read and checked in-process by `dive-trace`'s independent validator, with the model bound from the same spec, cfg, profile and module path; `dive`'s own `validation` field is ignored. A deadlock, which the validator refuses by name (ADR 0016, amendment 1), counts as validated only when every other check passed and its last state equals the last state of the golden's own counterexample. A safety, action-property, liveness or deadlock violation cannot agree without a whole counterexample. A selected golden carrying an error witness also requires a whole word when verdicts match, even if its payload is unavailable. An approved semantic Success divergence such as P2Choose_str carries no counterexample merely because TLC's differing verdict has one; it never counts as Agree. Absent or metadata-only traces are unconfirmed without invoking a claimed-valid callback; present JSON or text-step payloads still require independent validation against the same selected primary or worker-specific golden as comparison. Ordinary success, assumption, frontend or evaluation outcomes whose contract carries no witness remain legitimate without one. A violation whose counterexample does not validate cannot agree. When the validator refuses part of the claim by name, or no document is available, and nothing else differs, the outcome is `agree-pending-trace-validation`; when the counterexample is invalid, it is a `disagree` on `trace.invalid`. The validator's reason is in the detail.
- Only a TLC oracle gets the separate historical multi-worker self-check policy. With more than one worker, its counts are compared only after a complete search, and depth, counterexample and property are not compared. This oracle policy is not lent to native implementations.
- For TLC, a model's outcome is its single-worker run. Multi-worker runs appear only in the worker-invariance section.
- For anything else, the required verdict, class, physical exit, non-liveness property names, distinct counts and depth remain strict at every worker count; generated-count parity is required at one worker. A difference in a required field is `disagree`. Missing or invalid counterexamples cannot agree.
- v1.7.4 may omit violated temporal-property names, because its capability probe shows it never prints them.
- Models whose module reads `TLCGet("config")` are flagged `worker_introspecting`. Under `--impl-profile tlc` (or `tlc+overrides`), dive reports the physical worker count as TLC does (D-011), so such a model's run at N > 1 workers is compared with the worker-specific golden at N, if there is one, on verdict, error class and physical numeric exit, and is left out of the worker-invariance judgement (ADR 0004, amendment of 2026-10-09). Without one, the single-worker golden applies on every field. The default profile reports the logical count, 1, and always uses the single-worker golden.
- Simulation models compare verdict-level fields only.
- Every selected model and module gets exactly one outcome; one without a golden is `oracle-error`. The exception is an implementation that refuses every model (`Implementation::models_unsupported`): its models are `unsupported`, and the detail notes a missing golden. The summary asserts that the totals equal the selection.
- Pass `--observe-missing-goldens` to run selected in-scope models without a golden at each requested worker count. The model row stays `oracle-error`. Its `runs` record the implementation verdict, counts, elapsed time and any refusal or stop reason. They establish no parity or trace certification. Each observation uses the usual per-model timeout and resource options. A corrupt or outdated golden remains a harness error.

## Corpora

`conformance/corpora.toml` pins each corpus and its `layout`:
- `examples-manifest` (tlaplus/Examples): modules and models from the per-spec `manifest.json` files (ADR 0005).
- `modules-only`: every `.tla` file is a module for SANY goldens, and there are no models. Hidden directories are included (CommunityModules keeps smoke tests in `.smoke/`); `.git` is not.
  - `include = [globs]` narrows the modules (`tlaplus-tests`: `tlatools/org.lamport.tlatools/test/**` and `test-model/**`, 808 files).
  - `[[corpus.exclude]]` entries (`path`, `reason`) are modules that are enumerated but never run, for example because they crash SANY. They are reported as `out-of-scope` with the reason, never dropped. An exclusion that names no enumerated module is a configuration error.
  - A job copies only the files of the module's own directory into its scratch directory, since SANY never looks into subdirectories. An Examples job copies the whole spec directory.
- `tbd`: not yet enumerated.

**Holdout corpora** (ADR 0005, amendment 2) pin an upstream repository by
`repo` and `commit` and add `manifest = "conformance/holdout/<id>.json"`: a
dive-owned manifest in Examples' schema that lists the selected modules and
models by plain corpus-relative path (no `..`, `.`, empty or absolute
segments). Every listed file must exist at the commit;
unlisted upstream files are not reported as uncovered. `[[corpus.exclude]]`
names model ids (`.cfg` paths) reported as `out-of-scope` with their reason.
The identity goldens record is `<commit>+manifest-sha256:<hex>`. Sources stay
upstream; `corpus fetch` checks them out like any pinned corpus.
A plain `corpus fetch` with no `--corpus` clones every pinned corpus,
including the seven holdout repositories (microsoft/CCF is large); pass
`--corpus examples` (as CI and the gate do) to fetch only Examples.

Finite Simulate entries with feature `constructive generation` invoke pinned TLC
`-generate num=N` and native `simulate --generate`; the feature refuses outside
Simulate. Native sampling admission checks actual request/profile/algorithm,
engine, physical TLC exit and complete finite work, then independently validates
retained words. It binds the requested/default aril fields and finite-safety mode,
and requires generated work to cover every actual Init candidate plus all
retained transitions, using checked arithmetic; rejected candidates may add work.
The deciding violation must be last and within the requested depth, with its
initial/successor endpoint shape. Every earlier Behavior sample must complete
the full requested depth; unproved early completion receives no exception. Behavior samples additionally require the explicit fresh
`validate-trace --sample` result and its `validation_goal: sample` marker.
Producer validation booleans carry no authority. Complete samples remain
non-exhaustive; arbitrary draw/count equality is excluded by ADR 0116. Ordinary
exhaustive comparison rules are unchanged. The legacy manifest `generate` mode
is not reinterpreted by this feature.

**In-repository corpora** (ADR 0005, amendment 1) give `path = "conformance/..."` instead of `repo` and `commit`. Their identity is the SHA-256 of their files (`sha256:<hex>`, recorded in goldens as the corpus commit), so changing any file makes `run` refuse the goldens until they are regenerated. `corpus fetch` skips them. These include:
- `constraint-error-endings`: eight terminal safety/constraint demand controls
  pinned before the validator repair (#499). Official generation wrote eight
  SANY and four TLC goldens. The invariant-error streams for
  `ConstraintInvError`, `ActionConstraintInvError`, `FirstFalseConstraint` and
  `InitialConstraintInvError` remain unclassified, with no model golden or
  agreement credit. Compact consumed source pins and qualified raw observations
  are in `crates/dive-api/tests/fixtures/constraint-error-endings/oracle.json`.
  Select the models with `--corpus constraint-error-endings --tier all`.
- `configured-temporal-operators` (`conformance/corpus/configured-temporal-operators/`):
  two direct configured Reply temporal controls, holding and violated, with
  generated TLC/SANY goldens. This separate corpus preserves the existing
  `probes` input identity. Run it with `--corpus configured-temporal-operators`.
- `probes` (`conformance/corpus/probes/`, `examples-manifest` layout): the honesty and determinism probes of `docs/internal/survey/competitive.md` (P1–P9, M1–M2), 12 modules and 14 models with goldens from the gating oracle. Run it with `--corpus probes`.

## Commands

```sh
cargo run -p dive-conformance --release -- <command>      # or target/release/dive-conformance

oracle fetch [--oracle ID]       # download + verify gating oracles (or ID), classpath jars, the host JRE, module libraries
oracle verify [--oracle ID]      # re-verify the cache and print the gating oracles' (or ID's) probed capabilities
corpus fetch [--corpus ID]      # git checkout of pinned corpora; submodules are recorded, not fetched
corpus list  [--tier small|deep|all] [--filter GLOB]
golden generate [--oracle ID] [--tier T] [--filter GLOB]... [--reason TEXT] [--no-sany|--sany-only] [--workers 1,4] [--raw-invocations DIR] [--raw-setup-status]
run --impl tlc|tlc:<oracle-id>|dive|dive:<engine>|adapter:<file.toml>
    [--corpus ID]... [--golden-oracle ID] [--workers 1,4] [--crosscheck-manifest] [--tier T] [--filter GLOB]...
    [--sany-phase full|parse] [--dive-bin PATH] [--impl-profile P] [--impl-option N[=V]]...
```

The `shadow` command, which compared Core with the reference engine, was
retired with the reference engine (#791): golden agreement is the evidence.

`run` takes several `--corpus`, into one report and one summary (default: `examples`).

`trace-controls [--corpus ID]... [--tier T] [--filter GLOB]... [--golden-oracle ID]` runs the trace validator's controls (ADR 0016 §3) over every golden counterexample of the selection, under `--compat=tlc`, in-process: each TLC `-dumpTrace json` document must validate (positive controls; a deadlock may be refused by name if every other check passed), and every by-construction mutant of it (`dive_trace::mutate`) must be rejected (negative controls). Goldens with `trace.json_unavailable` or an omitted trace are skipped and counted. It prints a Markdown table and exits 1 on any finding. Named validator refusals remain listed; they are not validation credit. The table reports the actual positive and mutation outcomes for the selected goldens.

**Common options:**
- `--jobs N` (default `min(4, cpus/2)`): parallel oracle runs.
- `--xmx SIZE` (default `3g`): JVM heap per run. Four 3 GB JVMs fit an 8-core / 24 GB machine with room to spare.
- `--timeout SECS`, or the derived default:
  - 20 × the manifest runtime + 120 s, at least 300 s, capped by `--timeout-cap` (default 3600); unknown runtimes get the cap;
  - in `run`, also at least 4 × the golden's recorded elapsed time + 60 s, even above the cap. Manifest runtimes were measured with several workers.

  By default `dive check` runs with `--time-limit none --max-memory none`, so only this timeout applies, as before. `--tlc-budgets` (ADR 0107) instead gives a model with a golden a budget derived from TLC's recorded single-worker time, `max(60 s, 10 × golden elapsed)`, hard-capped by `--budget-cap` (default 900). dive receives it as `--time-limit` in both profiles (labelled `tlc+overrides` under compat) and `--xmx` as `--max-memory`, reports a structured `stopped` result, and is killed only 30 s after its limit. Either stop is recorded as `timeout`, a finding. The gate, smoke/cm1 scripts and nightly model checks select `--tlc-budgets --budget-cap 900` explicitly. The strict temporal reader accepts only the harness-requested budget override with unchanged TLC semantic settings; other overrides remain ineligible. On Linux/macOS, timeout handling sends `SIGKILL` to the owned group and retains the leader through a bounded quiescence check. Ctrl-C (`SIGINT`) and `SIGTERM` permanently block new work and attempt the same owned-group cleanup before exit. An unproven drain is a harness error requiring external supervision (ADR 0112).
- `--tlc-timings REPORT` (`run --impl dive` only): reuses a same-host `run --impl tlc` report at one worker for R2. Each model that report completed gets a Core budget of `max(10 s, 2 × TLC elapsed)`, hard-capped by `--budget-cap`. The option implies `--tlc-budgets`, so untimed models keep that policy, and it conflicts with `--timeout`. After the run the harness prints the R2 summary: the dive/TLC ratio for each agreeing model, their geometric mean, budget stops as lower bounds, the models above 1x and above 2x, and the untimed models. It also writes the summary to `<report-dir>/<run-id>-r2.json`. See `docs/developer/ci.md`.
- `--java PATH`: opts into a system Java. It is recorded in provenance as `source: system`, with the executable's SHA-256.
- `--repo-root DIR` and `--cache-dir DIR`.

**Outputs:**
- `run` writes `conformance/reports/<run-id>.json` (schema version 1) and writes a detailed local summary to `docs/conformance.md` by default. The checked-in page is a compact source-pinned snapshot; keep detailed generated output outside the repository with `--summary PATH`, or select `--no-summary`. The detailed generated summary keeps one section per implementation and profile (`tlc:<oracle>`, `dive (tlc)`, `dive (dive)`, ...); a run replaces only its own section (issue #27). `report summarise <report.json>...` re-renders existing reports' sections without running anything. Use `--no-summary` to skip the summary, or `--summary PATH` to put it elsewhere.
- `run --baseline <file>` compares the run with a committed baseline (`conformance/baselines/`, written by `report baseline <report.json> --out <file>`) or a full report of the same implementation, profile, golden oracle, tier, workers and corpora. It prints one line per change and fails if a model or module that was `agree` is now anything else or missing, or if any model result's observables (verdict, error class and code, physical exit, violated property, distinct and generated counts, depth) changed while it does not agree. Improvements to `agree` (whose observables then match the golden) and other outcome changes are listed and pass. A filtered run checks only what it selected. See `docs/developer/ci.md`.
- Exit status: 0 for success; 1 for findings (unwritten goldens, or `disagree`, `oracle-error` or `timeout` outcomes, except in runs involving a reference oracle, or `--baseline` regressions); 2 for a usage error, including an empty selection or stray arguments; 3 for a harness error.

**Examples:**

```sh
dive-conformance golden generate --tier small                     # all small models + SANY goldens
dive-conformance golden generate --tier all --no-sany --filter 'specifications/DieHard/MCDieHardest.cfg' \
    --workers 3,4,8 --reason "<why>"                                 # worker-specific goldens (worker-introspecting models only)
dive-conformance run --impl tlc --crosscheck-manifest              # self-consistency + manifest drift
dive-conformance run --impl tlc:tlc-1.7.4 --no-summary             # cross-oracle diff vs default goldens (reference: never gates)
dive-conformance run --impl tlc --workers 1,4 --filter 'specifications/ewd840/*'
cargo build --release -p dive-cli                                  # then, for dive's front end:
dive-conformance run --impl dive --tier all --corpus examples --corpus tlaplus-tests --corpus community-modules
dive-conformance run --impl dive --sany-phase parse --sany-only --corpus tlaplus-tests   # dive parse vs SANY's parse phase
dive-conformance run --impl dive:core --impl-profile dive --no-sany --baseline conformance/baselines/small-core-dive-w1.json   # the Core ratchet
```

`golden generate --raw-invocations DIR` optionally retains original TLC model
invocations outside the golden format. The default is off. It adds no oracle
passes: each existing clean run and any existing cross-checked trace run gets a
unique directory under `DIR`. Java version checks, capability probes and SANY
runs are outside this capture option.

`DIR` must be outside every directory copied for the selected model, including
directories reached through source symlinks. Physical ancestry and missing path
suffixes are checked before creating the capture directory, so an alias cannot
make isolation copy into its own output. The default temporary path is unchanged.

Each directory receives an atomic `started.json` before dispatch, containing the
actual executable/arguments, working directory, requested timeout, model/config
and clean/trace phase. Inherited environment variables are never recorded.
Arguments must be UTF-8 when recording is enabled. `stdout.txt` and `stderr.txt`
receive the original bytes directly; `trace.json` exists only if TLC emits it.
The directory also holds the isolated model inputs and any TLC scratch files.
These original files can contain machine paths and spec output; they are external
run evidence, never files to commit.

When execution returns, `completed.json` records the actual exit code, signal,
timeout flag and elapsed duration, or `harness-error.json` records a harness
failure without inventing a native outcome. A missing terminal marker means the
invocation is incomplete; `started` alone does not prove that a child launched.
Partial files survive cancellation and a killed harness. Establish child-process
quiescence before collecting them: file retention does not prove cleanup.
Recording failures fail the model job, including a terminal recording failure
after TLC exits; an uncaptured result cannot become a golden. Unknown outcomes,
timeouts and harness failures retain their available streams even when no golden
is written. The caller owns collection and removal of these directories.

`--raw-setup-status` requires `--raw-invocations DIR` and is also off by
default. It records setup separately under `DIR/setup-status`, without changing
the TLC model-record schema or adding model passes. Each fresh session retains
Java version, TLC help, SANY and temporal capability probes, jar-manifest
extraction, and any module-library/corpus Git verification subprocesses. The
final cached-corpus pristine check is retained there too. These setup commands
still execute on each preparation. Verified cached archives are marked
`not-executed` with `cache_hit: true`. Freshly computed in-repository corpus
digests are separate in-process verification records with `cache_hit: false`.
Neither records an invented native exit status.

Setup records use their own `schema: 1` and `kind: oracle-setup` namespace.
A `preparation-completed.json` summary must be durable before model dispatch;
the final session summary also includes any post-model pristine check.
An atomic `started.json` precedes each dispatch and lists actual argv, cwd and
its 120-second timeout; `completed.json` adds native status, timeout, elapsed
time, and original stdout/stderr SHA-256 and byte counts. Normal nonzero exits
are records, not automatic failures: existing capability checks still decide
whether the result is usable (the temporal probe expects exit 13). A timeout,
signal, missing normal status, spawn error or recording error fails preparation
before another subprocess can be dispatched. `harness-error.json` preserves an
available error without inventing a native completion. A retained started or
partial record alone proves neither dispatch nor cleanup. The existing owned
process supervisor still establishes quiescence before returning a normal
outcome; callers must preserve uncertainty after external cancellation.

The capability workspace is retained with these records. `DIR` must additionally
be outside the corpus and all physical directory aliases reached from it; this
is checked before setup creates files. No inherited environment is recorded.
Setup receipts are external evidence, not golden provenance or a public golden
schema; no capture is enabled by the default command.

**`dev capture`** (`--oracle ID --specs DIR --out DIR [--seed N] [--lncheck MODE] [--workers N] [--fp N] [--feature F]... NAME...`) runs an oracle exactly as the harness does on `DIR/NAME/NAME.tla` and `.cfg`, and records the raw stdout, stderr, exit code, JSON trace and SANY output. The fixtures under `tests/fixtures/tlc/` were made this way. `--seed N` replaces the harness's `-seed 0`; the randomization fixtures in `crates/dive-stdlib/tests/fixtures/random/` use it. `--lncheck MODE` (`final`, `default` or `off`) replaces `-lncheck final`, `--workers N` (one count) replaces `-workers 1`, and `--fp N` replaces `-fp 0`. `--feature F` gives the specs an Examples manifest feature, so `--feature 'action composition'` adds `-Dtlc2.tool.impl.Tool.cdot=true`. They record TLC's other modes as oracle facts for probes (the liveness probes in `crates/dive-liveness/tests/fixtures/tlc-probes/` use all three); goldens never use them. A capture with more than one worker has no JSON trace, because the trace pass runs only at one worker. Like golden generation, it records the clean run (no `-dumpTrace`); a counterexample's `.trace.json` comes from the cross-checked trace pass, run with the same `-seed`, whose command line is in `.trace.command`. Transcripts captured before issue #35 were made with `-dumpTrace json` on the deciding run: their `.command` ends in `-dumpTrace json $TMP/trace.json`, and their SANY lines show `_JsonTrace`, `TLCExt`, `Json` and `Integers` being parsed. `dev capture` does not retain the trace pass's stdout/stderr; use `golden generate --raw-invocations` when both passes need original-byte evidence.

## Oracle invocation: flag audit

Every flag the harness passes to TLC and SANY was checked against TLC's source at the pinned commit `94d0c50` (issue #35). "Inert" means the flag cannot change a verdict, a count, an error or a printed value; "chosen" means it fixes one of the settings a user may legitimately choose, and goldens are facts about that setting; "perturbs" means it changes TLC's semantics in a way users do not see.

| Flag | Effect at `94d0c50` | Status |
|---|---|---|
| `-tool` | `TLCGlobals.tool`: framed `@!@!@` messages, extra progress messages, runtime in ms, and no trace-exploration spec (`TLC.java` `generateTESpec`). Output format only | inert |
| `-workers 1` | TLC's default (`TLCGlobals.numWorkers = 1`); visible as `TLCGet("config").worker` | inert (the default) |
| `-fp 0` | Fixes the fingerprint polynomial (default: random). For safety checking it changes only which states could collide. **For liveness it also chooses the counterexample**: TLC orders each state's behaviour-graph edges by fingerprint hash bucket, so another polynomial can give a different lasso, length and set of named properties (6 of 21 models measured; ADR 0017 §10). Visible as `TLCGet("config").fingerprint` | chosen |
| `-seed 0` (`dev capture --seed N`: `N`) | Fixes the seed of `RandomElement`, `Randomization` and simulation (default: random); visible as `TLCGet("config").seed` | chosen |
| `-lncheck final` | `TLCGlobals.doLiveness()` is false, so liveness is checked once, after the search, not periodically. The verdict is the same; counts and the counterexample at a liveness violation can differ from the default, whose periodic checks are timed by the wall clock (probe `LvPeriodic` in `crates/dive-liveness/tests/fixtures/tlc-probes/`: 2,253,003 distinct states under `final`, 418,795, 167,402 and 162,021 in three `default` runs). Mirrors upstream Examples CI | chosen |
| `-cleanup`, `-metadir <tmp>` | Where TLC keeps its state files, and deleting them | inert |
| `-config <cfg>`, `-simulate num=N` | The model and the mode: the input itself | chosen (from the manifest) |
| `-dumpTrace json <file>` | `ParameterizedSpecObj` adds `_JsonTrace` as an extendee of the root module and a post-condition. `_JsonTrace` loads `TLC`, `TLCExt`, `Json`, `Sequences`, `FiniteSets`, `Naturals` and, through `TLCExt`, `Integers`, so their Java overrides are installed: `Integers`' then replace `Naturals`' on the shared definitions. The extra modules are also lexed, which can move later names in TLC's intern order (and so string order) | **perturbs**: only in the trace pass, never on the deciding run |
| `-Dtlc2.tool.impl.Tool.cdot=true` | Lets TLC evaluate `\cdot`. Passed only for manifests with the `action composition` feature, as upstream Examples CI does | chosen (from the manifest) |
| `-Xmx`, `-Xss8m`, `-XX:+UseParallelGC` | Heap, stack and garbage collector. Change only whether a run exhausts a resource, which is `resource-limit`, never a golden | inert |
| `-Duser.home=<cache>/home` | TLC reads only `~/.tlaplus/esc.txt` (statistics opt-out) and the REPL history from it | inert |
| `-Djava.io.tmpdir=<run dir>` | Where TLC unpacks the standard modules it parses | inert |
| `-Duser.language=en -Duser.country=US` | Number formatting in messages (`1,234`) | inert (format only) |
| `-Dfile.encoding=UTF-8` | Decoding of `.tla`/`.cfg` files and output; UTF-8 is TLA+'s encoding and the macOS and Linux default | inert for UTF-8 input |
| `-cp` (jar, CommunityModules, TLAPS library, Apalache modules) | Makes those modules resolvable. Standard modules and the spec's own directory always win (SANY's search order), and a Java override is installed only for a module the spec loads (`SpecProcessor`, `modSet.get(evaluation.module())`). Mirrors upstream Examples CI and the jar manifest's own `Class-Path`. The jar's location is visible as `TLCGet("config").install` (a machine path; read by no small-tier model) | chosen |
| Scrubbed `JAVA_TOOL_OPTIONS`, `_JAVA_OPTIONS`, `JDK_JAVA_OPTIONS`, `CLASSPATH` | Keeps host settings out | inert |
| SANY: `-error-codes` | Exit code 4 on semantic errors instead of 0; the text is unchanged | inert (exit code only) |

SANY is never given `-dumpTrace`, so SANY goldens were never perturbed. Only `-dumpTrace` perturbs, so a counterexample's JSON document comes from a separate *trace pass* (ADR 0003 amendment, issue #35):
- The model runs without `-dumpTrace`. That run decides the verdict, counts, errors and the counterexample's states, and its command is the golden's `provenance.command`.
- If it ends with a counterexample (one worker, an oracle with `-dumpTrace`), the model runs again with `-dumpTrace json`. Its command is `provenance.trace_command`.
- The JSON is used only if the trace pass reproduces the clean run's verdict, deciding code, violated property and trace exactly: the same length, ending, and every state's label and printed value, byte for byte. Otherwise the clean result stands as it is (verdict, counts, error, violated property, and the printed states in `steps`, which give the trace length), and only the document is marked unavailable: `trace.json_unavailable` records that the trace pass diverged and how (for example `trace state 2 differs`). JSON is never taken from a diverging run, nor rebuilt. Such a golden still supports every comparison the harness makes (verdict, counts, trace length and ending), but not trace-content ones, such as replaying the document through `dive-trace`. When the JSON is used, its state count is checked against the clean run's stream, as before.
- When TLC writes no JSON (assertion, evaluation and overflow errors), the clean run's printed states are kept.

## Cache layout

`$DIVE_CACHE_DIR` defaults to `~/.cache/dive`.

```text
sha256/<hex>/<file>      one directory per artifact digest (jars, JRE archive). Verified on every use;
                         a mismatch deletes the file and is a hard error.
jre/<hex>/               the JRE unpacked from archive <hex> (macOS: */Contents/Home/bin/java)
git/<id>/<commit>/       corpora and module libraries, `git init` + `fetch --depth 1 <sha>` + checkout;
                         completion marker in .git/, HEAD checked and the work tree must be clean
jar-modules/<hex>/       root-level .tla files of the classpath jar <hex>, for `--impl dive`
home/.tlaplus/esc.txt    TLC's user.home: NO_STATISTICS (opts out of TLC telemetry)
tmp/                     per-run scratch: an isolated copy of the spec directory (the run's working
                         directory), TLC -metadir, java.io.tmpdir, -dumpTrace output; removed after each run
locks/<entry>.lock       advisory locks, one per cache entry (never deleted)
```

**Concurrent runs.** Several harness processes may share one cache (issue #46).
- Every step that creates an entry (download, JRE unpacking, checkout, jar module extraction) holds that entry's exclusive advisory lock (`std::fs::File::lock`, `flock(2)`) and checks again under the lock whether another process finished it meanwhile. A process that has to wait says so on stderr.
- Entries appear only when complete: downloads are renamed into place after verification, unpacked trees are renamed from a staging directory, and a checkout's completion marker is written last. So readers take no lock: they see a complete entry or none, and verify it before every use. Per-run scratch directories are unique per process and run.
- `tests/cache_concurrency.rs` starts two `corpus fetch` processes together against an empty cache; one checks out, the other waits and finds it cached.
- The locks coordinate processes on one machine. Do not put the cache on a network file system.

**Parallel agents and worktrees.** Locking makes a shared cache safe, but a checkout is still shared state: anything that writes into it by hand (a stray `ln -s`, an editor) breaks every run that uses it, and the harness then refuses to run until the checkout is deleted and fetched again. Agents working in parallel should therefore each use their own cache, seeded from the shared one:

```sh
export DIVE_CACHE_DIR="$PWD/.dive-cache"                 # per worktree; or a scratch directory
rsync -a --exclude tmp ~/.cache/dive/ "$DIVE_CACHE_DIR/"   # optional: copy instead of downloading
dive-conformance oracle fetch && dive-conformance corpus fetch   # verifies the copy, fetches what is missing
```

A copied cache is verified exactly like a downloaded one: artifacts by SHA-256, checkouts by commit and a clean work tree.

**Isolation.** Each jar sits alone in its own directory. That matters because `tla2tools.jar`'s manifest `Class-Path` names `CommunityModules*.jar` *relative to the jar*; isolation guarantees that only the jars on `-cp` are used.

**Why shell out to `curl`, `git` and `tar`.**
- The harness needs HTTPS with redirects, GitHub authentication, shallow git fetches of a single commit, and `.tar.gz` extraction.
- Doing that in-process would pull in an HTTP stack, TLS, git and archive crates, a large dependency surface for a tool that already needs a JVM.
- `curl`, `git` and `tar` are present on every developer machine and CI runner.
- Integrity does not depend on them: every artifact is hashed in-process (`sha2`) before use, and checkouts are verified by commit SHA.
- The GitHub token is passed to curl on stdin (`--config -`), never on a command line.
- `unzip` reads the jar manifest's `X-Git-Revision` (if it is missing, the revision is simply not recorded), and, for `--impl dive`, extracts the `.tla` files of the verified classpath jars into `jar-modules/<sha256>/` (required there).

**The re-hosted oracle asset.** The pinned `tla2tools.jar` is a release asset of this repository. Every asset is first fetched from its URL without a token. If that fails and the URL names a GitHub release asset, the harness falls back to the GitHub REST API: the release by tag, then the asset URL with `Accept: application/octet-stream`, with a token from `GH_TOKEN`, `GITHUB_TOKEN` or `gh auth token`. The fallback is needed only while the repository is private, because the browser download URL answers 404 for private assets even with a token.

## Adding an implementation adapter

Implement `implementation::Implementation`:

```rust
pub trait Implementation: Sync {
    fn name(&self) -> String;
    fn is_oracle(&self) -> bool;          // false for anything that is not TLC
    fn check_model(&self, req: &ModelRequest<'_>) -> Result<ImplResult<OracleResult>>;
    fn parse_module(&self, root: &Path, module: &str, timeout: Duration) -> Result<ImplResult<SanyResult>>;
    // Optional: parse-only mode (`--sany-phase parse`), a wholesale refusal of models,
    // profile, options, provenance; see the rustdoc.
}
```

**Rules for adapters:**
- A violation can `agree` only if the harness validates the counterexample (charter rule 2). The default `trace_validated` says no; `dive` overrides it with the in-process validator (`src/trace_check.rs`). An adapter whose tool emits counterexamples in dive's trace format can do the same.
- Return `ImplResult::Unsupported(reason)` for anything the tool does not implement exactly. Never return a partial or approximate result.
- Results use the oracle vocabulary. `verdict` and `error_class` must be the TLC `EC` name of the equivalent outcome; counts, depth and trace length go in their fields.

**Wiring.** Add the new variant to `--impl` in `src/main.rs`.

**Profiles, options and expected divergences (ADR 0010).**
- **Options.** `--impl-option NAME[=VALUE]` runs `dive` or an adapter under an option, and is recorded in the report. An option the implementation does not declare result-preserving (`result_preserving_options`) turns every would-be `agree` into `divergence-mode`. dive maps `symmetry-liveness=<mode>` to `--symmetry-liveness` and `store=exact|fingerprint` to `--store`. `seed`, `workers` and `store` are result-preserving.
- **Profiles.** `dive` also has a profile, set with `--impl-profile dive|tlc|tlc+overrides=<features>`, or `DIVE_COMPAT=tlc`. Every report records it, plus `strict_tier`, which is true only for pure `tlc`.
- **Engines.** `dive` also has an engine (ADR 0117): `--impl dive:core`. `dive:reference` was retired with the reference engine (#791) and is a usage error. Plain `--impl dive` takes `DIVE_ENGINE`, or else Core (ADR 0153). The harness removes `DIVE_ENGINE` from the child's environment and always passes `--engine`. The report records the request in `settings.impl_engine`. Each run row records the engine that ran (`engine`). Reports from before the cutover may also carry an `auto` run's refusal (`engine_fallback`), which the harness still reads; `summary.by_engine` counts models by `core`, `reference`, `static-fallback`, `dynamic-fallback` and `none` (the run named no engine: refused, or ended before the search). Reference baselines can compare other engines by observable results; a Core-only baseline requires Core. A baseline records a non-reference engine (`engine`), and a run against a baseline of another engine says so in its first line.
- **Core-only baselines.** A baseline recorded with `engine: core` requires an explicit Core run and rejects fallback, other checking runtimes and unconfirmed counterexamples. Named refusals and frontend parse/semantic/configuration errors can precede runtime startup. Existing agreeing fixture identities and registered-divergence results must remain covered; gaining another model cannot compensate for losing one. The scheduled/manual small-tier CI job and `cargo xtask gate --full` use `small-core-dive-w1.json` and `small-core-tlc-w1.json`. Required PR CI runs the Core smoke and Core golden gates; it does not repeat the whole small tier.
- **Expected divergences.** Under the `dive` profile, a disagreement listed in `conformance/expected-divergences.toml` is reported as `expected-divergence`. It is never `agree`. The registry never applies under `tlc` or `tlc+overrides`.
  - An entry names the model or module glob, dive's verdict, the `fields` that may differ, and optional exact `exit_code_pair` or `generated_count_pair` values. Field globs cannot cover exits or a generated count that already has an exact pair. Every declared exit pair must appear exactly in the diffs. A generated pair requires that difference at one worker; when native multi-worker policy omits generated counts, the worker-aware matcher instead requires the exact pair in the complete observed results. Every other differing field must be listed, so counts never hide a different verdict or property. Trace validation can never be excused.
  - Registered today: MissionariesAndCannibals and ACP_NB_WRONG_TLC (D-010), P1Overflow (D-004), P2Choose_str (D-006), and 14 measured default numeric-exit differences (D-023). `conformance.yml` runs the default profile on the tier's models, so these must stay `expected-divergence` and anything else unregistered fails.

**`dive` (`--impl dive`)** runs the `dive` binary as a black box: `target/release/dive` by default, or `--dive-bin PATH`. Build it first with `cargo build --release -p dive-cli`. The report records the binary's path, SHA-256 and version, and its module search path.
- **Modules, full phase (default).** Each module runs `dive analyze --output json --module-path ... <file>` from the module's own directory in the corpus checkout. `accepted` decides acceptance. A rejection is `parse-error` when any error is a syntax (`DIVE-Snnnn`) or module-resolution (`DIVE-E00nn`) diagnostic, which SANY reports in its parse phase, and `semantic-error` otherwise. The exception is `DIVE-E0005` (a module defined twice, SANY 4223), which SANY reports as a semantic error. `DIVE-E0001` diagnostics give the unresolved modules. The comparison with the golden is the usual one: acceptance, verdict and unresolved modules.
- **Modules, parse phase (`--sany-phase parse`).** Each module runs `dive parse --output json <file>`, which parses that one file. It is compared with SANY's parse-phase verdict for the same file (`accept`, `name-mismatch` or `reject`), which is derived from the *existing* full-SANY goldens, so no extra golden is needed. SANY parses the root file first and aborts at the first parse-phase failure, and the golden's `error_text` keeps SANY's attribution line from the first error marker on. So:
  - `accept` and `semantic-error` goldens mean the file parsed;
  - a `parse-error` is the file's own when SANY printed `Could not parse module <stem> from file`, a `name-mismatch` when it printed `File name '<stem>' does not match`, and a dependency's failure (so the file parsed) when it names another module or reports a circular dependency, which SANY detects only after every file of the cycle has parsed;
  - anything else is `oracle-error`, never a guess.

  `--impl tlc --sany-phase parse` derives the same verdict from a fresh SANY run, as a self-check.
- **Module search path.** It is derived from `conformance/oracles.toml` exactly as the golden oracle's classpath is. SANY looks in the root's directory, then in `tla2sany/StandardModules` inside `tla2tools.jar`, then in the rest of the classpath in order. So every later classpath entry becomes a `--module-path` (searched after dive's standard modules), in order:
  - the root-level `.tla` files of each classpath jar, extracted once into `$DIVE_CACHE_DIR/jar-modules/<sha256>/`;
  - each `[[module_library]]` directory: Apalache's `src/tla`, then the TLAPS `library/`.

  Standard modules therefore win over the TLAPS library's own `Bags`, `FiniteSets` and `RealTime`, as they do for SANY (ADR 0011).
- **Models.** Each exhaustive model runs `dive check --output json --config <cfg> --workers N --seed 0 --fp 0 --module-path ... <module>` from the module's directory (TLC's goldens use `-fp 0 -seed 0`). Simulation models are `unsupported`. Models with the `action composition` feature also get `--action-composition tlc` under the `tlc` profiles, as TLC gets `-Dtlc2.tool.impl.Tool.cdot=true`; the default profile composes actions without it (D-022). The report is mapped to the oracle vocabulary (`dive::check_result`):
  - the TLC message code dive reports (`tlc.code`) gives the verdict (`tlc::codes::verdict_for_code`) and the error class; under `--impl-profile tlc` the exit code must be TLC's for that code;
  - the statistics are taken only where TLC prints them (`reported_by_tlc`; none for a stop while generating initial states);
  - the counterexample's length and printed states (label and text) form the trace, ending `finite`;
  - `unsupported` and `resource-limit` reports (exit 3) are `unsupported`, naming the feature; an error whose TLC class dive does not claim yet (`tlc.verified: false`) is `unsupported` too, with dive's own finding in the detail;
  - a rejected spec is `config-error`, `parse-error` or `semantic-error` by its diagnostics' codes (as for modules), with TLC's class (`TLC_PARSING_FAILED` for SANY's rejections) only where dive claims it; under `--impl-profile tlc` the exit code must be TLC's;
  - `internal-error`, or a report that contradicts itself, is `unclassified`, which is a `disagree`.

  dive's counterexample document (`counterexample.trace`) is validated by the harness in-process with `dive-trace`; a violation agrees only if it validates.
- **Robustness.** A run whose exit code contradicts `accepted` (0 if accepted, 1 if rejected), a crash, missing or unparsable JSON, or a `schema_version` other than 1 is `unclassified`, which is a `disagree`. Exit 3 (`dive` refusing) is `unsupported`. Every run must leave the corpus checkout unmodified.
- **Profiles.** `--impl-profile` and `DIVE_COMPAT=tlc` are recorded. `dive parse` and `dive analyze` have no profile-dependent behaviour yet. The harness passes `DIVE_COMPAT=tlc` to `dive` under the pure `tlc` profile, and removes it otherwise. A result-changing `--impl-option` makes module outcomes `divergence-mode`, as it does for models.

**External tools (M2)** are described by a TOML file, loaded with `--impl adapter:<file>`:

```toml
name = "tla2"
source = "joweeba/tla2 @ 5e828d3 (Apache-2.0)"     # optional provenance
# Placeholders: {module} {config} {workers} {corpus} {library} {scratch}
check = ["tla2", "check", "{module}", "--config", "{config}", "--workers", "{workers}"]
parse = ["tla2", "parse", "{module}"]              # optional
library = "env"                                    # optional: flag | env | classpath | copy-beside-spec
max_workers = 1                                    # optional: a single-threaded tool
refused_exit_codes = [2]                           # optional: refusals and errors without a verdict
[env]                                              # optional, values are templates
TLA_PATH = "{library}"
[exit_codes]
0 = "success"
12 = "safety-violation"
[[rules]]                                          # optional; the first match wins, before exit_codes
pattern = "^Error: Invariant"                      # a line regex (stream = stdout|stderr|any), or
verdict = "safety-violation"                       #   json = "/pointer" with equals = "value";
                                                   #   and verdict = ... or refused = true
[counts]                                           # optional: distinct_states, states_generated, depth
distinct_states = { pattern = '([0-9,]+) distinct states found' }   # or { json = "/pointer" }
```

A result that no rule or exit code explains is unclassified, never a guess. Loading validates placeholders, exit codes, rules, counts and the library mode.

`conformance/adapters/` holds the invocations measured in `docs/internal/survey/competitive.md`: `tla-mc.toml`, `tla-rs.toml`, `tlaplusplus.toml`, `tlago.toml` and `tla2.toml`. A unit test loads every one. They are not run in CI; they exist for reproducible re-measurement.

The descriptions are parsed and validated today, but adapters are **not executed yet**: every request answers `unsupported`. Executing them, and mapping their output into `OracleResult`, is M2 work.

## Regenerating goldens

Goldens are generated, never edited (charter rule 6; ADR 0003).
- Overwriting an existing golden needs `--reason "<why>"`. The reason is recorded in provenance, and the command says whether the payload changed.
- **Regenerate when:**
  - an oracle pin or corpus pin moves;
  - the golden format changes (bump `GOLDEN_SCHEMA`);
  - the harness's extraction changes, as when nested `-tool` messages became parseable.
- Review the payload diff in the PR. A payload change with an unchanged oracle and corpus is a harness change and needs explaining.
- **Never regenerate to make a disagreement go away.** `run` refuses to compare against goldens whose corpus commit or jar SHA-256 no longer match the pins.

**Committed goldens.** `conformance/golden/` holds:
- for `tlc-1.7.4`, an 11-model smoke set with SANY goldens for its modules, as regression anchors;
- for the gating oracle:
  - 141 Examples model goldens (the small tier and more) and SANY goldens for all 424 Examples modules;
  - worker-specific goldens (`<model>.workers-N.json`, ADR 0004): `MCDieHardest` at 3, 4 and 8 workers. They are generated with `golden generate --workers N --reason ...`, only for worker-introspecting models, always with a reason, and never replace a single-worker golden;
  - SANY goldens for `tlaplus-tests` (807 of 808 modules; one SANY crash is excluded in `corpora.toml`);
  - SANY goldens for `community-modules` (78 modules);
  - for the `probes` corpus, 14 model goldens and 12 SANY goldens.

## Oracle behaviour notes

Each note has a minimal spec under `tests/fixtures/specs/` and captured transcripts for both oracles under `tests/fixtures/tlc/<oracle>/`.

| Behaviour | Spec |
|---|---|
| Integer overflow is an error (`TLC_MODULE_OVERFLOW`, 2178) and exits **255**, like a crash | `Overflow` |
| An out-of-domain function application in `Next` is reported as `GENERAL` (1000, "Attempted to apply…") followed by `TLC_FINGERPRINT_EXCEPTION` (2147), exit 75. Inferred, not verified in TLC's source: the successor's value is evaluated lazily, when it is fingerprinted | `EvalError` |
| Action-property violations exit 13, like liveness, but report `TLC_ACTION_PROPERTY_VIOLATED_BEHAVIOR` (2112) | `ActionProperty` |
| Parse and semantic errors share code 3002 and exit 150; only SANY's untagged text tells them apart | `ParseError`, `SemanticError` |
| An invariant violated by an initial state prints the state inside the violation message, not as a state message | `InitInvariant` |
| `-dumpTrace json` writes a file only for invariant, deadlock, liveness and action-property violations, not for `Assert`, evaluation or overflow errors. Stuttering and lassos appear as an extra action | all |
| v1.7.4 omits `TLC_SEARCH_DEPTH` after assertion, evaluation, overflow and liveness errors | `AssertFailure`, `Liveness` |
| v1.7.4 prints "Temporal properties were violated." without naming the property | `Liveness` |
| v1.7.4 does not know `ALIAS` in `.cfg`. It nests `CFG_EXPECTED_SYMBOL` (5006, class 0) *inside* `GENERAL` (1000) and exits 255 | `CfgAlias` |
| v1.7.4 cannot run with any CommunityModules release tried; specs that use them fail to parse | `UsesCommunityModules` |
| SANY exits 0 on semantic errors unless `-error-codes` is given (`94d0c50` then exits 4); v1.7.4 rejects `-error-codes` | `SemanticError` |
| TLC uses a random fingerprint polynomial per run unless `-fp` is given | all |
| With four workers, DieHard stops at 16 distinct / 84 generated / depth 9 (one worker: 14 / 73 / 7). The counterexample length stays 7, but the reported depth exceeds it | Examples `DieHard` |
| With four workers, `MCDistributedReplicatedLog` reports "Temporal properties AllExtending and InSync were violated"; with one worker, only `InSync`. This was reproduced twice. The violated property is therefore compared at one worker only, and its variance is reported under worker invariance | Examples `FiniteMonotonic/MCDistributedReplicatedLog` |
| `-generate` exists in `94d0c50` but is not listed in its usage text | n/a |
| `RandomElement`/`Randomization` use TLC's seed in exhaustive search too. Without `-seed`, Examples `SpanTreeRandom` gave 40460, 1878, 240806, 65396 and 106664 distinct states; the harness always passes `-seed 0` | Examples `SpanningTree/SpanTreeRandom` |
| At `-Xmx64m`, `94d0c50` reports `SYSTEM_OUT_OF_MEMORY_TOO_MANY_INIT` (1002, exit 255). v1.7.4 instead refuses a set of more than 1,000,000 elements as `TLC_INITIAL_STATE` (2102), and exit 75 is explained only by context code 2103, so it is unclassified | `Oom` |
| Deep recursion reports `GENERAL` ("StackOverflowError") then 2103, exit 75. This is a `resource-limit`, and `-Xss8m` is pinned | `Stack` |
| `PrintT("@!@!@ENDMSG 2185 @!@!@")` prints the marker text quoted and mid-line. Markers count only in TLC's own positions | `PrintMarker` |
| A state-predicate `PROPERTY` violated by an initial state is `TLC_PROPERTY_VIOLATED_INITIAL` (2108), exit 13, with the state inside the message. It is classified as `safety-violation` | `InitProperty` |
| v1.7.4 consumes `-simulate num=N` only when another argument follows it (`TLC.java`: `(index + 1) < args.length`). As the last argument, `num=N` is rejected (`WRONG_COMMANDLINE_PARAMS_TLC`, exit 1), so the harness puts `-simulate` early | Examples `ewd840/EWD840_anim` |
| v1.7.4's SANY prints a `Semantic errors:` header above a warnings-only list (`*** Warnings: 1`, "Multiple declarations or definitions for symbol Foo") and accepts the module. Rejection therefore requires `*** Errors:` or a parse/abort marker | `SanyWarning`; Examples `AP*.tla` |
| v1.7.4 does not know `ALIAS` in `.cfg`, and it fails in two different ways: as a nested config error in `GENERAL` (exit 255), or by reading `ALIAS` as a property name ("The property ALIAS ... is not defined", `TLC_CONFIG_SPECIFIED_NOT_DEFINED`, exit 151) | `CfgAlias`; Examples `braf`, `ewd426/TokenRing` |
| For constant-only models (no variables; only `ASSUME`s are checked, 0 distinct states), `94d0c50` reports search depth 0 and v1.7.4 reports depth 1 | Examples `SimpleMath`, `Stones`, `TransitiveClosure` |
| `ASSUME TLCGet("config").worker = 1` makes the verdict depend on `-workers` | Examples `DieHard/MCDieHardest` |
| `-dumpTrace json` makes TLC extend the root module with `_JsonTrace`, which loads `TLCExt`, `Json` and `Integers`, so `Integers`' Java overrides replace `Naturals`' even in a spec that extends only `Naturals`: `"a" >= 1` reports `>=` (not `Naturals.GEQ`'s `>`), and `-2147483648 \div -1` overflows instead of wrapping. Goldens and `dev capture` therefore take everything from a run without it, and `-dumpTrace` runs only in the cross-checked trace pass (issue #35; see "Oracle invocation: flag audit") | `crates/dive-eval/tests/fixtures/specs/ErGeqNaturals`, `ErPlusNonIntNaturals`; `crates/dive-value/tests/fixtures/specs/RvNatDiv` |
| In a spec that loads `Naturals` but not `Integers`, `\div` is `Naturals.Divide`: Java's truncating division, decremented only when the quotient is negative and inexact. A quotient that truncates to 0 is not rounded down, so `(0 - 1) \div 2` and `1 \div (0 - 2)` are `0` (the floored answer, and `Integers.Divide`'s, is `-1`), while `(0 - 7) \div 2` is `-4`. `Naturals.tla` defines `-` and `\div` only on `Nat`, so this is not a wrong answer, but dive must reproduce it. This was hidden while fixtures were captured with `-dumpTrace json` | `crates/dive-value/tests/fixtures/specs/RvNatDiv` |
| Animation specs write SVG files into their working directory, hence the isolated per-run copies | Examples `*_anim` |
| Simulation is not reproducible even with `-seed 0` and one worker. `EWD840_anim` generated 267,302, 267,196, 271,147 and 268,375 states in four runs, with the same verdict and trace length. Simulation counts are therefore kept out of the golden payload (`provenance.uncompared_counts`) | Examples `ewd840/EWD840_anim` |

## Limitations

- Only the `examples-manifest` and `modules-only` layouts are enumerated. `tbd` corpora (ValidationTestSuite) report "not yet supported". The tlaplus repo tests (`tlaplus-tests`) and CommunityModules are `modules-only`: SANY goldens, no models.
- Unsupported model features are reported by name as `unsupported`; see the
  current [temporal checking](../../docs/user/temporal.md) and
  [TLC differences](../../docs/tlc-differences.md) guides.
- Examples submodules (CCF, azure-cosmos-tla, BlockingQueue, microwave) are never fetched. They are listed in reports as unfetched.
- Simulation results are compared by verdict only; the simulation is pinned with `-seed 0`.
- External adapters are parsed but not executed.
- The harness targets macOS and Linux. It retains native process-group leaders
  unreaped through group cleanup (ADR 0112), using bounded `/bin/ps` inspection.
  Cancellation blocks new work and serializes signalling with terminal reap.
  Failed ownership/cleanup is a harness error and requires external supervision;
  it is never a model result. Child execution on other hosts is explicitly
  refused before launch; no successful direct-child fallback is substituted.

`run --sany-only` compares modules without running model checking. Use it
with `--sany-phase parse` for parser-only conformance; `--no-sany` selects
model checking alone. Combining these two selection flags is a usage error.

## Final/short temporal study

`study final` observes schema-2/3 final/short runs separately from strict
conformance. It compares final truth and counts and validates periodic witnesses
independently. See the study guide
and `scripts/final-temporal-study.sh conformance/studies/lm1.txt`. Named refusals
and all selected model/worker pairs remain visible; these observations never
count as strict `agree` or alter the conformance gate.
Time and memory budget stops are recorded as `timeout` findings, with the
resource retained in the document; neither is an unsupported feature or a
successful observation.

## Deep frontend campaigns

The deep workflow has a separate deterministic mutation/SANY-case job. Its
runner uses this harness for full SANY goldens and `dive analyze`, then a
pinned direct grammar driver for `dive parse`. Every selected root is
accounted for; oracle crashes, malformed reports and missing facts are
findings. Inputs and reports remain in an external disposable directory,
and artifacts retain the complete campaign. See `docs/internal/frontend-campaign.md`.

### Exact process-exit differences

Model comparison includes the physical numeric process exit, including absent
codes, under every worker/count policy. The default registry may declare
`exit_code_pair = { expected = 12, actual = 10 }` on an exact model path.
Its codes must be distinct integers in 0..255; the exact pair must appear in the
diffs and every other differing field must match the entry's existing `fields`.
Field globs cannot authorize exits. The expected native verdict and independent
trace validation remain required. Matching pairs are ExpectedDivergence, never
Agree; TLC and override profiles never apply the registry. See
[ADR 0096](../../docs/decisions/0096-truthful-default-numeric-exit-conformance.md)
for the correction of historical omitted-exit credits.

### Exact generated-count differences

The default-only registry can declare
`generated_count_pair = { expected = 1350001, actual = 900001 }` for a
specific model. Both values are unsigned integers and must differ. The exact
pair must appear in the comparison at one worker. Native multi-worker policy
omits generated-count differences; the pipeline then requires the exact pair
from both complete results and comparable-count worker context instead.
Missing, reversed, nearby or noninteger counts do not match. The legacy
source-free matcher still requires the pair in the differences. A field glob cannot also authorize
`counts.states_generated` when this pair is present. Other differences still
need their own listed fields or exact exit pair; independent trace validation
cannot be excused.

D-022 uses this form with `fields = []` for
`specifications/FiniteMonotonic/MCCRDT.cfg` (1,350,001/900,001) and
`MCReplicatedLog.cfg` (17,425/11,617). The default counts composed successors
without TLC's composition intermediates. These entries report
`expected-divergence`, never `agree`. TLC, TLC-with-overrides and implementations
without a dive profile do not use the registry. Goldens, native result values
and comparison policies remain unchanged. See
[the documented composition counts](../../docs/tlc-differences.md#d-022-action-composition-works-by-default-with-its-tla-meaning).
