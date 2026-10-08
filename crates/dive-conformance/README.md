# dive-conformance

The differential conformance harness. It:
- fetches and verifies the pinned TLC/SANY oracles, the Java runtime and the corpora;
- generates golden results from TLC and SANY;
- runs an implementation against those goldens: TLC itself, `dive` (its front end today: `dive analyze` and `dive parse` against the SANY goldens), and external tools later;
- writes per-model reports.

Read with ADRs [0002](../../docs/decisions/0002-oracle-pinning.md) (pins), [0003](../../docs/decisions/0003-golden-format-and-provenance.md) (goldens, invocation, classification), [0004](../../docs/decisions/0004-worker-count-and-depth-semantics.md) (workers and depth) and [0005](../../docs/decisions/0005-reuse-examples-manifests.md) (Examples manifests).

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
- An unknown exit code or message code, or a malformed stream, is `unclassified`, with the raw data kept.
- JVM resource exhaustion (out of memory, stack overflow) is `resource-limit`: never an oracle fact, never a golden.
- Unexpected stderr, or a success without statistics, is `oracle-error`.
- An implementation that cannot do something must answer `unsupported`.

**Each model gets exactly one outcome:**
- `agree`;
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

**Known upstream changes.** `conformance/oracle-changes.toml` is the machine-readable side of `docs/survey/oracle-history.md`. When a TLC oracle runs against another oracle's goldens, a difference that registered changes fully explain is reported as `known-upstream-change`, with the change ids and upstream commits in the detail. A difference that no entry explains stays `disagree`.

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

**Comparison rules** (ADR 0004 and its amendment):
- Every implementation is compared with the single-worker golden on:
  - the verdict class and the error class (the `EC` name of the deciding message);
  - the violated property, when the golden names one;
  - distinct, generated and queued counts, and depth;
  - the counterexample's length and how it ends (finite, lasso, stuttering).
- The counterexample's content is never compared. An implementation that is not TLC must report it validated (`Implementation::trace_validated`), or it cannot agree on a violation.
- Only a TLC oracle gets multi-worker leniency. With more than one worker, its counts are compared only after a complete search, and depth, queue, counterexample and property are not compared.
- For TLC, a model's outcome is its single-worker run. Multi-worker runs appear only in the worker-invariance section.
- For anything else, every worker count must match, and any variance is `disagree`.
- v1.7.4 may omit violated temporal-property names, because its capability probe shows it never prints them.
- Models whose module reads `TLCGet("config")` are flagged `worker_introspecting`.
- Simulation models compare verdict-level fields only.
- Every selected model and module gets exactly one outcome; one without a golden is `oracle-error`. The exception is an implementation that refuses every model (`dive` until `dive check` exists): its models are `unsupported`, and the detail notes a missing golden. The summary asserts that the totals equal the selection.

## Corpora

`conformance/corpora.toml` pins each corpus and its `layout`:
- `examples-manifest` (tlaplus/Examples): modules and models from the per-spec `manifest.json` files (ADR 0005).
- `modules-only`: every `.tla` file is a module for SANY goldens, and there are no models. Hidden directories are included (CommunityModules keeps smoke tests in `.smoke/`); `.git` is not.
  - `include = [globs]` narrows the modules (`tlaplus-tests`: `tlatools/org.lamport.tlatools/test/**` and `test-model/**`, 808 files).
  - `[[corpus.exclude]]` entries (`path`, `reason`) are modules that are enumerated but never run, for example because they crash SANY. They are reported as `out-of-scope` with the reason, never dropped. An exclusion that names no enumerated module is a configuration error.
  - A job copies only the files of the module's own directory into its scratch directory, since SANY never looks into subdirectories. An Examples job copies the whole spec directory.
- `tbd`: not yet enumerated.

## Commands

```sh
cargo run -p dive-conformance --release -- <command>      # or target/release/dive-conformance

oracle fetch [--oracle ID]       # download + verify gating oracles (or ID), classpath jars, the host JRE, module libraries
oracle verify [--oracle ID]      # re-verify the cache and print the gating oracles' (or ID's) probed capabilities
corpus fetch [--corpus ID]      # git checkout of pinned corpora; submodules are recorded, not fetched
corpus list  [--tier small|deep|all] [--filter GLOB]
golden generate [--oracle ID] [--tier T] [--filter GLOB]... [--reason TEXT] [--no-sany|--sany-only]
run --impl tlc|tlc:<oracle-id>|dive|adapter:<file.toml>
    [--corpus ID]... [--golden-oracle ID] [--workers 1,4] [--crosscheck-manifest] [--tier T] [--filter GLOB]...
    [--sany-phase full|parse] [--dive-bin PATH] [--impl-profile P] [--impl-option N[=V]]...
```

