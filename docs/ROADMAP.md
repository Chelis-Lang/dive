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

- **Core has its first full Examples measurement.** At one worker, the default
  profile records 126 agreements, two registered expected divergences, 42 named
  refusals, six declared budget stops and nine missing goldens, with zero
  disagreements among 185 in-scope models (233 selected, 48 excluded). The TLC
  profile records 128 agreements and the same remaining outcome counts. These
  2026-11-05 measurements predate canonical sets, now merged; subsequent
  focused gains are not folded into this full-tier result.
- **The Reference engine is still the default.** Its same-source measurement
  completed on 2026-11-06: default 146 agreements plus four expected divergences,
  13 refusals and 13 stops; TLC profile 150 agreements, 15 refusals and 11 stops.
  Both retain nine missing goldens and zero disagreements. Core covers seven
  models Reference cannot finish, but misses 29 Reference-covered models: a net
  coverage gap of 22. The historical 151 Examples agreements and 182 agreements
  across four corpora describe different measurements.
- **Core's small-tier floor is enforced in CI.** Default: 122 agreements, four
  registered divergences and 30 named refusals. TLC profile: 126 agreements and
  30 named refusals, projected from the completed full and auxiliary runs. Both
  have zero disagreements, budget stops or missing goldens; the TLC projection
  reports no internal errors.
- **Bare use, nominal compat and the public surface are delivered (R3–R5).**
  Libraries are bundled, the ten-spec bare-use smoke meets its checked/refused
  contract, and help/version and schema-3 JSON are pinned. Bare-use checks are
  Reference-backed and do not increase Core coverage.
- **Release speed is not established (R2).** Cross-host elapsed-time triage
  identifies three agreeing models over twice their golden TLC time; matched
  measurements are needed. Completed-model timing does not resolve the six
  full-tier stops.
- **There is no release.** Packaging preparation is pending in #805; R1, R2 and
  R6 remain open. [scoreboard.md](scoreboard.md) lists exact refusal reasons,
  model IDs, unmeasured cases and timing qualifications.

## Path to v0.1

Remaining work follows the [release gates](RELEASE.md), using measured reason
buckets rather than model-by-model history fixes:

1. Close the demonstrated Reference-covered gaps by reason bucket, with focused
   checks on current Core. The completed comparison identifies 29 such baseline
   refusals, led by recursive functions (10), quiet nested equality (4), lazy
   operator successor domains (3) and action composition (2). Canonical sets
   have since landed; retain verified focused results separately rather than
   adding them to the old census. Address the remaining recursive-helper/operator
   and substitution gaps. The owner resolved #813 in favor of precise registered
   Boolean-predicate bookkeeping differences; effect safety and bounded calls
   remain required.
2. Resolve the nine absent oracle facts through targeted pinned-oracle work on
   the original configurations. Keep them unmeasured until a legitimate golden
   exists; snapshots do not turn unknowns into native results.
3. Resolve Core performance and capacity stops, including MCInnerSerial after
   canonical admission. Confirm the three timing outliers on matched inputs;
   retain verdict, trace and observable-count obligations. The named #788
   liveness examples already agree or give their permitted refusal/stop; the
   measurement contains no capture-capacity refusal bucket.
4. Refresh the integrated Core measurement at the weekly or cutover boundary.
   Once R1 holds, remove the Auto fallback and Reference checking runtime from
   the product path, retaining shared startup and independent validation. Repeat
   the bare-use checks on Core before cutover.
5. Land packaging preparation (#805), complete R2, then run one integrated
   release-candidate qualification and tag `v0.1.0` when all gates hold.

Delivered release work: canonical sets (#807), lexically scoped applied
LET-recursive functions (#811), the Core-only small-tier ratchet (#806), bundled
TLAPS and CommunityModules with bare-use smoke (#803), nominal compatibility and
its known gaps (#810), CLI help/version (#802), and schema-3 report goldens (#804).

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
