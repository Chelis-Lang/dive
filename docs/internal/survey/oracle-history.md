# Oracle history: what changed in TLC and SANY between 1.7.x and the pin

Data gathered 2026-10-08 from a blob-filtered clone of `tlaplus/tlaplus`.

- **Paths** are relative to `tlatools/org.lamport.tlatools/` unless stated otherwise.
- **"Pin"** means commit `94d0c50`, CalVer tag `2026.10.06.014338`, our default oracle (ADR 0002).
- **"1.7.4"** means tag `v1.7.4` (`5a47802`).
- **"Merge base"** means `eb3ff99d16`, which is tag `v1.7.0` (2020-04-25).
- **Commit tags.** Upstream ends each commit message with tags such as `[Bug][TLC]`, `[Feature][SANY]` or `[Changelog]`. Those tags are quoted where they matter.

## 1. Release model (facts)

| Fact | Value | Source |
|---|---|---|
| Last stable release | `v1.7.4` "Xenophanes", 2024-08-08. It is the last commit on branch `origin/v1.7.4`; the maintenance branch has had no activity since. | `gh api …/releases`; `git log origin/v1.7.4` |
| Maintenance-branch content | 59 commits, mostly backports: DFID fixes, LazyValue (`3ea322254`), memory barriers (`6047253dd`), liveness done-state (`1e9fd54db`). | `git log eb3ff99d16..v1.7.4` |
| Rolling line | `v1.8.0` "The Clarke release". It is a pre-release, re-published on every green master push. | `.github/workflows/main.yml:194-252, 269-287` |
| Version string | `TLCGlobals.Version` has been CalVer `YYYY.MM.DD.HHmmss` since `5a565e087` (2026-02-23). It is derived from the build timestamp. 2.x numbers are "reserved for point releases" of the maintenance branch. | `src/tlc2/TLCGlobals.java:39-46` |
| CalVer git tags | **60** tags (not about 120), from `2026.04.27.203507` to `2026.10.06.014338`, introduced by `e3142ca3b`. They cover 53 distinct commits; 7 commits were rebuilt by a `repository_dispatch` (`181330706` has 5 tags). A tag names a *build*, not a commit. | `git tag`; `main.yml:276-287` |
| Master since first CalVer tag | 149 commits, linear history. Not every commit gets its own build: pushes are batched, and failed runs produce no tag. | `git rev-list` |
| Archived jars | **None for old builds.** The GitHub asset is replaced on each push. The nightly mirror is overwritten (`main.yml:258`). Maven `1.8.0-SNAPSHOT` metadata lists only the latest build (`1.8.0-20261006.015956-102`), and an older timestamp returns 404. **A past CalVer build can only be obtained by rebuilding from source.** | `maven-metadata.xml`, 2026-10-08 |
| Java | Bytecode targets Java 11 since `677f52f18` (2024-05-15; `customBuild.xml:11`). CI builds with Temurin 17 (`main.yml:57-62`). 1.7.4 targets Java 8 (major 52). | |

## 2. Upstream guidance on which build to use

