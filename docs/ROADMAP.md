# Roadmap

Milestones match the GitHub milestones on `Chelis-Lang/dive`. Each milestone ends with:
- a review pass (a red-team agent or a human);
- a conformance report delta;
- ADRs for the decisions it made.

The charter's soundness contract applies throughout.

The [scoreboard](scoreboard.md) records the latest corpus results and the
roadmap feature checklist. Parity means observable results (ADR 0086).

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

The design is in ADRs 0013–0016. The safety checker and parallel engine are implemented; storage, errors, performance and corpus follow-ups remain open.

## M6: Liveness, fairness and symmetry

- **Liveness:** explicit final/short checking (`--lncheck final --lasso short`) checks the supported pure temporal language, including weak and strong fairness, over the deterministic parallel safety graph. It selects deterministic lassos and validates them independently with `dive-trace`, including fairness and configured property attribution. Eligible ordinary TLC liveness requests now use this bounded final/short route. Default completed-level periodic checks remain pending, and unsupported language and request modes still refuse explicitly; full temporal parity remains open. TLC's internal tableau numbering, SCC selection and wall-clock scheduling are not parity obligations (ADR 0086).
- **Symmetry:** pure safety reduction now uses the bounded group and canonical keys in both stores, retaining concrete traces. `--compat=tlc` uses TLC's group, warning rules and representative order. Deferred/lazy permutations, unverified startup contracts and randomness remain partial. The intended default for symmetry with liveness is unreduced checking (D-005); TLC's behavior stays under `--symmetry-liveness=tlc`. Liveness symmetry modes and the symmetry audit remain planned and refuse explicitly.

The separate [symmetry safety study](../conformance/reports/studies/symmetry-safety.md)
records all eleven intended manifest targets plus the extra Voting selection
agreeing at four workers, with the original one-worker timeout preserved.
It does not increase the historical standard 83/71 report totals or certify
complete symmetry parity. The required CM1 gate now exercises symmetry-bearing
Paxos Voting at 1/4/8 workers without removing an existing case.

The design is in ADRs 0017–0019 (proposed).

## Through full TLC parity

The approved rescue roadmap in issue #241 retains complete symmetry, temporal
language, ordinary integrated liveness, simulation/generation and DFID, remaining
operators/configuration/output/debugger services, bounded-memory spill, native
checkpoint/recovery, native distributed checking, PlusCal and release
certification. Consolidating the search kernel and simplifying independent trace
replay support the remaining periodic liveness and mode integration. Corpus coverage, PlusCal, CLI/API
and performance work can proceed independently; modes and storage then build on
the consolidated kernel. Current capabilities and remaining work are recorded in
[the scoreboard](scoreboard.md).

Native protocols/checkpoints satisfy the approved scope; interoperability with
Java workers/checkpoints and arbitrary JVM plugin loading are excluded. Host
access still requires explicit category grants even under `--compat=tlc`.

## After parity

The empty `dive-lsp` and `dive-compile` placeholder crates are retired
([ADR 0095](decisions/0095-retire-tooling-placeholders.md)). Language-server and
native-compilation work remain deferred until after v1, when they return as
fresh implementations.

- **Integration:** LSP on the certified front end/checker.
- **Compilation:** native specification compilation and the Verus export path.

Public checking, simulation, translation, checkpoint and distributed library
APIs with `cargo test` integration are part of parity, not postponed here.

## Ordinary liveness integration

Final/short checking and independent validation already support finite
constant-domain quantifiers inside state/action predicates, scoped temporal
operators and pure WF/SF actions. One shared planner retains TLC's observable
conversion errors; the retained formula classifier covers TLC errors 2213 and
2214 for later integration. See [the temporal guide](user/temporal.md) for the
current language and explicit refusal boundary.

Eligible ordinary TLC requests already use FinalShort under the request guards
described in the temporal guide. Default completed-level periodic checking remains
pending. After kernel consolidation and independent replay simplification, add
deterministic completed-level checks with geometric growth thresholds. Reuse the bounded partial-graph SCC and growth kernels where
useful. A prefix check may prove a violation, but only complete exploration can
prove success. Independent validation, named-property attribution, temporal
language coverage and unreduced symmetry checking remain acceptance requirements.

The removed Phase-L receipt, producer-clock, request-owned, hook and terminal
service diagnostics are historical. Ordinary liveness does not require matching
TLC's wall-clock thresholds, queue suspension order, tableau numbering or lasso
choice (ADR 0082, ADR 0086 and charter appendix 12). Checkpoint/recovery and other
modes retain their observable contracts, including equivalent resumed and
uninterrupted outcomes. Any new result-changing default still needs an approved
and documented divergence under ADR 0010.
