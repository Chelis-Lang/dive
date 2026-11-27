# Release 0.2.0: faster Core with Sail 0.3.1

[ADR 0153](decisions/0153-core-is-the-product.md) sets the direction. This file
defines when dive 0.2.0 ships and what work is on the path to it. It gives no
dates and no effort estimates. Progress is the measured gate numbers below.

dive 0.2.0 ships together with Sail 0.3.1, the Sail release that runs its native
temporal jobs on it. Release tracking issue: #1418, milestone
[`v0.2: faster Core with Sail 0.3.1`](https://github.com/Chelis-Lang/dive/milestone/12).

## What 0.2.0 delivers

0.1.0 gave a TLA+ user with an existing specification the TLC answer from
`dive check`, faster and without a JVM. 0.2.0 keeps that user and that promise
and makes the default path faster and smaller: Core speed and memory work,
strong fairness on Core, the reworked simulator (ADR 0162) and the incremental
recheck APIs that Sail uses.

Symbolic proofs are not part of 0.2.0. The symbolic worker and its strategies
stay in the source tree but are left out of the installed product: not in the
archive and not in `--help`. They ship working on every install route in v0.3
([epic #1417](https://github.com/Chelis-Lang/dive/issues/1417), milestone
`v0.3: symbolic proofs`).

## Breaking changes

0.2.0 is a minor bump under ADR 0157 because of these changes:

- The JSON report is `schema_version` 4 (#1084, #1087). It removes four
  always-zero construction-history metrics. Readers accept 4 and stop requiring
  those fields.
- `dive simulate` has two new owner-approved defaults: `--liveness-extend auto`
  (D-039) and `--sample-unbounded geometric` (D-040). `--compat=tlc` keeps
  TLC's behaviour for both.
- Core checkpoint archives use format `DIVECC02`. A `DIVECC01` archive is
  refused by name with `DIVE-S0400`.
- The library API gains `SimulateRequest` fields and removes public items in
  dive-eval, dive-core, dive-explore and dive-liveness.

The curated `## 0.2.0` section of `CHANGELOG.md` gives the migration for each.

## Gates

Each gate is measured on the frozen release-candidate commit. All seven must
hold to tag `v0.2.0`.

| Gate | Requirement | How it is measured |
|---|---|---|
| **R1 Coverage** | Default profile, **Core engine** (`--impl dive:core`), pinned Examples full tier, 1 worker. **0 disagreements.** Agreements plus registered expected divergences are at least 140, the frozen floor from the first release. The run leaves out the long tier, per the cached-TLC discipline. For comparison, 0.1.0 covered 169 with the long tier included. Every other model gives a named refusal or a declared budget stop. The four Core baselines in `conformance/baselines/` are refreshed through the harness from this run. | `dive-conformance run --impl dive:core --impl-profile dive --corpus examples --tier all --workers 1`, without the long-tier models, and the same with `--impl-profile tlc` |
| **R2 Speed** | End to end, including TLC's JVM startup, Core is faster than TLC on the geometric mean over the R1 agreeing models. A model slower than TLC gets a `perf` issue. A model more than 2x slower than TLC blocks the release. | One host, one worker, seed and fingerprint 0: pinned TLC (through the harness JRE) and `dive check` on every R1 agreeing model, wall clock including startup. Report the geometric mean of dive/TLC and list every model above 1x and above 2x. Golden TLC times from another host are triage only. The [R2 measurement procedure](developer/performance.md#r2-measurement-procedure) governs a gate run. |
| **R3 Bare use** | TLAPS and the CommunityModules ship inside dive at the lowest search priority, with provenance recorded. Ten original Examples specifications resolve with **no** `--module-path` or `--library-path` options. Nine must check successfully, including MCPaxos, MCBakery, at least two PlusCal specs and two liveness/fairness specs. EWD998ChanID may succeed or give its specific named VIEW/liveness or recursive-LET refusal (owner-approved bounded fallback); other failures block the gate. | The CI smoke test, and the same smoke on the built release archive with an empty `PATH` |
| **R4 Compat exists** | `--compat=tlc` runs on Core. Its known gaps are listed in `docs/tlc-differences.md`. It is not measured against an agreement target. | Small-tier run in the TLC profile reports no internal errors |
| **R5 Stable surface** | Per-command `--help` works. `--version` names the commit. Unimplemented commands such as `lsp` are hidden. The symbolic strategies and their flags are absent from installed `--help`; experimental tuning flags say so in their help. The JSON report (`schema_version` 4) and the CLI help are pinned by golden tests. | Golden tests in the workspace gate |
| **R6 Shipped** | A release workflow builds Linux x86_64 and aarch64 archives with checksums and without the symbolic worker. Every published crate is on crates.io at `0.2.0`, and `cargo install dive-cli --locked` works in a clean Cargo home. `CHANGELOG.md` has a curated `0.2.0` section with measured performance numbers. One release-candidate qualification (`cargo xtask gate --full`) passes on the tagged source. | Release workflow dry run on the candidate, then the run on the tag |
| **R7 Sail with dive** | Sail pinned to the dive release candidate, released as Sail 0.3.1. Through Sail's native temporal backend, checks on small specs and a session of many small checks are much faster than the standard TLC approach (a fresh JVM per check), with the same verdicts and distinct-state counts as the goldens and independently replayed counterexamples. A slower or disagreeing case blocks the release. | S7 of the [release performance qualification](developer/release-performance-plan.md), on one quiet host, 5 alternating repetitions, pinned TLC with its standard invocation |

The [release performance qualification](developer/release-performance-plan.md)
runs on the release-candidate source before the tag. Its R2 and S7 runs are
gate runs; its other measurements are directional, and each regression gets a
`perf` issue.

Not required for 0.2.0:
- symbolic proofs and the symbolic worker (v0.3, #1417);
- macOS archives and macOS as a gate;
- Core-native coverage beyond R1;
- distributed workers;
- DAP;
- checkpoint, spill or recovery breadth;
- IOUtils/IOEnv;
- TLC coverage and logical costs;
- LSP;
- the deep tier.

## Critical path

Features are frozen at `a1fd9e003` (#1407). The release is that commit plus
the PRs below, then one qualification of the frozen candidate. Each item is a
checkbox in #1418.

1. **Fixes.** A deterministic after-capture budget test (#1336) and the
   repeated forced Boolean formal (#1166).
2. **Freeze the surface** (R5). Take the symbolic worker out of the release
   archive, take the symbolic strategies and flags out of installed `--help`,
   mark the experimental tuning flags, and mark the dive-api modules Sail does
   not use `doc(hidden)` and the request types `non_exhaustive`.
3. **Release documentation.** This file, the release performance plan, the
   scoreboard's R2 row and the roadmap.
4. **Curate and version.** The default-path performance wins with their
   records, the `## 0.2.0` CHANGELOG section, the README numbers and the
   lockstep version bump to `0.2.0`.
5. **Sail on the candidate.** Pin dive to the release candidate, accept report
   schema 4 (Chelis-Lang/sail#786), use `SimulateReport::write_json`
   (Chelis-Lang/sail#767) and curate Sail's `0.3.1` CHANGELOG.
6. **Qualify the frozen candidate**, posting each result in #1418:
   - `cargo xtask gate --full`;
   - R1 on Core in both profiles, refreshing the Core baselines;
   - R2 under the quiet-node procedure;
   - R3 on the built archive;
   - D2 memory and D4 scaling;
   - R7/S7, Sail with dive;
   - dry runs of `release.yml` and Sail's publish workflow.
7. **Tag and publish** on an explicit owner go: dive `v0.2.0`, then Sail
   `0.3.1` pinned to the published dive.
8. **Docs surfaces**, before the tag and after publication, per the [release playbook](developer/releasing.md#docs-surfaces).

## Status

The current numbers live in [scoreboard.md](scoreboard.md). The pinned status
issue gets one comment per week with the R1–R3 numbers from the latest `main`.
Release-candidate results go in #1418.