| Source | What it says |
|---|---|
| `README.md:11-14` (pin) | "Currently, every commit to the master branch is built & uploaded to the 1.8.0 Clarke pre-release. **If you want the latest fixes & features you can use that pre-release.**" Maven `1.8.0-SNAPSHOT` is offered for library use. |
| `README.md:18`, `USE.md:4` | "The TLA⁺ tools require Java 11+." |
| `TLCGlobals.java:39-46`, `5a565e087` | "The rolling release now uses Calendar Versioning … disambiguates **multiple releases** on the same day." Upstream calls each CalVer build a release. |
| Release body of `v1.8.0` (`general/docs/changelogs/ch1_8_0.md`, appended by `main.yml:236-248`) | The changelog text is frozen at 2020 content (see §3). It now ends with "**The TLA+ Toolbox is no longer actively maintained**", added by `52c0195d4` on 2026-03-03, and recommends the VS Code extension or the command line. |
| docs.tlapl.us `using:vscode:migrating_from_tlatoolbox` | "The TLA+ Toolbox has now moved to archived mode … not recommended for daily use." |
| VS Code extension (`tlaplus/vscode-tlaplus`) | It **ships the rolling `v1.8.0` jar**: `ci.yml:21` and `release.yml:37` download `releases/download/v1.8.0/tla2tools.jar`, and each tlaplus build dispatches an extension rebuild (`main.yml:330-337`). |
| Toolbox | Built from master as `TLAToolbox-1.8.0-*` (`main.yml:33-36`). The last stable Toolbox is 1.7.4. |
| tlaplus/Examples CI | Validates against the nightly (= `v1.8.0`) build (dive `docs/internal/survey/corpora.md` finding 2). |
| lamport.azurewebsites.net/tla/tools.html, docs.tlapl.us `using:start` and `using:vscode:getting_started` | No version guidance. `current-tools.md` speaks only of "the current release". |
| Deprecation of 1.7.x | Not stated anywhere. 1.7.x is simply unmaintained: no commit since 2024-08-08, and no backport of any fix in §4. |

**Reading.** Upstream points every user-facing channel (the VS Code extension, the Toolbox build, Examples CI and the README) at the rolling 1.8.0 build. It never formally deprecates 1.7.4, which GitHub still labels "Latest" because 1.8.0 is a pre-release.

## 3. Changelogs

