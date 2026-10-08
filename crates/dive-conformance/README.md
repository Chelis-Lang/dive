# dive-conformance

The differential conformance harness. It:
- fetches and verifies the pinned TLC/SANY oracles, the Java runtime and the corpora;
- generates golden results from TLC and SANY;
- runs an implementation against those goldens: TLC itself today, `dive` and external tools later;
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
implementation.rs   Implementation trait: TLC, dive (stub), external adapter (stub)
pipeline.rs / report.rs   commands, JSON report, docs/conformance.md
```

**The harness never guesses.**
- Every TLC run is classified from its `-tool` message stream, then cross-checked against the exit code.
- An exit code that the messages do not explain is `oracle-error`, with both recorded.
- An unknown exit code or message code, or a malformed stream, is `unclassified`, with the raw data kept.
- An implementation that cannot do something must answer `unsupported`.

**Each model gets exactly one outcome:**
- `agree`;
- `disagree`, with a field-by-field diff;
- `unsupported`;
- `oracle-error`;
- `timeout`;
- `out-of-scope`.

**Comparison rules** (ADR 0004):
- Always compared: the verdict class and the error class (the `EC` name of the deciding message).
- Distinct and generated counts: compared at one worker, or when the oracle searched the whole state space (success, liveness violation).
- Depth, counterexample length, and the violated property when both sides name one: compared only at one worker.
- The counterexample is compared by length, never by content.
- Simulation models compare verdict-level fields only.

## Commands

```sh
cargo run -p dive-conformance --release -- <command>      # or target/release/dive-conformance

oracle fetch                    # download + verify oracles, classpath jars, the host JRE, module libraries
oracle verify                   # re-verify the cache and print each oracle's probed capabilities
corpus fetch [--corpus ID]      # git checkout of pinned corpora; submodules are recorded, not fetched
corpus list  [--tier small|deep|all] [--filter GLOB]
golden generate [--oracle ID] [--tier T] [--filter GLOB]... [--reason TEXT] [--no-sany|--sany-only]
run --impl tlc|tlc:<oracle-id>|dive|adapter:<file.toml>
    [--golden-oracle ID] [--workers 1,4] [--crosscheck-manifest] [--tier T] [--filter GLOB]...
```

**Common options:**
- `--jobs N` (default `min(4, cpus/2)`): parallel oracle runs.
- `--xmx SIZE` (default `3g`): JVM heap per run. Four 3 GB JVMs fit an 8-core / 24 GB machine with room to spare.
- `--timeout SECS`, or the derived default: 10 × the manifest runtime + 60 s, at least 120 s, capped by `--timeout-cap` (default 3600). Unknown runtimes get the cap. On timeout the whole process group is killed with `SIGKILL`.
- `--java PATH`: opts into a system Java. It is recorded in provenance as `source: system`, with the executable's SHA-256.
- `--repo-root DIR` and `--cache-dir DIR`.

**Outputs:**
- `run` writes `conformance/reports/<run-id>.json` (schema version 1) and `docs/conformance.md`. Use `--no-summary` to skip the summary, or `--summary PATH` to put it elsewhere.
- Exit status: 0 for success; 1 for findings (unwritten goldens, or `disagree`, `oracle-error` or `timeout` outcomes); 2 for a usage error; 3 for a harness error.

**Examples:**

```sh
dive-conformance golden generate --tier small                     # all small models + SANY goldens
dive-conformance run --impl tlc --crosscheck-manifest              # self-consistency + manifest drift
dive-conformance run --impl tlc:tlc-1.7.4 --no-summary             # cross-oracle diff vs default goldens
dive-conformance run --impl tlc --workers 1,4 --filter 'specifications/ewd840/*'
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
tmp/                     per-run scratch: TLC -metadir, java.io.tmpdir, -dumpTrace output; removed after each run
```

**Isolation.** Each jar sits alone in its own directory. That matters because `tla2tools.jar`'s manifest `Class-Path` names `CommunityModules*.jar` *relative to the jar*; isolation guarantees that only the jars on `-cp` are used.

**Why shell out to `curl`, `git` and `tar`.**
- The harness needs HTTPS with redirects, GitHub authentication, shallow git fetches of a single commit, and `.tar.gz` extraction.
- Doing that in-process would pull in an HTTP stack, TLS, git and archive crates, a large dependency surface for a tool that already needs a JVM.
- `curl`, `git` and `tar` are present on every developer machine and CI runner.
- Integrity does not depend on them: every artifact is hashed in-process (`sha2`) before use, and checkouts are verified by commit SHA.
- The GitHub token is passed to curl on stdin (`--config -`), never on a command line.
- `unzip` is used only to read the jar manifest's `X-Git-Revision`. If it is missing, the revision is simply not recorded.

**The private oracle asset.** It is fetched through the GitHub REST API: the release by tag, then the asset URL with `Accept: application/octet-stream`. The token comes from `GH_TOKEN`, `GITHUB_TOKEN` or `gh auth token`. The browser download URL answers 404 for private assets even with a token. Public assets are fetched from their URL without a token.

## Adding an implementation adapter

Implement `implementation::Implementation`:

```rust
pub trait Implementation: Sync {
    fn name(&self) -> String;
    fn is_oracle(&self) -> bool;          // false for anything that is not TLC
    fn check_model(&self, req: &ModelRequest<'_>) -> Result<ImplResult<OracleResult>>;
    fn parse_module(&self, root: &Path, module: &str, timeout: Duration) -> Result<ImplResult<SanyResult>>;
}
```

**Rules for adapters:**
- Return `ImplResult::Unsupported(reason)` for anything the tool does not implement exactly. Never return a partial or approximate result.
- Results use the oracle vocabulary. `verdict` and `error_class` must be the TLC `EC` name of the equivalent outcome; counts, depth and trace length go in their fields.

**Wiring.** Add the new variant to `--impl` in `src/main.rs`.

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

**Committed goldens.** `conformance/golden/` currently holds an 11-model smoke set for both oracles, plus SANY goldens for their modules, as regression anchors. Full-corpus goldens are generated by the lead and committed separately.

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
| Simulation is not reproducible even with `-seed 0` and one worker. `EWD840_anim` generated 267,302, 267,196, 271,147 and 268,375 states in four runs, with the same verdict and trace length. Simulation counts are therefore kept out of the golden payload (`provenance.uncompared_counts`) | Examples `ewd840/EWD840_anim` |

## Limitations

- Only the `examples-manifest` and `modules-only` layouts are enumerated. `tbd` corpora (ValidationTestSuite, the tlaplus repo tests) report "not yet supported".
- Examples submodules (CCF, azure-cosmos-tla, BlockingQueue, microwave) are never fetched. They are listed in reports as unfetched.
- Simulation results are compared by verdict only; the simulation is pinned with `-seed 0`.
- External adapters are parsed but not executed.
- The harness targets macOS and Linux. Process-group killing uses Unix process groups.
