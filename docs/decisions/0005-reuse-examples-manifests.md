# 0005: Reuse tlaplus/Examples manifests as corpus metadata

- **Status:** accepted
- **Date:** 2026-10-08

## Context

The charter asks for tlaplus/Examples to be pinned by commit as the backbone of conformance. Examples already has a schema-validated `manifest.json` per spec directory (`/manifest-schema.json`). Each model entry records:
- `path`, `runtime`, `mode` (exhaustive search, generate, symbolic, or simulate) and `result` (success, assumption/deadlock/safety/liveness failure, or unknown);
- optionally `distinctStates`, `totalStates`, `stateDepth` and `workers`.

Modules also record features such as `pluscal` and `action composition`. At commit `6a7b26a` there are 81 manifests listing 233 models, of which 182 are exhaustive. Examples' own CI (`.github/scripts/tla_utils.py`, `check_small_models.py`) runs TLC with `-workers`, `-lncheck final` and `-cleanup`, classifies results by exit code (only 0 and 10–13; see `tla_utils.py:265-273`), and treats "small" as a recorded runtime of 30 seconds or less in any mode.

## Decision

- **Corpus enumeration.** The harness reads the Examples manifests to enumerate modules and models, mode, expected result and features. It does not maintain its own parallel list.
- **Goldens.** Our goldens are regenerated from *our pinned* TLC (ADR 0002). The manifest's recorded numbers are a secondary cross-check: any golden that disagrees with its manifest is surfaced in the report as an oracle-drift finding.
- **Out of scope.** Models with `mode: symbolic` (Apalache) and `generate` are reported as `out-of-scope` with a reason. They are never silently dropped.
- **Tiers.** "Small" (standard tier) means exhaustive with a recorded runtime of 30 seconds or less: 140 of the 182 exhaustive models at `6a7b26a`. Restricting it to exhaustive models is our choice; upstream applies the runtime filter to every mode. Everything else belongs to the deep tier.
- **Verdict coverage.** No Examples model expects a deadlock, assumption or assertion failure. Those verdicts are covered by dive's own minimal specs, with oracle results, and by the tlaplus repo's test models.

## Consequences

We inherit upstream's curation for free. Manifest schema changes upstream need a harness update when the corpus pin moves.

## Amendments

- **Amendment 1, 2026-10-09 (issue #18): a dive-owned corpus in the Examples layout.**
  - **Decision.** dive's own conformance specs live in this repository as corpora with the `examples-manifest` layout, so the harness enumerates, tiers, isolates and compares them exactly like tlaplus/Examples. Each spec has its directory `specifications/<Name>/` holding its `.tla` and `.cfg` files and a `manifest.json` in Examples' schema. The manifests' `result` is the measured TLC result where it is a verdict, and `unknown` for errors (`check_small_models.py` has no error results). Counts are recorded only where they were measured.
  - **First corpus.** `probes` (`conformance/corpus/probes/`): the honesty and determinism probes of `docs/survey/competitive.md` (P1–P9 and M1–M2: overflow, `CHOOSE` order, a Java-only override, `ACTION_CONSTRAINT`, symmetry with liveness, `TLCGet`/`TLCSet`, `\div` and `%`, parallel determinism, `RandomElement`, a syntax error and an undefined name), copied verbatim from its appendix. Its startup benchmark `Trivial` is not a probe and is left out.
  - **Pinning.** `corpora.toml` names an in-repository corpus by `path` (under `conformance/`) instead of `repo` and `commit`. Its identity is the SHA-256 of its files (`sha256:<hex>`; `corpus::tree_digest`: every file's relative path, length and contents, in path order, `.DS_Store` ignored, anything but plain files refused). Goldens record it as their corpus commit, so editing, adding or renaming any file in the corpus makes `run` refuse its goldens until they are regenerated with a `--reason`, exactly as moving a pin does. `corpus fetch` skips it, and after a run the harness checks the digest instead of `git status`.
  - **Goldens.** Generated from the gating oracle under `conformance/golden/<oracle>/probes/`, like any corpus.
  - **Adapters.** The per-tool invocations of the competitive analysis are recorded as external adapters in `conformance/adapters/` (tla-mc, tla-rs, tlaplusplus, tlago, tla2). They are parsed and validated by the harness's tests, not run in CI, and exist for reproducible re-measurement once adapters execute (M2). The adapter schema gained what those invocations need: `source`, `library` (how the tool finds module libraries), `max_workers`, `env`, `refused_exit_codes`, ordered output `rules` (a line regex or a JSON pointer, giving a verdict or a refusal) and `counts` extractors. A result that no rule or exit code explains is unclassified, never a guess.
- **Amendment 2, 2026-11-08 (issue #854): dive-owned manifests over pinned upstream repositories.**
  - **Context.** R1 measures only tlaplus/Examples, the development target. A holdout of public industrial specifications (CCF, etcd, TiDB, Kafka and similar) needs the same enumeration, but those repositories have no Examples manifests, and their sources must not be copied into dive.
  - **Decision.** A repository-pinned `examples-manifest` corpus may name `manifest = "conformance/holdout/<id>.json"`: a dive-owned manifest in Examples' schema that lists the modules and models of the checkout by corpus-relative path. Every listed file must exist at the pinned commit. Files it does not list are not reported as uncovered. `[[corpus.exclude]]` entries name model ids (`.cfg` paths) reported as `out-of-scope` with their reason, for example models the pinned TLC cannot finish within the holdout cap; such models never get a golden. The corpus identity that goldens record is `<commit>+manifest-sha256:<hex>`, so editing the manifest makes `run` refuse the goldens until they are regenerated with a `--reason`, as moving the pin does. Listed paths must be plain relative paths inside the checkout, so the identity covers every file a model reads. The `[[corpus.exclude]]` entries are not part of the identity: editing an exclusion reason does not invalidate goldens, and removing an exclusion makes the model in scope with no golden, which `run` reports as a missing golden rather than a pass.
  - **Holdout.** Corpora with ids `holdout-*` are measured, not developed against: they inform R1's "effectively complete coverage" and are not a release gate. Disagreements on them are wrong user-visible results and become bug issues.
