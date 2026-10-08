# Oracles: tlaplus/tlaplus (TLC and SANY)

Data gathered 2026-10-08. Unless stated otherwise, upstream paths are relative to
`tlatools/org.lamport.tlatools/` in `tlaplus/tlaplus`. "Master" means commit
`94d0c5070852b37b801bb8998c436383ab7ee37b` (2026-10-06), which is also the commit
the current `v1.8.0` pre-release was built from. "1.7.4" means tag `v1.7.4` at
`5a47802b5c391f59ecdd44117981f4ff8c0656ba`.

## Findings that affect the charter and ADRs

> **1. v1.7.4 is a 2020 maintenance branch, not a recent cut of master.**
> `GET /repos/tlaplus/tlaplus/compare/v1.7.4...master` (2026-10-08) reports the
> two as diverged, with merge base `eb3ff99d16` (the `v1.7.0` tag, 2020-04-25).
> Master is 1663 commits ahead and 59 behind. v1.7.4 therefore lacks about six
> years of development. Its SANY has no Unicode syntax (`8fa1f19c4d`, "Add unicode
> support to SANY (#896)", is on master only). Its jar ships no `TLCExt.tla` or
> `Json.tla` standard module. Its TLC has no `-dumpTrace`, `-generate`,
> `-noGenerateSpecTE`, `-inv` or `-messagesAsErrors` flags (table below). The
> tlaplus/Examples corpus is validated only against the nightly build
> (`corpora.md`). Charter appendix item 3 is correct that 1.7.4 is the latest
> *stable* release. But goldens generated with 1.7.4 will disagree with the
> corpus manifests in ways that are oracle drift, not `dive` bugs. The re-hosted
> 1.8.0 snapshot should be the **primary** oracle, with 1.7.4 as a secondary
> comparison oracle.
>
> **2. The SANY CLI exits 0 on semantic and level errors unless it is given
> `-error-codes`, and 1.7.4 rejects `-error-codes`.** On master,
> `tla2sany/drivers/SANY.java:223-225` returns `OK` unless `doStrictErrorCodes` is
> set. Only parse errors, which raise `ParseException`, exit non-zero without it.
> In 1.7.4, `frontEndMain` (`tla2sany/drivers/SANY.java:117-146` at `v1.7.4`)
> returns -1 only on exceptions, and semantic errors do not throw. Any unknown
> switch, including `-error-codes`, prints `Illegal switch` and exits -1 (255)
> (`:409-420` at `v1.7.4`). The charter treats the pinned SANY as the
> acceptance/rejection oracle. The harness must therefore classify SANY results
> per oracle version, not by exit code alone (details below).
>
> **3. The 1.8.0 snapshot changes semantics between pushes.** The commit at
> master HEAD is titled "Len, Tail, and SubSeq count UTF-16 chars, not characters"
> (`94d0c50`, 2026-10-06). Every re-pin of the rolling 1.8.0 build can move
> verdicts. The pin record should hold the `X-Git-Revision` from the jar
> manifest, not just the SHA-256.
>
> **4. ADR 0005 is slightly inaccurate.** It says Examples CI classifies "by exit
> code (0, or 10–14)". `tla_utils.py:265-273` (Examples `6a7b26a`) maps only 0, 10,
> 11, 12 and 13. Exit code 14 (assert) is not mapped, and there is no manifest
> `result` for it. See `corpora.md`.

## Releases and pinning

Source: `gh api repos/tlaplus/tlaplus/releases`, `git ls-remote --tags`, local
`shasum -a 256` of downloaded jars, 2026-10-08.

| Artifact | Commit | Published | `tla2tools.jar` SHA-256 (computed locally) | Upstream digest | Class-file target |
|---|---|---|---|---|---|
| `v1.7.4` ("Xenophanes", stable) | `5a47802b5c39` | 2024-08-05 (assets uploaded 2024-08-08) | `936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88` | none | major 52 (Java 8) |
| `v1.8.0` ("Clarke", pre-release, rolling) | `94d0c5070852` | 2026-10-06T01:48Z | `7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d` | GitHub `digest` field: `sha256:7beec0f0…` (matches) | major 55 (Java 11) |
| `https://nightly.tlapl.us/dist/tla2tools.jar` | same as v1.8.0 | Last-Modified 2026-10-06T01:43:53Z | `7beec0f0…` (identical to v1.8.0 asset) | none | 55 |

- **Rolling 1.8.0.** `.github/workflows/main.yml:269-287` runs `git tag -f v1.8.0`
  and force-pushes on every master push. `:194-252` deletes and re-uploads the
  release assets, then flips the release to draft and back to refresh
  `published_at`. The same job rsyncs the jar to `upload.tlapl.us:dist/` (`:258`),
  which serves as `nightly.tlapl.us`.
- **GitHub asset digests.** Assets uploaded recently carry a server-computed
  `digest` (`sha256:…`) in the releases API. The 1.7.4 assets (uploaded 2024)
  have none. The digest for 1.8.0 changes on every push. It is still useful as a
  download-time cross-check of a snapshot, but it is not a pin.
- **CalVer tags.** `main.yml:277-285` also creates an annotated tag from
  `TLCGet("revision").calver`, for example `2026.10.06.014338` → `94d0c50`. On
  2026-10-08 there are 60 such tags, from `2026.04.27.203507` to
  `2026.10.06.014338`. No GitHub release exists for them
  (`releases/tags/2026.10.06.014338` returns 404), so they carry no assets. They
  are useful as human-readable commit names for a re-hosted snapshot.
- **Maven Central snapshots.** `main.yml:292-299` deploys `org.lamport:tla2tools:1.8.0-SNAPSHOT`
  to `central.sonatype.com/repository/maven-snapshots`. The latest is
  `1.8.0-20261006.015956-102` (maven-metadata.xml, fetched 2026-10-08). That jar
  has `javax/*` and `com/*` removed (`main.yml:298`), so it is **not**
  byte-identical to the GitHub asset. Snapshot repositories are also subject to
  purging. It is not a substitute for re-hosting.
- **Jar manifest.** Both jars record `X-Git-Revision` in `META-INF/MANIFEST.MF`
  (1.7.4: `5a47802…`; 1.8.0: `94d0c50…`, `Build-TimeStamp: 2026-10-06T01:43:38.313Z`).
  Both set `Class-Path: CommunityModules-deps.jar CommunityModules.jar` (1.7.4:
  `CommunityModules.jar` only). **A CommunityModules jar placed next to
  `tla2tools.jar` is silently put on the classpath.** The harness must run the
  oracle from an isolated directory, and add CommunityModules only on purpose.

### Java requirement

- Master: `README.md:18` and `USE.md:4` say "require Java 11+".
  `customBuild.xml:11` sets `java.release=11`, used by every `javac` task
  (`:108`, `:168`, `:338`, …). CI builds with JDK 17 (`.github/workflows/main.yml:58-62`).
- 1.7.4: `customBuild.xml:122` at `v1.7.4` compiles with `source="1.8" target="1.8"`.
  This matches class-file major 52 in the released jar, and its manifest says
  `Created-By: 11.0.3+7-LTS (Azul Systems, Inc.)`.
- One pinned JDK 17 or 21 runs both jars.

## TLC command line

Flags parsed in `tlc2/TLC.java` (`handleParameters`), listed by grepping
`args[index].equals("-…")`. The usage text is `TLC.java:1721-1930` on master.

| Flag | Master | 1.7.4 | Notes for the harness |
|---|---|---|---|
| `-tool` | `:632` | `:410` | Wraps every message in `@!@!@STARTMSG`/`ENDMSG` (below). |
| `-workers N\|auto` | `:1034` | `:741` | Default 1. Use `1` for depth/trace goldens (ADR 0004). |
| `-config file` | `:684` | `:527` | Default `SPEC.cfg`. |
| `-deadlock` | `:508` | `:378` | **Disables** deadlock checking (same as `CHECK_DEADLOCK FALSE`). |
| `-cleanup` | `:512` | `:382` | Deletes the states directory. |
| `-metadir path` | `:1005` | `:712` | Default `SPEC-dir/states`. Point it at a per-run temp dir. |
| `-seed N` | `:927` | `:634` | Simulation RNG seed. If omitted, a random long is chosen (`:1394`). |
| `-fp N` | `:1086` | `:793` | Picks the Nth polynomial of 131 in `FP64.Polys` (`tlc2/util/FP64.java:267`). **If omitted, the index is random per run** (`TLC.java:230`). Pin it so collision-probability output is reproducible. |
| `-lncheck default\|final\|seqfinal\|off` | `:672` | `:515` | `TLCGlobals.java:152-159`. Examples CI uses `final`. |
| `-simulate [num=N,file=…]` | `:465` | `:351` | `-depth` (default 100) and `-aril` also apply. |
| `-generate` | `:465-469` | **absent** | Simulation with `Tool.probabilistic=true`. |
| `-dumpTrace tla\|json file` | `:766` | **absent** | Repeatable; writes the counterexample. |
| `-noGenerateSpecTE` / `-noTE` | `:642` | **absent** | On master, the TE spec is generated by default unless `-tool` is set (`:1242-1250`). Pass it anyway. |
| `-teSpecOutDir`, `-noGenerateSpecTEBin` | `:646-662` | absent | |
| `-inv expr`, `-invlevel n`, `-postCondition mod!op` | `:581`, `:599`, `:817` | absent | |
| `-messagesAsErrors`, `-suppressMessages` | yes | absent | Used by ValidationTestSuite. |
| `-loadTrace fmt file`, `-debugger` | yes | absent | |
| `-generateSpecTE`, `-continue`, `-view`, `-dfid`, `-depth`, `-maxSetSize`, `-fpmem`, `-fpbits`, `-checkpoint`, `-recover`, `-coverage`, `-difftrace`, `-terse`, `-nowarning`, `-userFile`, `-gzip`, `-dump` | yes | yes | |

**TLC has no `-deterministic` flag** (charter appendix 6 confirmed). Full
reproducibility of an oracle run needs `-workers 1`, an explicit `-fp`, and,
for simulation, `-seed`. JVM flags seen in the wild: `-XX:+UseParallelGC`, and
`-Dtlc2.tool.impl.Tool.cdot=true` for action composition (Examples
`tla_utils.py:142-149`).

## The `-tool` message protocol

Source: `tlc2/output/MP.java` (master).

```
@!@!@STARTMSG <code>:<class> @!@!@
<message body, may span lines>
@!@!@ENDMSG <code> @!@!@
```

- `DELIM = "@!@!@"` (`MP.java:179`), `STARTMSG = "STARTMSG "` (`:180`),
  `ENDMSG = "ENDMSG "` (`:166`). Assembly is at `:374-379` and `:404-407`. The
  wrapping is applied only when `TLCGlobals.tool` is set.
- Message classes (`MP.java:187-203`): `0` NONE (standard output), `1` ERROR,
  `2` TLCBUG, `3` WARNING, `4` STATE (state print).
- Without `-tool`, classes become the prefixes `Error: `, `TLC Bug: `, `Warning: `
  and `State ` (`:381-397`). Stats numbers are printed with `String.valueOf`
  (`tlc2/tool/ModelChecker.java:885-886`), so they have no thousands separators.
  Progress lines use `DecimalFormat("###,###.###")` (`MP.java:220`).

### EC codes a harness needs (`tlc2/output/EC.java`, master)

| Code | Constant (EC.java line) | Message text (`MP.java` line) | Use |
|---|---|---|---|
| 2262 | `TLC_VERSION` (:342) | version banner | record oracle version |
| 2187 / 2188 / 2271 | `TLC_MODE_MC` / `TLC_MODE_SIMU` / `TLC_MODE_MC_DFS` (:231-233) | mode banner | |
| 2220 / 2219 | `TLC_SANY_START` / `TLC_SANY_END` (:298, :123) | "SANY finished." (:592) | parse phase boundary |
| 2185 | `TLC_STARTING` (:205) | "Starting... (date)" (:1153) | start |
| 2189 / 2190 | `TLC_COMPUTING_INIT` / `TLC_INIT_GENERATED1` (:234, :236) | initial states | |
| 2192 | `TLC_CHECKING_TEMPORAL_PROPS` (:240) | liveness phase | |
| 2200 | `TLC_PROGRESS_STATS` (:252) | progress | ignore |
| 2193 | `TLC_SUCCESS` (:242) | "Model checking completed. No error has been found." plus "calculated (optimistic): %1%" and "based on the actual fingerprints: %2%" (:1228-1236) | success and **collision probability** |
| 2199 | `TLC_STATS` (:249) | "%1% states generated, %2% distinct states found, %3% states left on queue." (:1260-1261) | generated and distinct counts |
| 2210 | `TLC_STATS_SIMU` (:251) | "The number of states generated: %1% … seed %2% and aril %3%" (:1266) | simulation |
| 2194 | `TLC_SEARCH_DEPTH` (:243) | "The depth of the complete state graph search is %1%." (:1238) | depth |
| 2268 | `TLC_STATE_GRAPH_OUTDEGREE` (:244) | outdegree stats | optional |
| 2186 | `TLC_FINISHED` (:206) | "Finished in %1% at (…)" (:1156) | end |
| 2104 / 2105 | `TLC_ASSUMPTION_FALSE` / `TLC_ASSUMPTION_EVALUATION_ERROR` (:73-74) | "Assumption %1% is false." (:574) | assumption verdict |
| 2107 / 2110 | `TLC_INVARIANT_VIOLATED_INITIAL` / `_BEHAVIOR` (:80, :83) | "Invariant %1% is violated." (:622) | safety |
| 2111 | `TLC_INVARIANT_EVALUATION_FAILED` (:84) | | eval error in invariant |
| 2108 / 2112 / 2113 | `TLC_PROPERTY_VIOLATED_INITIAL`, `TLC_ACTION_PROPERTY_VIOLATED_BEHAVIOR`, `_EVALUATION_FAILED` (:81, :87-88) | | action properties |
| 2114 | `TLC_DEADLOCK_REACHED` (:89) | "Deadlock reached." (:662) | deadlock |
| 2116 | `TLC_TEMPORAL_PROPERTY_VIOLATED` (:92) | "Temporal properties … violated" (:668) | liveness |
| 2132 | `TLC_VALUE_ASSERT_FAILED` (:151) | "The first argument of Assert evaluated to FALSE…" (:848) | `Assert` |
| 2118 / 2256 | `TLC_NO_STATES_SATISFYING_INIT[_AND_CONSTRAINT]` (:94-95) | | |
| 2106 / 2109 | `TLC_STATE_NOT_COMPLETELY_SPECIFIED_INITIAL` / `_NEXT` (:78, :82) | | eval errors |
| 2103 | `TLC_NESTED_EXPRESSION` (:72) | location stack of an eval error | |
| 1000 | `GENERAL` (:57) | | most next-state evaluation errors (`ModelChecker.java:645-660`) |
| 2178 | `TLC_MODULE_OVERFLOW` (:193) | "Overflow when computing %1%" (:901) | 32-bit overflow (`tlc2/module/Integers.java:149,162,207`) |
| 2265 | `TLC_INTEGER_TOO_BIG` (:345) | "TLC can't handle a number this big." (:1453) | literal out of range |
| 2179, 2183, 2184, 2169, 2170 | `TLC_MODULE_DIVISION_BY_ZERO`, `_ARGUMENT_NOT_IN_DOMAIN`, `_APPLY_EMPTY_SEQ`, `_ARGUMENT_ERROR`, `TLC_ARGUMENT_MISMATCH` (:178-200) | | eval-error class |
| 2121 | `TLC_BEHAVIOR_UP_TO_THIS_POINT` (:99) | "The behavior up to this point is:" (:752) | trace header |
| 2264 | `TLC_COUNTER_EXAMPLE` (:343) | "The following behavior constitutes a counter-example:" (:665) | trace header |
| 2120 | `TLC_ERROR_STATE` (:98) | | |
| 2216 / 2217 | `TLC_STATE_PRINT1` / `TLC_STATE_PRINT2` (:287, :296) | "%1%: %2%\n%3%" (:1476-1480) | **one trace state**, class 4 |
| 2218 | `TLC_STATE_PRINT3` (:121) | "%1%: Stuttering" (:1483) | stuttering lasso |
| 2122 | `TLC_BACK_TO_STATE` (:112) | "%1%: Back to state…" (:728-736) | lasso loop-back |
| 2279 | `TLC_FEATURE_UNSUPPORTED_LIVENESS_SYMMETRY` (:54) | "Declaring symmetry during liveness checking is dangerous…" (:983) | **warning** only, emitted at `tlc2/tool/AbstractChecker.java:119-121` |
| 2284 | `TLC_FEATURE_LIVENESS_CONSTRAINTS` (:55) | constraints with liveness (:988) | the call at `AbstractChecker.java:128` is commented out |
| 2171 / 3002 | `TLC_PARSING_FAILED2` / `TLC_PARSING_FAILED` (:185-186) | "Parsing or semantic analysis failed." (:840) | TLC-side SANY failure |
| 2222–2242, 2257–2259, 2272, 2280–2281, 5002+ | `TLC_CONFIG_*`, `CFG_*` (:225, :271-335) | | config errors |
| 2501 / 2502 | `TLC_TE_SPEC_GENERATION_COMPLETE` / `_ERROR` (:351-352) | | ignore |

### Exit codes (`EC.ExitStatus`, `EC.java:360-480`)

| Exit | Name | Mapped from |
|---|---|---|
| 0 | `SUCCESS` | `NO_ERROR` |
| 10 | `VIOLATION_ASSUMPTION` | 2104, 2105; on master also 2404, 2405, 2780 (postcondition, `_POSSIBLE`) |
| 11 | `VIOLATION_DEADLOCK` | 2114 |
| 12 | `VIOLATION_SAFETY` | 2107, 2110 |
| 13 | `VIOLATION_LIVENESS` | 2108, 2112, 2113, 2116 |
| 14 | `VIOLATION_ASSERT` | 2132 |
| 75 | `FAILURE_SPEC_EVAL` | 2109, 2115, 2103, 2147 |
| 76 | `FAILURE_SAFETY_EVAL` | 2111, 2146 |
| 77 | `FAILURE_LIVENESS_EVAL` | 2253 |
| 150 | `ERROR_SPEC_PARSE` | 2171, 3002 |
| 151 | `ERROR_CONFIG_PARSE` | `TLC_CONFIG_*`, `CFG_*` (the `CFG_*` cases are master only) |
| 152 / 153 | `ERROR_STATESPACE_TOO_LARGE` / `ERROR_SYSTEM` | declared but not produced by the mapping |
| 255 | `ERROR` | **everything else**, including `GENERAL` (1000) and `TLC_MODULE_OVERFLOW` |

`TLC.java:382` exits with `errorConstantToExitStatus(errorCode)`. Most
evaluation errors found during exploration are recorded as `EC.GENERAL` or as
the `EvalException` code (`ModelChecker.java:645-660`; `Worker.java:150`). Both
fall through to **255**. **The exit code alone cannot tell an overflow from a
crash.** The harness must classify on the `-tool` message stream (first class-1
message code) and use the exit code as a cross-check. The exit-status mapping in
1.7.4 differs from master only in the extra cases noted above (diff of
`EC.java` `ExitStatus` between the two tags).

## SANY standalone

Main class `tla2sany.SANY`, a shim calling `tla2sany.drivers.SANY.SANYmain`
(`tla2sany/SANY.java:11-13`, identical in both versions).

| | Master | 1.7.4 |
|---|---|---|
| Flags | `-s` (no semantic analysis), `-l` (no level checking), `-g` (toggle linting), `-d` (explorer), `-stat` (ignored), `-error-codes`, `-suppressMessages codes`, `-messagesAsErrors codes`, `-help` (`drivers/SANY.java:545-690`) | `-s`, `-l`, `-d`, `-stat` only (`:409-420`) |
| Unknown flag | exception → exit 255 | prints `Illegal switch: …` → exit 255 |
| Parse error | `ParseException` → `SanyExitCode.ERROR` (-1) → **exit 255**, regardless of `-error-codes` (`:330-360`, `:212-216`) | exit 255 (`:133-136`) |
| Semantic or level error | **exit 0** without `-error-codes`; **exit 4** with it (`:223-225`; `SanyExitCode.java:45-62`: OK 0, SYNTAX_PARSING_FAILURE 2, SEMANTIC_ANALYSIS_OR_LEVEL_CHECKING_FAILURE 4, ERROR -1) | **exit 0** (no exception is raised for ordinary semantic errors; `spec.errorLevel=4` is not consulted) |
| Missing file | exit 255 | exit 255 |
| Output stream | stdout. Errors go to stderr only with `-Dtla2sany.drivers.SANY.errors2stderr=true` (`:712`) | stdout |

Output format: a `****** SANY2 <version>` banner per file; `Parsing file …` and
`Semantic processing of module …` progress lines; then `*** Errors: N` followed
by each error as `<location>\n\n<message>` (`tla2sany/semantic/Errors.java:85-86`,
`:185-200`). Locations read `line L, col C to line L2, col C2 of module M`
(`tla2sany/st/Location.java:344-361`). On master, errors carry stable codes from
`tla2sany/semantic/ErrorCode.java` (66 codes, for example 4003
`INTERNAL_ERROR`, 4805 `PLUSCAL_TRANSLATION_CHANGED_SINCE_LAST_TRANSLATION`).
That file does not exist at `v1.7.4`.

Examples CI runs `java -cp tla2tools.jar:apalache.jar:<spec dir>:<community>:<tlapm lib> tla2sany.SANY -error-codes <file>`
and treats exit 0 as accepted (Examples `parse_modules.py:42-64`).

## Semantics facts relevant to soundness

- **Fingerprints.** 64-bit Rabin fingerprints, `tlc2/util/FP64.java` ("Written by
  Allan Heydon and Marc Najork"). The irreducible polynomial table has 131
  entries (`:267`). The table index is random unless `-fp` is given
  (`TLC.java:230`, `:1086-1100`). The `TLC_SUCCESS` message reports an optimistic
  collision bound and, when available, a bound based on the actual
  fingerprints (`MP.java:1228-1236`).
- **Integers** are Java `int` (`tlc2/value/impl/IntValue.java:48`, `public final int val`).
  Overflow raises `EvalException(EC.TLC_MODULE_OVERFLOW, …)` in
  `tlc2/module/Integers.java` (for example `:149` `--2147483648`, `:162`
  `-2147483648 \div -1`, `:207` `x^y`), and in `Naturals`, `Bags`, `TLCGetSet`,
  `SubsetValue`, `FcnParams` and `SetOfRcdsValue` (grep of master). It is an error
  (exit 255), never wraparound. This confirms charter appendix 5.
- **Multi-worker BFS is approximate.** Lamport's comment is at
  `tlc2/tool/TLCTrace.java:102-115`, and the diameter approximation is described at
  `:138-152`. The usage tip "When using more than one worker, the reported depth
  might differ across runs" is at `TLC.java:1922`. tlaplus#883 "Nondeterminism in
  diameter reported by TLC" was opened 2024-02-21 and closed 2024-06-21. The
  usage tip and the Examples CI workaround (`tla_utils.py:300`) remain in place.
  This is consistent with ADR 0004.
- **Symmetry with liveness** only warns (2279) and continues
  (`AbstractChecker.java:118-121`). The matching warning for constraints with
  liveness (2284) is disabled (`:128`).
- **Strings (1.8.0 only, from 2026-10-06).** `Len`, `Tail` and `SubSeq` on strings
  count UTF-16 code units (`94d0c50`).

## TLC's own test suites

Counts come from a shallow clone of master on 2026-10-08, made with a local file
walk.

| Path | Contents | Size | Use for `dive` |
|---|---|---|---|
| `test-model/` | 456 entries; 654 `.tla`, 530 `.cfg` | 9.3 MB | Regression specs driven by `test/` JUnit classes |
| `test/tlc2/` | 711 `.java` | 2.5 MB | 503 files reference `ModelCheckerTestCase`, and 358 assert `EC.TLC_STATS` values, for example `recordedWithStringValues(EC.TLC_STATS, "6", "5", "0")` (`test/tlc2/tool/AbsoluteSpecPathTest.java:37`). **The expected results live in Java, not in sidecar files**, so they must be extracted or regenerated with the harness. |
| `test-model/suite/` | 135 `.tla`, 116 `.cfg` (`test1…`, `etest1…`) | 0.3 MB | Original Yuan Yu regression suite; small, focused on the evaluator. Good fast-tier seeds. |
| `test-model/symmetry/`, `test-model/pcal/`, `test-model/coverage/`, `test-model/simulation/`, `test-model/Github*`/`gh*` | 44/86/… `.tla` | | Symmetry, PlusCal, simulation and bug-regression cases |
| `test/tla2sany/corpus/` | 42 tree-sitter-format `.txt` files, 312 tests | | Syntax corpus shared with tree-sitter-tlaplus (README in that directory). Same format as `dive-syntax` parser tests. |
| `test/tla2sany/semantic/corpus/` | 28 `.tla` | | Semantic-acceptance cases (`SemanticCorpusTests.java`) |
| `test/tla2sany/semantic/error_corpus/` | 126 `.tla`, named by expected `ErrorCode`, for example `E4200_Test.tla`, `E4201_OpParameter_Test.tla` | | **Expected SANY rejection with a specific error code.** Directly usable as `dive-sema` negative conformance. |
| `test/pcal/` | 98 `.java` | 0.3 MB | PlusCal translator tests |
| `test-long/`, `test-concurrent/`, `test-verify/`, `test-jbmc-models/`, `test-benchmark/` | Java only | | Not a corpus |
| `src/tla2sany/StandardModules/` | 20 `.tla` (master), 10 at `v1.7.4` | | Standard modules `dive-stdlib` must reproduce |
| `src/tlc2/module/` | 19 `.java` | | TLC's Java overrides (provenance targets) |

All of this is MIT (`LICENSE` in tlaplus/tlaplus). Test specs carry their own
headers. Most are Microsoft or Linux Foundation copyright under MIT.

## Implications for dive

- Make the re-hosted **1.8.0 snapshot the primary oracle** and 1.7.4 the
  secondary. Record `X-Git-Revision` and the CalVer tag with every snapshot. Expect
  and report 1.7.4 disagreements on Unicode syntax, `TLCExt`/`Json`, and string
  length as oracle drift. ADR 0002 should state this explicitly.
- Oracle invocation for goldens: `java -cp <isolated dir>/tla2tools.jar tlc2.TLC -tool -workers 1 -fp 0 -lncheck final -metadir <tmp> -cleanup -config <cfg> <spec>`,
  plus `-noGenerateSpecTE -dumpTrace json <tmp>` on 1.8.0 only, and `-seed` for
  simulation. Never let a CommunityModules jar sit next to `tla2tools.jar` unless
  the model needs it (`Class-Path` manifest entry).
- Classify the verdict from the **first class-1 `-tool` message code**, using the
  tables above. Treat the exit code as a consistency check. Treat 255 as
  "evaluation error or crash", to be resolved from the message stream.
- SANY oracle: on 1.8.0 always pass `-error-codes` (exit 0, 2/255, 4). On 1.7.4,
  pass nothing extra and classify from stdout (`*** Errors:`, `Fatal errors`,
  `***Parse Error***`) as well as exit 255. A run that exits 0 but prints errors
  is a **rejection**.
- Use `test/tla2sany/semantic/error_corpus/` (126 coded rejections) and
  `test/tla2sany/corpus/` (312 syntax tests) in the fast tier for `dive-syntax` and
  `dive-sema`. Mine `test-model/` plus the `EC.TLC_STATS` assertions in
  `test/tlc2/` as a second model corpus, regenerating goldens through the harness
  instead of copying the Java literals.
- `dive` must model overflow as a distinct error class (TLC 2178), and report it
  as an evaluation error with TLC-compatible classification even though TLC exits
  255.
- Fingerprint mode must report a collision bound with the same semantics as
  `TLC_SUCCESS` (optimistic, and based on actual fingerprints) to be comparable.
