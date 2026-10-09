# Roadmap

Milestones match the GitHub milestones on `Chelis-Lang/dive`. Each milestone ends with:
- a review pass (a red-team agent or a human);
- a conformance report delta;
- ADRs for the decisions it made.

The charter's soundness contract applies throughout.

## M0: Foundation

Delivers:
- the workspace with one stub crate per charter component;
- pinned toolchain, workspace lints, `cargo deny`, and crate layering enforcement;
- lean CI (`ci.yml`): AI authorship guard plus workspace gate;
- the commit-msg hook, `AGENTS.md`, and the charter with its corrections appendix;
- GitHub tracking.

**Exit:** CI green on `main`, and an AI-trailer commit rejected by both the hook and CI.

## M1: Oracles and conformance harness

Delivers:
- pinned oracles: a snapshot of TLC 1.8.0 re-hosted in this repo's releases, plus v1.7.4; both use a Temurin JDK fetched by SHA-256;
- pinned corpora: tlaplus/Examples, CommunityModules, ValidationTestSuite, and the tlaplus repo's own tests;
- the `dive-conformance` harness:
  - oracle and corpus fetch;
  - TLC `-tool` output parsing;
  - golden generation with provenance;
  - per-model reports in JSON and `docs/conformance.md`;
  - worker-count invariance runs;
  - a cross-check against the Examples manifests;
- the survey of oracles and adjacent work in `docs/survey/`.

**Exit:** the first oracle-only report (TLC against TLC goldens and the Examples manifests) is committed, and every model's outcome is accounted for.

## M2: Competitive analysis

Measures tla2, tla-rs, tla-mc and tlago through harness adapters, and reads tlaplusplus's source (it is GPL, so reference only). Each is assessed on:
- conformance, including whether it honestly refuses what it doesn't support;
- performance: startup, states per second, memory, and scaling from 1 to 8 workers;
- Rust ergonomics;
- architecture.

Outputs:
- `docs/survey/competitive.md`, covering where `dive` can beat each tool;
- an ADR on positioning and which designs to borrow;
- baseline numbers in `dive-bench`.

This is not a build-or-buy gate: `dive` is built regardless.

## M3: Syntax and values

- **`dive-syntax`:**
  - a lossless lexer and CST with spans;
  - error recovery and stable error codes;
  - round-trip, property and fuzz tests;
  - parse acceptance matching SANY on every corpus module.
- **`dive parse`** with `--output json`.
- **`dive-value`**, in parallel:
  - canonical ordering, hashing and FP64-compatible fingerprints;
  - property tests.

## M4: Semantics and configuration

- **`dive-sema`:** SANY-equivalent acceptance and rejection, covering the module system, levels and the proof-language syntax, measured against the Examples and SANY test corpora.
- **`dive-config`:** the TLC `.cfg` format.

## M5: First verdicts

- **IR and evaluation:** `dive-ir`, `dive-eval` and the `dive-stdlib` core.
- **Safety checking:** `dive-check` breadth-first safety checking, in both exact and fingerprint modes. It is designed from the start for *deterministic* parallel search (ADR 0010): the first release may run one worker, but its architecture must already be level-synchronous with deterministic ordering, so that `--workers N` gives results identical to TLC's single-worker run.
- **Trace validation:** the `dive-trace` validator replays every counterexample.
- **Conformance gate:** the standard conformance tier becomes a PR gate on a small subset.

The design is in ADRs 0013–0016 (proposed). The sequenced plan, with work packages, conformance milestones, the gate subset and benchmark targets, is `docs/design/m5-plan.md`.

## M6: Liveness, fairness and symmetry

- **Liveness:** `dive-liveness` checks temporal properties with weak and strong fairness, through TLC's tableau and possible-error models, on a behaviour graph built by the deterministic parallel search. Counterexamples are lassos or stuttering endings, deterministic at any worker count, and validated by `dive-trace` (fairness and property included).
- **Symmetry:** `SYMMETRY` for safety, with TLC's group and canonical representatives under `--compat=tlc`. The intended default for symmetry with liveness is unreduced checking (D-005); TLC's behavior stays under `--symmetry-liveness=tlc`. These modes remain planned.

The design is in ADRs 0017–0019 (proposed). The plan, with work packages, conformance milestones, gate additions and benchmark targets, is `docs/design/m6-plan.md`.

## Later

- **Checking features:** simulation and PlusCal.
- **Integration:** the library API with `cargo test` integration, and the LSP.
- **Compilation:** specification compilation and the Verus export path.
