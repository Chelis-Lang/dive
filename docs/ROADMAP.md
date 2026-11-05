# Roadmap

The owner's direction since 2026-11-05 is
[ADR 0153](decisions/0153-core-is-the-product.md):
- Core is the product.
- The default profile does the better thing.
- `--compat=tlc` is nominal.
- dive releases at the first point of real user value.

[RELEASE.md](RELEASE.md) defines the release gates and the critical path.
[scoreboard.md](scoreboard.md) has the current numbers. The
[2026-11-05 progress review](reviews/2026-11-05-progress-review.md) explains the
reset. Plans give no dates or effort estimates.

GitHub milestones:
- `v0.1: first value`, the release work;
- `Reference retirement`, removing the old checking runtime;
- `post-v0.1`, everything frozen or deferred.

## Where we are

- **Front end, configuration and values work.** The front end agrees with SANY
  on 458 of 458 cases.
- **The Reference engine is still the default.** It agrees with TLC on 150 of
  the 156 small-tier models and on 182 of the 217 full-tier models in the TLC
  profile, with no disagreements. It runs at about a third of TLC's speed.
- **Core, the new execution engine (ADR 0117), is fast where it runs:** about
  2.6x TLC on its benchmark panel. It runs 246 of the 312 small-tier
  shadow models natively and refuses the rest by name. Core alone has never been
  measured on the full tier.
- **There is no release.**

## Path to v0.1

The order follows [RELEASE.md](RELEASE.md#critical-path):

1. Measure Core alone on the full tier and publish its refusal-reason histogram.
   Ratchet the Core-native count in CI.
2. Make sets canonical in the default profile, and delete the TLC
   normalization-history machinery.
3. Burn down Core refusals by reason bucket, largest first.
4. Core liveness at corpus scale (MCInnerSerial first, then capture capacity).
5. Bundle TLAPS and the CommunityModules; add a bare-use smoke test.
6. Make `--compat=tlc` nominal on Core, with a documented gap list.
7. Remove the Auto fallback and the Reference checking runtime from the product
   path.
8. Freeze the CLI and JSON surface, package, run one release-candidate
   qualification, and tag `v0.1.0`.

## After v0.1

These areas are frozen until v0.1 ships. Code already merged stays, and is not
advertised where it is incomplete.

- **Breadth:** the remaining TLC feature families, including IOUtils/IOEnv, TLC
  coverage and logical costs, and the full PlusCal flag matrix.
- **Modes and storage:** checkpoint, spill and recovery breadth; distributed
  workers; full simulation and DFID matrices.
- **Integration:** a DAP debugger; Rust APIs for checking, simulation,
  translation and checkpoints with `cargo test` integration; a language server
  built on the checker.
- **Performance:** ADR 0119 default floors across the full tier, scaling, and
  native compilation of specifications.
- **Platforms:** macOS as a release platform, and crates.io publication.
- **Verus:** an exporter to `verus-tla` state machines from the IR.

The empty `dive-lsp` and `dive-compile` placeholder crates are retired
([ADR 0095](decisions/0095-retire-tooling-placeholders.md)). The language server
and native compilation come back as new implementations after v0.1.

## Completed foundation milestones

| Milestone | Delivered |
|---|---|
| M0 Foundation | Workspace, lints, layering check, lean CI, commit hooks, charter and AGENTS.md |
| M1 Oracles and harness | Pinned TLC/SANY snapshots and JRE; pinned Examples, CommunityModules and TLC test corpora; `dive-conformance` with goldens, reports and worker invariance; the survey in `docs/survey/` |
| M2 Competitive analysis | `docs/survey/competitive.md` and the positioning ADR |
| M3 Syntax and values | Lossless parser with spans and recovery; `dive-value` with canonical order and FP64 fingerprints |
| M4 Semantics and configuration | SANY-equivalent `dive-sema`; the TLC `.cfg` format |
| M5 First verdicts | IR, evaluator, standard library, deterministic parallel safety checking, the independent `dive-trace` validator |
| M6 Liveness and symmetry | Final/short liveness with fairness, symmetry reduction, VIEW, the TLC profile |

Earlier phase narratives (Phases 4–8) are in git history. Their checkpoint
records remain under `conformance/reports/core/`.
