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
- `Reference retirement`, the remaining cleanup after the old checking runtime's removal;
- `post-v0.1`, everything frozen or deferred.

## Where we are

- **Core is dive's only checking engine.** `dive check`, `dive simulate`,
  `dive dfid` and `dive tlc` run on Core, with shared start-up (`dive-model`)
  and the independent trace validator (`dive-trace`, on `dive-eval`). Features
  Core does not implement are named refusals (#910).
- **Coverage and speed (R1, R2) are met on the release candidate.** On the full
  Examples tier Core agrees on 162 models, matches a registered divergence on 7
  and refuses 7 by name, with no disagreements; its geometric-mean time is
  0.195x TLC's, with no model above 2x. The [scoreboard](scoreboard.md) holds
  the numbers and the refusal-reason histogram.
- **Core's small-tier floor is enforced.** The committed baselines record 141
  agreements, nine registered divergences and six named refusals in the default
  profile, and 142 agreements, one registered divergence and 13 named refusals
  in the TLC profile, with zero disagreements, budget stops or missing goldens
  ([conformance snapshot](conformance.md)). The full gate and the nightly job
  ratchet against them.
- **Bare use, nominal compat and the public surface are met on Core (R3–R5).**
  Libraries are bundled; the ten-spec bare-use smoke checks nine and refuses
  EWD998ChanID by name, including from the release archive with an empty
  `PATH`. `cargo xtask gate --full` passes, and help, `--version` and schema-3
  JSON are pinned.
- **The release is prepared but not tagged.** The version is `0.1.0`, the
  curated changelog is on main and the release dry run built both Linux
  archives (#805). Only the tag remains (R6, #794).

## Path to v0.1

Remaining work follows the [release gates](RELEASE.md), using measured reason
buckets rather than model-by-model history fixes:

1. Resolve the nine absent oracle facts through targeted pinned-oracle work on
   the original configurations. Keep them unmeasured until a legitimate golden
   exists.
2. Run `cargo xtask gate --full` on the release source and tag `v0.1.0` when
   all gates hold (#794).

Delivered release work: canonical sets and history removal (#786, #807, #815),
lexically scoped applied LET-recursive functions (#811), the Core-only small-tier
ratchet (#806), bundled TLAPS and CommunityModules with bare-use smoke (#803),
nominal compatibility and its known gaps (#810), CLI help/version (#802),
schema-3 report goldens (#804), Core as the only engine (#879) and removal of
the `dive-check` crate (#935).

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
- **Platforms:** macOS as a release platform.
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
