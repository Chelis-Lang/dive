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