| Item | Finding |
|---|---|
| Location | `general/docs/changelogs/ch1_5_7.md … ch1_8_0.md` plus a `gh-*.jq` release template per version. `readme.txt` holds the publishing script. |
| Structure | A header linking the GitHub milestone, then "Additional noteworthy changes", split into **Tools** and **Toolbox**, each with **Feature** and **Bugfix** bullets that cite commit SHAs. Then Contributors, then a Checksums table (sha1). CI appends the sha1 of each build (`main.yml:239-243`) and PATCHes the release body (`:245-248`). |
| `ch1_8_0.md` currency | **Stale.** Its bullets stop at 2020 commits (`e328ae9…`, `8b52d23…`). The 2026 edits only removed a dead link (`8fca17f1a`) and added the Toolbox notice (`52c0195d4`). The milestone link still says `milestone:1.7.1`. |
| De facto changelog | Commit trailers. Between the merge base and the pin there are 1083 `[TLC]`, 358 `[Bug]`, 436 `[Feature]`, 76 `[SANY]`, 16 `[Regression]` and **59 `[Changelog]`** tags. `git log --grep='\[Changelog\]'` is the closest thing to release notes. The 2026 `[Changelog]` commits are `bdf9e1415`, `afd3e0f9a`, `97c2d6603`, `4ba7d8811`, `c45c1258f`, `aee88d0ed`, `5fb516810` and `8a23cb1dc`. |
| Hardening effort | `tlaplus/model-checker-hardening` was created 2026-08-10 for "systematic, differential, fuzz, and regression testing" of TLC and Apalache. The issues it raised (#1407, #1415, #1433, #1445, #1449, …) drive most of the September 2026 fixes below. |

## 4. Upstream testing of builds

| Question | Answer | Source |
|---|---|---|
| Does a build pass tests before `git tag -f v1.8.0`? | **Yes, the unit/regression suite.** The `build` job runs `ant … compile compile-test test` (`main.yml:137-141`). The `test` target ends with `<fail message="Unit test failures." if="unit-test.failed"/>` (`customBuild.xml:942`), so any failure fails the step. Release upload (`:194`), INRIA rsync (`:254`) and tagging (`:269-287`) are later steps of the same Ubuntu job without `if: always()`, so they are skipped. | `main.yml`, `customBuild.xml:674-943` |
| Which tests? | All `**/*Test*.java` under `test/`: model-checker regression specs in `test-model/`, the SANY corpora, PlusCal and the formatter. A few are excluded as flaky or port-bound (`customBuild.xml:813-835`). Known bugs are kept as `@Ignore`d tests; for example `94d0c50` adds three `@Ignore`d UTF-16 tests. | |
| Not run before tagging | `test-long`, JPF (`test-verify`), OpenJML, JBMC (separate workflows) and **the tlaplus/Examples and CommunityModules integration runs, which are dispatched *after* tagging** (`main.yml:306-325`). An Examples regression is therefore discovered after the CalVer tag exists. | |
| Matrix | Ubuntu, macOS and Windows run the tests. Only the Ubuntu job publishes and tags. GitHub's default `fail-fast` usually cancels Ubuntu when another OS fails first. | `main.yml:24-36` |
| Track record since CalVer | Pushes produced 50 successful, 11 failed and 1 cancelled runs; dispatches produced 10 successful and 2 failed. No failed run produced a tag. | `gh run list --workflow main.yml` |
| Is a CalVer tag a release? | **It is a tested, uniquely named build, but not an archived release.** Upstream calls it a release (§2), and it passed the unit suite. It has no GitHub release object, no changelog, no retained asset and no Examples gate. Rebuilding the same commit gives a different CalVer and different jar bytes. For dive, "release" should mean the pair (CalVer tag, SHA-256 of the jar we captured), which is what ADR 0002 records. | |

## 5. Semantics-affecting changes, merge base → pin

**Category codes:**

| Code | Meaning |
|---|---|
| **V** | verdict or soundness |
| **L** | liveness |
| **N** | state counts or depth |
| **P** | SANY acceptance (parse, semantic or level) |
| **S** | standard-module or value semantics |
| **R** | randomisation or simulation |
| **C** | cfg file or CLI |
| **X** | exit or EC code |
| **O** | output text or format |
| **T** | `TLCGet`/`TLCSet` |
| **D** | default |
| **J** | Java |
| **W** | multi-worker, disk or distributed robustness |

**The ★ column** marks a fix where older builds gave a **wrong answer**: an oracle-correctness fact. Where 1.7.4 differs, the 1.7.4 result is wrong.

**The Goldens column:**

| Value | Meaning |
|---|---|
| **Y** | Can change a verdict, count or code on ordinary specs. |
| **E** | Edge construct only. Needs a minimal conformance spec. |
| **O** | Message text or format only. |
| **–** | No effect under our golden flags (`-tool -workers 1 -fp 0 -seed 0 -lncheck final -cleanup`, no distributed mode; `-dumpTrace json` only in the separate trace pass, see §7). |

**In 1.7.4?** "no" unless noted.

### 5a. Since CalVer tags began (2026-04-27 → pin)

| First tag | Commit | Change | Cat | ★ | Goldens |
|---|---|---|---|---|---|
| 2026.04.27.203507 | `e3142ca3b` | Adds `TLCGet("revision").calver`; CI tags builds. | T | | – |
| 2026.04.29.170100 | `cc6b616cf` | `_POSSIBLE` stops printing `_PrintCounts` periodically. | O | | O |
| 2026.05.12.162145 | `5d411bfab` | PlusCal `define` accepts INSTANCE with several substitutions. | P | | – |
| 2026.05.26.235334 | `4ba7d8811` | RECURSIVE operators may define liveness properties. Previously `TLC cannot handle the temporal formula`. `[Changelog]` | L | | E |
| 2026.07.15.134331 | `227f61b98` | A missing cfg file is reported directly, without the "unexpected exception" wrapper. | O | | O |
| 2026.08.21.155922 | `6d60c50d5` | Unwitnessed `_POSSIBLE` now raises new EC **2780** `TLC_POSSIBLE_UNWITNESSED`, where it raised 2404 before. Exit stays 10. | X | | E |
| 2026.08.21.155922 | `b2e4b7fc9`, `76ef7d3e6` | Coverage counts INSTANCE substitutions per action, and reports implied init and actions (`-coverage` only). | O | ★ | – |
| 2026.09.09.124804 | `bd49ddf43`, `77d5cfb7a`, `35a6e0e9a`, `60bfecd58`, `b70b5c3cb`, `79db56853` | **Empty-set semantics (#1407).** Emptiness, `=`, `IsFiniteSet`, `Cardinality`, printing and `\in` for `[S -> T]`, records and `\X` with an empty constituent are now decided correctly. Example: `IsFiniteSet([{} -> Nat])` was FALSE. **Also new:** `77d5cfb7a` makes TLC *refuse* (error) some comparisons it answered before, such as `[1 -> 2] = [1 -> 2]` with a non-set argument. | S V | ★ | E |
| 2026.09.09.124804 | `65fbace6a`, `647de3151`, `96497919f`, `ac9ad279a`, `1a7b8816d`, `005f8a168` | SANY diagnostic wording and parameters. | O | | O |
| 2026.09.09.150548 | `c918bed2e` | `{}` and `{<<>>}` sets of functions, records or tuples enumerate without enumerating their arguments. This used to be a "cannot enumerate" error. | S | ★ | E |
| 2026.09.09.162536 | `4ad12e85c` | `RandomSubset` on an empty function or record set: NPE becomes a correct result. | R | | E |
| 2026.09.09.162536 | `8cd757a79` | SANY: a long `\h`/`\o`/`\b` numeral crashed with NumberFormatException (#1419). | P | ★ | E |
| 2026.09.09.201538 | `163ea4395` | `RandomSubset` of a Cartesian product draws without enumerating it. | R | | – |
| 2026.09.11.160459 | `97c2d6603` | SANY: a named THEOREM directly after a submodule was rejected (#430). `[Changelog]` | P | ★ | E |
| 2026.09.14.200039 | `a6df19756`, `701c30eb2`, `ee2b86b37`, `1e5cb06dd`, `5dbdb429a` | **k-subsets (#1415).** Distinct k-subsets compared equal and containment was wrong. Cardinality is no longer limited to 63 elements, and the enumeration order is normalized. | S V | ★ | E |
| 2026.09.16–17 | `2df1029b7`, `078405c22`, `458675f3c` | State-queue lost notification (a hang), checkpoint suspension, disk pool writer. | W | | – |
| 2026.09.22.174218 | `afd3e0f9a` | SANY: `CASE` nested inside a `/\` or `\/` list was rejected as "not properly indented" (#487). `[Changelog]` | P | ★ | E |
| 2026.09.22.222048 | `35d40c91a` | **A liveness evaluation error in the initial state let TLC report success**, hiding later invariant violations (hardening #137). | V L | ★ | Y |
| 2026.09.23.154203 | `4260e4712` | **Liveness tableau: temporal negations were not normalized**, producing spurious counterexamples (e.g. `[]~~[]P`). | L V | ★ | Y |
| 2026.09.25.163503 | `ba8014317` | Intervals at MinInt and MaxInt: non-termination, inconsistent comparison, unsound fingerprints. | S V | ★ | E |
| 2026.09.25.163503 | `9be36babe` | Empty interval-domain functions were equal under `=` but distinct in sets, e.g. `Cardinality({[x \in 2..1 \|-> x], [x \in 3..2 \|-> x]})` was 2. Can change distinct-state counts. | S N | ★ | E |
| 2026.09.25.163503 | `8f4bc8b73` | Every empty function is `<<>>` to the Sequences operators, e.g. `Len([x \in 2..1 \|-> x])` raised an error. | S | ★ | E |
| 2026.09.30.131340 | `bdf9e1415` | **`Bags` no longer `EXTENDS TLC`.** Specs relying on leaked `Print`, `:>` or `@@` now fail in SANY (#1449). `[Changelog]` This is a breaking change, not a fix. | P S | | **Y** |
| 2026.09.30.135409 | `341472c7e` | Bags count arithmetic wrapped: `BagCardinality([x \in {1,2} \|-> MaxInt])` was −2. It now raises overflow 2178. | S V | ★ | E |
| 2026.09.30.153528 to 2026.10.01.024053 | `fbab21214`, `240fd525d`, `001c588de` | Overflow in ValueVec search, interval size, and growth of vectors and hash tables. Only affects collections of about 2^30 elements. | W | ★ | – |
| 2026.10.01.003433 | `d17f1d35e` | **A lexical error in the cfg silently truncated the cfg**, so TLC could report success while ignoring an INVARIANT. It is now `CFG_LEXICAL_ERROR` 5007 with exit 151. | C V X | ★ | Y |
| 2026.10.01.003433 | `f66e82a01` | A cfg syntax error exits **151** (`ERROR_CONFIG_PARSE`) instead of 255. | X | | Y |
| 2026.10.02.140004 | `0a20a0bfb` | SANY: nonfix minus `- (1, 2)` was rejected. | P | ★ | E |
| 2026.10.02.152454 | `dfd4f7ff9` | Checkpoint recovery read the wrong width. | W | ★ | – |
| 2026.10.02.163556 | `181330706` | `DOMAIN` of a function with a tuple-bound parameter after a multi-name group: ClassCastException, or a wrong answer (`\A t \in DOMAIN … : FALSE` gave TRUE). | S V | ★ | E |
| 2026.10.05.160859 | `610da1d07` | `RandomSubset(k, 1..MaxInt)` failed. | R | | – |
| 2026.10.05.225551 | `ed3aed8f3`, `65bcb92a3` | Distributed TLC dropped states (BitVector). Worker ids of 128 or more were mis-decoded from the trace file. | W | ★ | – |
| 2026.10.06.014338 | `94d0c50` | **Test-only.** It adds `@Ignore`d tests documenting that `Len`, `Tail` and `SubSeq` count UTF-16 units (#1076). **No behaviour change**; 1.7.4 behaves identically (`Sequences.Len` uses `StringValue.length()` in both). | – | | – |

### 5b. Earlier in the last 12 months (2025-10-08 → 2026-04-26, untagged nightlies)

| Date | Commit | Change | Cat | ★ | Goldens |
|---|---|---|---|---|---|
| 2025-10-20 | `8d3143cf0` | SANY sends all output to **stdout** again. PR #1209 (mid-2025) had split it across stderr. | O | | O |
| 2025-10-24 | `602c9f0f2` | ALIAS was ignored when the error trace was printed. | O | ★ | E |
| 2026-01-01 | `9d644452c` | `x \subseteq S` in Init and `x' \subseteq S` in Next are supported. | S | | E |
| 2026-01-26 | `b11dbbca2`, `5eeb01f10`, `6320a09cb` | `-loadTrace` (replay), combinable with `-dumpTrace` and VIEW. | C | | – |
| 2026-02-20 | `72a0c2912` | A variable is accepted as a `PROPERTY`; it was rejected before. | C | | E |
| 2026-02-20 | `7e64fd037`, `32c479bb0` | SANY exit codes enumerated (0, 2, 4, −1). `errorLevel` is set when a syntax-phase exception occurs; before, it could read 0 (success). | X P | ★ | E |
| 2026-02-23 | `5a565e087` | Version banner (EC 2262) changes from "2.19 of …" to CalVer. | O | | O |
| 2025-01-07 (merged 2026-02-24) | `b1de459f0` | Bogus cfg error "substitutes constant C with non-constant C2". | C | ★ | E |
| 2026-02-24 | `2ff583f1a` | EvaluatingValue crashed when used as a constant substitute. | C | ★ | E |
| 2026-03-09 | `8a23cb1dc`, `7cfc5a9b7` | **ENABLED was unsound with INSTANCE substitution plus Java overrides** (e.g. `SelectSeq` with a LAMBDA). `[Changelog]` | V L | ★ | Y |
| 2026-03-13 | `01cac36a8` | **A bogus safety counterexample was reported while checking liveness** (the PEM was ignored), so properties that hold were reported violated. | L V | ★ | Y |
| 2026-03-13 | `4daa15584` | Properties defined via parameterized operators in an INSTANCEd module (#1161). | L | ★ | E |
| 2026-03-17 | `fbc17038b` | Warnings 2257 and 2259 are deferred until a counterexample ends in stuttering. | O | | O |
| 2026-03-18 | `d87585ecf` | SANY gains `-suppressMessages` and `-warningsAsErrors`. | C | | – |
| 2026-03-20 | `11bffb17c` | SANY lints only after semantic analysis and level checking. | P | | – |
| 2026-03-23 | `ff514f9e0` | Counterexamples are fully built before printing (no interleaving). | O | | – |
| 2026-03-23/30 | `6b13ef4e9`, `110851e63`, `2ee132cd7` | The liveness message names the property: "Temporal property **X** was violated." (EC 2116 now has parameters), with the definition name rather than the subformula location. | O | | O |
| 2026-04-06 | `95642cc4f` | TLC gains `-suppressMessages` and `-messagesAsErrors`. | C | | – |
| 2026-04-08 | `5fb516810` | `TLCGet`/`TLCSet` string registers `"s:…"` and `TLCGet("all:named")`. `[Changelog]` | T | | E |
| 2026-04-08 | `aee88d0ed` | Several POSTCONDITIONs. New EC **2404** and **2405** replace the assumption codes for postconditions; exit stays 10. `[Changelog]` | C X | | E |
| 2026-04-08 | `c45c1258f` | `_POSSIBLE` reachability assertions (provisional) and `_Possible.tla`. `[Changelog]` | C | | E |

### 5c. 2020-04 → 2025-10 (prioritised: not exhaustive for output, tooling or the debugger)

| Date | Commit | Change | Cat | ★ | 1.7.4 | Goldens |
|---|---|---|---|---|---|---|
| 2020-05-11 | `e6bd13ec9` | `String.strip` (Java 11). 1.7.x reverted it to `trim`. | J | | Java 8 OK | – |
| 2020-06/07 | `fa7663050`, `d62b289af`, `f5306c601`, `ced926989` | `TLCGet("generated")`, `TLCSet("exit")` in simulation, ALIAS, POSTCONDITION. | T C | | no | E |
| 2020-07-08 | `10f77cf67` | SANY `-error-codes` (opt-in non-zero exit). | X | | rejects flag | – |
| 2020-07-09 | `ab047b129` | `CHECK_DEADLOCK` keyword read as a value. | C | ★ | yes (`afae113db`) | E |
| 2020-08-10 | `51b5ef4ad` | Simulation ignored ASSUME. | V R | ★ | yes | – |
| 2020-09-22 | `d5654d93b` | The minimal module end marker is `====`. | P | ★ | no | E |
| 2020-12-19 | `7c9e44327` | A TE spec is generated by default (files written), unless `-tool` is given. | D | | no | – |
| 2020-12-29 | `0faf57869`, `f01d9250f` | DFID: missed invariant violations, and no liveness check. | V L | ★ | yes | – |
| 2021-01-30 | `856398af6` | `TLCExt` and `Json` become standard modules. | S | | no | Y |
| 2021-02-25 | `6fd29f2a6` | `TLCGet("level")` in constraints. | T | ★ | no | E |
| 2021-03-22 | `d2db743ce` | Liveness rejected mixed-type variables. | L | ★ | no | E |
| 2021-04-14/20 | `773e75e48`, `1b8546882` | **Bogus violations of `[](FALSE ~> P)` and `<>TRUE`** (#604). | L V | ★ | no | Y |
| 2021-04-22 | `673971ac7` | EvalException in a `~>` consequent ruled out by the antecedent. | L | ★ | no | E |
| 2021-05-06 | `1004befcf`, `d88b17093` | Bogus undefined-identifier error in fairness (#317). | L | ★ | no | E |
| 2021-06-30 | `9c63802db` | **`Head(str)`, `Append(str, str)` and string-as-tuple removed** (1.7.x has them; added 2019-12). `Len`, `Tail`, `\o` and `SubSeq` on strings remain. | S | | 1.7.4 accepts `Head("ab")` | E |
| 2021-07-08 | `79297f077` | `Next == \E i \in {} : …` reported "No next state actions" as an error; it now reports deadlock. | V X | ★ | no | E |
| 2021-07-14 | `dc8015b7a`, `6fc0f6831`, `041598b3a` | `TLCGet("stats")`, `TLCGet("config")`, `TLCGet("spec")`, `TLCGet("revision")`. | T | | no | E |
| 2021-07-29 | `6086bf2fd` | Bogus eval error comparing functions with equal-size, unequal domains. | S | ★ | no | E |
| 2021-07-29 | `508aaf05f` | Depth 1 was reported when zero states were generated. | N | ★ | no | E |
| 2021-11-02 | `f3520b81c` | Simulation: `RandomEnumerableValues` was always seeded with 0. | R | ★ | no | – |
| 2022-01-31 | `b04167104` | A behaviour-graph safety counterexample was a lasso instead of a finite prefix (#710). `[Changelog]` | L | ★ | no | E |
| 2022-04-15 | `df427fb9e` | `TLCGet("level")` was 1 inside `[i \in S \|-> …]` and `{i \in S : …}`. | T | ★ | no | E |
| 2022-04-19 | `3965cfadd` | Labels in liveness properties gave a bogus error (#647). | L | ★ | no | E |
| 2022-07-14 | `f60e10c21`, `2cb48326b` | **Spurious violation of an obvious temporal property** (#525). Bogus "tautology" error with `\A` in SF (#742). | L V | ★ | no | Y |
| 2022-08-19 | `65f3f8911` | SANY checks arity of implicit substitutions (#723). Accepted before. | P | ★ | no | E |
| 2022-09-06 | `9ff697509` | Counterexamples were sometimes truncated (#757). | O V | ★ | no | E |
| 2023-03/06 | `e7c51944f` then `dfbf8cea8`, `9aee565df` | LazyValue produced too many or too few states. **The branches differ:** 1.7.3 and 1.7.4 keep the original fix, while master reverted its logic (#817) and refactored. | N | ★ | different fix | E |
| 2023-06-16 | `7aac7873f` | `\A` over an empty set in liveness raised an error (#819). | L | ★ | no | E |
| 2023-07-21 | `e86829e72` | Error-trace headers show action arguments. | O | | no | O |
| 2023-10-23 | `1f9f3cb59` | The simulator drops finite stuttering from counterexamples. | R | | no | – |
| 2024-01-02 | `79c86b52e` | The empty sequence always prints `<<>>`; it was `<< >>` for function-backed values. | O | | no | O |
| 2024-04-10 | `8fa1f19c4` | **Unicode syntax in SANY** (#896). | P | | no | Y |
| 2024-05-15 | `677f52f18` | Java 11 bytecode. | J | | Java 8 | – |
| 2024-06-22 | `fde1c99ad` | RNG reset before constants are evaluated per worker (#866). | R | ★ | no | – |
| 2024-06-26 | `afa3233af` | `-lncheck off`. | C | | no | – |
| 2024-07-25 | `c61bad9a0` | Liveness overwrote a fingerprint's done state. | L | ★ | yes (`1e9fd54db`) | E |
| 2024-09-10/12 | `3465b79de`, `caf0c33ca` | Refinement checking for INSTANCE with arguments. `[Changelog]` | L | | no | E |
| 2024-09-30 | `3bf6f2544` | SANY drops the non-standard set-except syntax. | P | | accepts | E |
| 2024-11-09 | `673a71e5e` | **Level checking: false acceptances fixed.** | P | ★ | no | E |
| 2024-11-19 | `3f3d2998f` | `-dump dot,constrained` skipped invariants. | V | ★ | no | – |
| 2025-01-29 | `1c831f1c3` | Broken `LimitedPrecisionIndexer` in OffHeapDiskFPSet ("already on disk", hang; #1112). | W | ★ | no | – |
| 2025-02-07 | `2d8d41375`, `8ffdb7dab` | **"TLC calls operator with wrong value"** (#1134). | S V | ★ | no | E |
| 2025-02-17 | `9c0dc0762`, `be8659891` | SANY standard error codes (`ErrorCode.java`, 4200 block). | O X | | no | O |
| 2025-04/06 | `4b84a3b24`, `006fcfd12`, `e61495931`, `96d6a1adc`, `ef013c28c`, `2ec6b5f33` | New warnings: state-level PROPERTY, property without a behaviour spec, field name equal to a definition, trivially true liveness, constant-level invariant (EC 2149), missing fairness. They are fatal only under `-messagesAsErrors`. | O | | no | O |
| 2025-05-19 | `aac8d09c7` | Unbalanced backticks in string literals (#802). | P | ★ | no | E |
| 2025-06-18 | `36fab3ad8`, `85c924cc5` | `-inv`, `-invlevel`. | C | | no | – |
| 2025-08-01 | `4c10e1f23` | SANY parse-error details were silenced. This was a nightly-only regression from #1209. | O | | n/a | O |

**Unchanged between 1.7.4 and the pin** (diff of `TLC.java` and `TLCGlobals.java`):

- `-fp` is random unless given;
- `-workers` defaults to 1;
- `-lncheck` defaults to `default`;
- deadlock checking is on;
- `Integers`, `Naturals`, `Sequences`, `FiniteSets` and `TLC` `.tla` texts are unchanged. Only `Bags`, `Randomization` and `Toolbox` changed, plus the new modules `Json`, `TLCExt` and `_*`.

### 5d. Pending upstream, after the pin

| Branch | Change | Relevance |
|---|---|---|
| `mku-gh1445` (`c03865d98`, 2026-10-07) | **Workers raced to normalize shared values** (action arguments, `\E`/`\A` bindings, `TLCEval`, `TLCSet` registers): bogus states and bogus liveness counterexamples (#1445). | The pin is affected **only with more than one worker**. Our goldens use `-workers 1` (ADR 0004). Do not trust multi-worker pin runs. |
| `fhackett-fix-1076` (`e9715a670`, 2024-11) | Unicode in value serialization. Stale; does not fix `Len`. | UTF-16 `Len` stays the pin's behaviour. |

## 6. Corrections to dive documents

| Document | Claim | Correction |
|---|---|---|
| ADR 0002 addendum, bullet 2; `oracles.md` finding 3 and `:269-270` | "`94d0c50` … rolling builds change semantics, e.g. master counts `Len`, `Tail`, `SubSeq` in UTF-16 units." | `94d0c50` changes only a test file and adds `@Ignore`d tests. UTF-16 counting is long-standing and identical in 1.7.4. The general point ("rolling builds change semantics") is true. Cite `35d40c91a`, `4260e4712`, `d17f1d35e` or `bdf9e1415` instead. |
| `oracles.md:68-72` | "60 such tags" | Correct. The "about 120" in the task brief is not; there are 60 tags over 53 commits. |
| ADR 0002 Consequences | "Keeping v1.7.4 lets us detect oracle drift between releases." | 1.7.4 is not on the 1.8.x line: it is a 2020 fork with selected backports. Disagreements with it mix drift and known 1.7.4 bugs (★ rows with "no"). It cannot measure drift *within* 1.8.x. |

## 7. Effects of the harness's own invocation

Not upstream changes, but oracle facts that depend on how dive runs the pin.

| Found | Flag | Effect | Goldens |
|---|---|---|---|
| 2026-10-08 (issue #35) | `-dumpTrace json` | `ParameterizedSpecObj` makes the root module extend `_JsonTrace`, which loads `TLC`, `TLCExt`, `Json`, `Sequences`, `FiniteSets`, `Naturals` and, through `TLCExt`, `Integers`. Their Java overrides are installed, so `Integers`' replace `Naturals`' in a spec that extends only `Naturals`: `"a" >= 1` reports `>=` instead of `>`, `-2147483648 \div -1` overflows instead of wrapping, and `(0 - 1) \div 2` is `-1` instead of `0` (fixtures `ErGeqNaturals`, `ErPlusNonIntNaturals`, `RvNatDiv`). The extra modules are also lexed first, which moves names in TLC's intern order and so its string order: probe `StringOrderPreinterned` reports `InvA` with the flag and `InvB` without it. | Since #35 the deciding run omits the flag, and only a cross-checked trace pass uses it for the JSON document (ADR 0003 amendment; flag audit in the harness README). Regenerating all 141 Examples goldens without it changed no payload. Recapturing all 292 `dev capture` transcripts without it changed only the four above (and `Oom`, captured at a different `-Xmx`). |

