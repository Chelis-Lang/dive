# 0001: Workspace, toolchain, lints and CI tiers

- **Status:** accepted
- **Date:** 2026-10-08

## Context

The charter asks for:
- a Cargo workspace of small crates with strict dependency direction;
- a stable toolchain with a pinned MSRV;
- forbidden `unsafe`;
- pedantic clippy;
- tiered testing.

The Chelis-Lang org's Rust repos (buoy, barnacle, chelis, c-note) set the local conventions:
- resolver 3 and edition 2024;
- exactly pinned toolchains;
- `[workspace.lints]` with pedantic deny and `unwrap`/`expect`/`panic` deny;
- `cargo deny`;
- a per-repo AI-authorship guard;
- lean CI on `ubuntu-latest` with `Swatinem/rust-cache` saving only on main.

The owner explicitly asked that CI not grow bloated: perf and profiling work will be heavy, and CI does not need to repeat it on every commit.

## Decision

- **Workspace and toolchain:**
  - one crate per charter component under `crates/`, created now as documented stubs, so boundaries exist before code does;
  - `xtask/` for repo automation;
  - toolchain pinned exactly to Rust **1.99.0** (latest stable on 2026-10-08), with `rust-version = "1.99"`;
  - no nightly.
- **Lints:** `[workspace.lints]` follows buoy:
  - forbid `unsafe_code`;
  - deny `missing_docs`, `clippy::all`, `clippy::pedantic`, `unwrap_used`, `expect_used`, `panic`, `todo`, `unimplemented` and `dbg_macro`.
- **Crate layering:** permitted internal edges live in one table (`xtask/src/layering.rs`), checked by `cargo xtask check-layering` in CI. Unit tests prove the table is acyclic and keep `dive-trace` independent of the checker.
- **CI tiers:**
  - `ci.yml` (every push and PR) has two jobs: `No AI authorship markers`, then `Workspace gate`. The gate runs fmt, clippy, nextest, doctests, rustdoc, layering and cargo-deny, with a target under 10 minutes.
  - `conformance.yml` (nightly and dispatch) runs the standard and deep conformance tiers.
  - Benchmarks, profiling and long fuzzing never run in CI.
- **Hosting:** GitHub-hosted runners only. The org's self-hosted pool and Sand Dollar caches are admission-limited to other repos.
- **Debug info:** `debug = "line-tables-only"` in dev and test profiles. This follows the org's runner disk guidance.
- **Profiling builds:** a `profiling` profile (release plus full debug info).

## Consequences

- **Stub crates cost little now** and make layering violations visible from day one.
- **The gate stays fast.** The expensive conformance and perf work is opt-in by schedule or dispatch.
- **Toolchain bumps are deliberate,** and touch both `rust-toolchain.toml` and `Cargo.toml`.

## CI scheduling addendum (2026-10-11)

Main run 38093242207 exhausted the 15-minute Workspace gate limit. Authorship
and conformance passed; 202 of 894 tests had passed without a reported failure
when the runner cancelled the workspace job. This is a scheduling defect, not
evidence that the remaining tests passed. Issue #173 tracks the repair.

The full nextest workspace suite now runs in four hash partitions on separate
runners. No test selection, ignored-test policy or per-test timeout changes.
Each partition retains the 15-minute job limit, fail-fast stays disabled, and
the existing 60-second/three-period CI test limit remains intact. A separate
Workspace checks job runs every remaining original check and verifies that
the four partitions are disjoint and cover the unpartitioned test inventory.

Workspace gate retains its required check name. It runs even if dependencies
fail or are skipped, and explicitly requires success from authorship, workspace
checks and the complete test matrix. Cancellation, skipped prerequisites and
missing results fail closed. The conformance job remains unchanged.

Local contributors still run the whole gate. Partition reproduction and
coverage verification were documented in [CI](../developer/ci.md); see the
lean-gate addendum below.

## Recovery-matrix scheduling addendum (2026-10-11)

PR #179 run 38104630509 passed conformance and workspace checks, but three
test partitions exhausted the unchanged 15-minute job bound. Runtime-name
recovery wrappers also exhausted the unchanged 180-second test bound. The
complete failed log and original test source remain under
`conformance/probes/native-parity-integration/current-controls/full-runtime-ci-before-scheduling-repair/`.

Runtime-name controls now partition their exact original cross-product by store
and requested width. Both chaos settings, all three pause boundaries, changed
recovery widths, both counter policies, metadata comparisons and independent
witness checks remain unchanged. Ten broad wrappers become 60 smaller tests;
no setting or assertion is removed. The full workspace uses eight disjoint
hash partitions, checked against the complete actual inventory. The per-test
and per-job bounds, required aggregate gate and no-ignore policy are unchanged.

## Lean-gate addendum (2026-10-12)

Issue #241 replaces the partitioned CI. The twenty-job, roughly 190
runner-minute PR run (sixteen test partitions, an aggregator and about seventy
evidence-script steps) becomes three jobs with the unchanged required names:
`No AI authorship markers`, `Workspace gate` (one compile; fmt, clippy, the
nextest `ci` profile, doctests, rustdoc, layering, standalone `dive-trace`, and
`cargo deny` only when `Cargo.lock` or `deny.toml` changed) and `Conformance
gate` (an eight-model CM1 smoke list at 1 and 4 workers). The `ci` profile
excludes slow, evidence-oriented and multi-worker matrix tests by name.

The excluded tests, the whole CM1 list at 1, 4 and 8 workers, the
validation-smoke corpus and the small corpus tier run locally with
`cargo xtask gate --full`, which is required before PRs that touch the checker
crates and writes a Markdown report for the PR body. `conformance.yml` runs on
manual dispatch only.
