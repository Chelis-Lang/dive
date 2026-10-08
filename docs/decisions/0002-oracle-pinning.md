# 0002: Oracle pinning and Java runtime

- **Status:** accepted
- **Date:** 2026-10-08

## Context

The charter calls for pinning `tla2tools.jar` by release version and SHA-256, and for being able to run conformance against more than one TLC release. Upstream reality on 2026-10-08:

- **v1.7.4** (2024-08-05) is the latest *stable* release. It publishes no checksum.
- **v1.8.0** is a *rolling* pre-release. The workflow `.github/workflows/main.yml` force-moves its tag and re-uploads `tla2tools.jar` on every master push, so its URL and digest change over time. The upstream Examples CI uses this rolling build, via the nightly mirror.
- **CalVer tags** such as `2026.10.06.014338` mark each *successful* master build. Not every commit gets one, and some commits get several (see `docs/survey/oracle-history.md`). They are git tags only, with no release assets.
- **Java and build provenance:**
  - TLC needs Java 11+.
  - The 1.8.0 jar's manifest records Temurin 17 and the `X-Git-Revision` it was built from.
  - The development machine and CI runners have no Java installed.

## Decision

- **Primary oracle `tlc-2026.10.06.014338`:**
  - It is the 1.8.0 jar built from upstream commit `94d0c50`, downloaded unmodified.
  - It was verified against GitHub's asset digest `sha256:7beec0f0…` and the manifest's `X-Git-Revision`.
  - It is re-hosted as release `oracle-tlc-2026.10.06.014338` on `Chelis-Lang/dive`, together with the MIT licence and a NOTICE.
- **Secondary oracle `tlc-1.7.4`:** fetched from upstream, verified against our own recorded SHA-256.
- **Classpath:** the CommunityModules release jars `202610040242` are pinned the same way. Examples specs depend on them.
- **Java runtime:**
  - Eclipse Temurin **JRE 17.0.20.1+1**, pinned per platform by Adoptium's published SHA-256.
  - The harness downloads and unpacks it into `$DIVE_CACHE_DIR`.
  - Using a system `java` is an explicit opt-in (`--java`), recorded in report provenance.
- **Location of pins:** all pins live in `conformance/oracles.toml`. Every artifact is checked by SHA-256 before use, and a mismatch is fatal.
- **Provenance:** every golden and report records the oracle id and SHA-256.
- **Refresh:**
  - A new snapshot gets a new id and release. Old ids stay available.
  - Changing the default oracle requires regenerating goldens with a recorded reason and reviewing the diff.

## Addendum (survey, 2026-10-08)

- **v1.7.4 is a 2020 maintenance branch.** It diverged from master at v1.7.0 and is 1663 commits behind. It has no Unicode syntax, no `TLCExt` or `Json` modules, and no `-dumpTrace`, `-generate` or `-noGenerateSpecTE` flags. It is a secondary comparison oracle only. The harness keeps per-oracle flags and records the features each oracle lacks.
- **The rolling builds change semantics.** Many recent changes are fixes to TLC answers that were wrong, for example `35d40c91a`, `4260e4712`, `d17f1d35e` and `bdf9e1415`; see `docs/survey/oracle-history.md`. (An earlier version of this addendum cited `94d0c50` as changing `Len` to UTF-16 units. That commit only adds disabled tests documenting a long-standing behaviour.) Pins therefore record the jar's `X-Git-Revision` as well as its SHA-256.
- **TLC's `-fp` is random per run unless pinned.** The harness pins it.
- **SANY exits 0 on semantic errors** unless it is passed `-error-codes`, which v1.7.4 rejects. The harness reads SANY output as well as the exit code.

## Owner decision on refresh (2026-10-08)

There is no refresh cadence for now. The single snapshot `tlc-2026.10.06.014338` is the oracle until the owner decides otherwise. Moving it remains a deliberate decision, made with regenerated goldens and a recorded reason.

## Revision: the 1.8.x line is the oracle (2026-10-08)

The owner asked to work against the 1.8.x line. Research on its release history is recorded in `docs/survey/oracle-history.md`. Findings:
- **Upstream points every channel at the rolling 1.8.0 build:** the README, the VS Code extension, the Toolbox build and Examples CI.
- **1.7.x is unmaintained.** It is not formally deprecated, but no fixes are backported to it.
- **Each CalVer tag is a build that passed upstream's unit and regression suite.** Old jars are not archived anywhere; they can only be rebuilt from source.