`run` takes several `--corpus`, into one report and one summary (default: `examples`).

**Common options:**
- `--jobs N` (default `min(4, cpus/2)`): parallel oracle runs.
- `--xmx SIZE` (default `3g`): JVM heap per run. Four 3 GB JVMs fit an 8-core / 24 GB machine with room to spare.
- `--timeout SECS`, or the derived default:
  - 20 × the manifest runtime + 120 s, at least 300 s, capped by `--timeout-cap` (default 3600); unknown runtimes get the cap;
  - in `run`, also at least 4 × the golden's recorded elapsed time + 60 s, even above the cap. Manifest runtimes were measured with several workers.

  On timeout the whole process group is killed with `SIGKILL`. Ctrl-C (`SIGINT`) and `SIGTERM` kill every live oracle process group before the harness exits.
- `--java PATH`: opts into a system Java. It is recorded in provenance as `source: system`, with the executable's SHA-256.
- `--repo-root DIR` and `--cache-dir DIR`.

**Outputs:**
- `run` writes `conformance/reports/<run-id>.json` (schema version 1) and `docs/conformance.md`. Use `--no-summary` to skip the summary, or `--summary PATH` to put it elsewhere.
- Exit status: 0 for success; 1 for findings (unwritten goldens, or `disagree`, `oracle-error` or `timeout` outcomes, except in runs involving a reference oracle); 2 for a usage error, including an empty selection or stray arguments; 3 for a harness error.

**Examples:**

```sh
dive-conformance golden generate --tier small                     # all small models + SANY goldens
dive-conformance run --impl tlc --crosscheck-manifest              # self-consistency + manifest drift
dive-conformance run --impl tlc:tlc-1.7.4 --no-summary             # cross-oracle diff vs default goldens (reference: never gates)
dive-conformance run --impl tlc --workers 1,4 --filter 'specifications/ewd840/*'
cargo build --release -p dive-cli                                  # then, for dive's front end:
dive-conformance run --impl dive --tier all --corpus examples --corpus tlaplus-tests --corpus community-modules
dive-conformance run --impl dive --sany-phase parse --corpus tlaplus-tests   # dive parse vs SANY's parse phase
```

**`dev capture`** (`--oracle ID --specs DIR --out DIR NAME...`) runs an oracle exactly as the harness does on `DIR/NAME/NAME.tla` and `.cfg`, and records the raw stdout, stderr, exit code, JSON trace and SANY output. The fixtures under `tests/fixtures/tlc/` were made this way.

## Cache layout

`$DIVE_CACHE_DIR` defaults to `~/.cache/dive`.

```text
sha256/<hex>/<file>      one directory per artifact digest (jars, JRE archive). Verified on every use;
                         a mismatch deletes the file and is a hard error.
jre/<hex>/               the JRE unpacked from archive <hex> (macOS: */Contents/Home/bin/java)
git/<id>/<commit>/       corpora and module libraries, `git init` + `fetch --depth 1 <sha>` + checkout;
                         completion marker in .git/, HEAD checked and the work tree must be clean
home/.tlaplus/esc.txt    TLC's user.home: NO_STATISTICS (opts out of TLC telemetry)
tmp/                     per-run scratch: an isolated copy of the spec directory (the run's working
                         directory), TLC -metadir, java.io.tmpdir, -dumpTrace output; removed after each run
```

**Isolation.** Each jar sits alone in its own directory. That matters because `tla2tools.jar`'s manifest `Class-Path` names `CommunityModules*.jar` *relative to the jar*; isolation guarantees that only the jars on `-cp` are used.

**Why shell out to `curl`, `git` and `tar`.**
- The harness needs HTTPS with redirects, GitHub authentication, shallow git fetches of a single commit, and `.tar.gz` extraction.
- Doing that in-process would pull in an HTTP stack, TLS, git and archive crates, a large dependency surface for a tool that already needs a JVM.
- `curl`, `git` and `tar` are present on every developer machine and CI runner.
- Integrity does not depend on them: every artifact is hashed in-process (`sha2`) before use, and checkouts are verified by commit SHA.
- The GitHub token is passed to curl on stdin (`--config -`), never on a command line.
- `unzip` reads the jar manifest's `X-Git-Revision` (if it is missing, the revision is simply not recorded), and, for `--impl dive`, extracts the `.tla` files of the verified classpath jars into `jar-modules/<sha256>/` (required there).