Decision:
1. **One gating oracle:** `tlc-2026.10.06.014338`. It is trusted **only at `-workers 1`**: the pin has an upstream multi-worker race in value normalisation (tlaplus#1445, fix pending on branch `mku-gh1445`), which can produce bogus states and liveness counterexamples. Goldens are already single-worker (ADR 0004).
2. **v1.7.4 becomes reference-only** (`role = "reference"`, follow-up issue). It never gates and runs only when selected explicitly. Its differences from the pin are mostly 1.7.4 bugs fixed upstream.
3. **No older 1.8.x drift oracle for now.** From now on, each outgoing pin is kept as a byte-exact reference oracle when the pin moves.
4. **The register `docs/survey/oracle-history.md`** catalogues semantics-affecting upstream changes:
   - Wrong-answer fixes get minimal conformance specs, reusing upstream's `test-model/` specs where they exist.
   - Questionable behaviours, such as UTF-16 `Len` and the #1445 race, go through ADR 0010 as possible divergences.
   - A harness-readable `conformance/oracle-changes.toml` (follow-up issue) lets differences between two TLC builds be reported as known upstream changes rather than dive bugs.
5. **One bundle:** the oracle, the CommunityModules jars and the module libraries (TLAPS library, Apalache modules) are pinned and moved together.
6. **Moving the pin** (no cadence; owner decision):
   1. Pick a tag whose upstream CI and Examples run both passed, and capture its jar immediately.
   2. Diff commits, the standard modules, `EC.java`, flag parsing and defaults.
   3. Update the register.
   4. Run the old and new oracles over the full corpus, and classify every difference as known or unexplained.
   5. Block the move while any difference is unexplained.

## Consequences

- The private release asset needs a GitHub token to fetch. CI has one; locally the harness uses `gh auth token`.
- **Default choice:** goldens follow the current upstream TLC, which is also what Examples is validated against, rather than a two-year-old stable release.
- **v1.7.4 cannot detect drift within the 1.8.x line.** It is a 2020 fork that lacks many fixes. Drift is tracked by the register in `docs/survey/oracle-history.md`.
- **Refresh cadence** is an open owner question.

## Amendments

- **2026-10-08 (ADR 0003).**
  - The classpath is now per oracle. The pinned CommunityModules jars break v1.7.4 on every spec (`TLCOverrides` references classes 1.7.4 lacks), so v1.7.4 runs without them.
  - The TLAPS library and Apalache's modules are pinned as `[[module_library]]` git checkouts, as upstream Examples CI uses them.
  - Fetching the private asset works only through the GitHub API asset endpoint; the browser URL answers 404 even with a token.
- **2026-10-08 (issue #16, roles and upstream changes).** Decisions 2 and 4 of the 1.8.x revision are implemented.
  - **Roles.** `oracles.toml` gives every oracle a required `role`:
    - `tlc-2026.10.06.014338` is `gating`, and the default oracle must be gating;
    - `tlc-1.7.4` is `reference`. The harness uses a reference oracle only when it is named (`--oracle`, `--golden-oracle`, `--impl tlc:<id>`). A run involving one never fails on findings, and its report is marked reference.
  - **Upstream changes.** `conformance/oracle-changes.toml` registers upstream changes, each with:
    - its upstream commit (or a `<from>..<to>` range when not attributed to one commit);
    - a description and a link;
    - the oracles that lack it and the oracles that have it;
    - the model and module globs and the fields it may change;
    - optional conditions on either oracle's result.

    A cross-oracle difference that registered changes fully explain is reported as `known-upstream-change` (ADR 0003). It is never `agree`, and it is never applied to `dive` or adapters.
  - **Seed.** It is seeded from the v1.7.4-vs-pin report:
    - CommunityModules incompatibility (`eb3ff99d16..94d0c50`);
    - `TLCExt`/`Json` (`856398af6`);
    - `ALIAS` (`f5306c601`);
    - depth for constant-only models (`508aaf05f`).

    Two differences there have no recorded commit and stay `disagree`: v1.7.4 omits the depth after a liveness violation, and it cannot resolve a nested module that an earlier module in the same file extends.