**The private oracle asset.** It is fetched through the GitHub REST API: the release by tag, then the asset URL with `Accept: application/octet-stream`. The token comes from `GH_TOKEN`, `GITHUB_TOKEN` or `gh auth token`. The browser download URL answers 404 for private assets even with a token. Public assets are fetched from their URL without a token.

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
- Override `trace_validated` once counterexamples are replayed by an independent validator (charter rule 2). Until then a violation verdict cannot `agree`.
- Return `ImplResult::Unsupported(reason)` for anything the tool does not implement exactly. Never return a partial or approximate result.
- Results use the oracle vocabulary. `verdict` and `error_class` must be the TLC `EC` name of the equivalent outcome; counts, depth and trace length go in their fields.

**Wiring.** Add the new variant to `--impl` in `src/main.rs`.

**Profiles, options and expected divergences (ADR 0010).**
- **Options.** `--impl-option NAME[=VALUE]` runs `dive` or an adapter under an option, and is recorded in the report. An option the implementation does not declare result-preserving (`result_preserving_options`) turns every would-be `agree` into `divergence-mode`.
- **Profiles.** `dive` also has a profile, set with `--impl-profile dive|tlc|tlc+overrides=<features>`, or `DIVE_COMPAT=tlc`. Every report records it, plus `strict_tier`, which is true only for pure `tlc`.
- **Expected divergences.** Under the `dive` profile, a disagreement listed in `conformance/expected-divergences.toml` is reported as `expected-divergence`. It is never `agree`. The registry never applies under `tlc` or `tlc+overrides`.

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
- **Models.** `dive check` does not exist yet, so every model is `unsupported` ("`dive check` is not implemented yet"), with or without a golden. Nothing passes silently, and `unsupported` is not a finding.
- **Robustness.** A run whose exit code contradicts `accepted` (0 if accepted, 1 if rejected), a crash, missing or unparsable JSON, or a `schema_version` other than 1 is `unclassified`, which is a `disagree`. Exit 3 (`dive` refusing) is `unsupported`. Every run must leave the corpus checkout unmodified.
- **Profiles.** `--impl-profile` and `DIVE_COMPAT=tlc` are recorded. `dive parse` and `dive analyze` have no profile-dependent behaviour yet. The harness passes `DIVE_COMPAT=tlc` to `dive` under the pure `tlc` profile, and removes it otherwise. A result-changing `--impl-option` makes module outcomes `divergence-mode`, as it does for models.

**External tools (M2)** are described by a TOML file, loaded with `--impl adapter:<file>`:

```toml
name = "tla2"
check = ["tla2", "check", "{module}", "--config", "{config}", "--workers", "{workers}"]
parse = ["tla2", "parse", "{module}"]     # optional
[exit_codes]
0 = "success"
12 = "safety-violation"
```

The description is parsed and validated today, but adapters are **not executed yet**: every request answers `unsupported`. Executing them, and mapping their output into `OracleResult`, is M2 work.

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
  - SANY goldens for `tlaplus-tests` (807 of 808 modules; one SANY crash is excluded in `corpora.toml`);
  - SANY goldens for `community-modules` (78 modules).

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
| Animation specs write SVG files into their working directory, hence the isolated per-run copies | Examples `*_anim` |
| Simulation is not reproducible even with `-seed 0` and one worker. `EWD840_anim` generated 267,302, 267,196, 271,147 and 268,375 states in four runs, with the same verdict and trace length. Simulation counts are therefore kept out of the golden payload (`provenance.uncompared_counts`) | Examples `ewd840/EWD840_anim` |

## Limitations

- Only the `examples-manifest` and `modules-only` layouts are enumerated. `tbd` corpora (ValidationTestSuite) report "not yet supported". The tlaplus repo tests (`tlaplus-tests`) and CommunityModules are `modules-only`: SANY goldens, no models.
- `dive check` does not exist, so `--impl dive` compares modules only; models are `unsupported`.
- Examples submodules (CCF, azure-cosmos-tla, BlockingQueue, microwave) are never fetched. They are listed in reports as unfetched.
- Simulation results are compared by verdict only; the simulation is pinned with `-seed 0`.
- External adapters are parsed but not executed.
- The harness targets macOS and Linux. Process-group killing uses Unix process groups.
