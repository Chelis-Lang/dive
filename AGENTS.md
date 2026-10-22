# AGENTS.md

Operating guide for humans and coding agents working on **dive** (D.I.V.E., the
Deterministic Invariant Verification Engine), a ground-up Rust implementation of
the TLA+ toolchain. `CLAUDE.md` is a symlink to this file. `docs/CHARTER.md` is
the charter; this file must stay consistent with it. When they conflict, ask the
owner. The charter's appendix records later corrections.

## Ground rules

- **Parity means observable results** (ADR 0086). dive must match the pinned
  TLC/SANY on the verdict, distinct and generated counts (at 1 worker), the
  violated property name, the error class and exit code, the depth, and a
  counterexample that `dive-trace` independently validates. Do not emulate
  arbitrary or nondeterministic TLC internals, such as timing, thread or queue
  order, tableau numbering, SCC or lasso choice, or Java method projections.
- **Soundness first.** Anything dive does not implement exactly yields an
  explicit `unsupported` result naming the feature. Never approximate, never
  report success you have not proven, and never weaken a test, golden or
  conformance check to make a change pass. If a check looks wrong, raise it with
  evidence.
- **Goldens are generated, never edited.** `conformance/golden/` comes only from
  the harness against a pinned oracle, with a recorded `--reason`.
- **Surprising TLC behaviour:** add the smallest spec that shows it to the
  corpus with the oracle's result, and document it before implementing.
- **Ports follow TLC's semantics, not its structure.** Record provenance for
  ported algorithms in `docs/provenance.md`.
- **Better by default, TLC-compatible always** (ADR 0010).
  - `--compat=tlc` (also `DIVE_COMPAT=tlc` and the config key) is the single
    switch that makes every observable behaviour TLC-equivalent. The exception is
    host access: spec I/O needs explicit grants even in compat mode (D-017).
    Every new option declares its TLC-equivalent setting.
  - Where TLC is wrong or limiting, build a named dive option that does better.
    Result-preserving improvements are on by default once the harness shows they
    match the goldens. Result-changing ones are opt-in unless the owner approved
    them as the default. Propose new defaults in `docs/open-questions.md`.
  - Nondeterminism is replayable: always report the seed; `--seed` reproduces.
  - Register each divergence in `docs/divergences.md` (stable `D-NNN`, never
    reused) and describe it for TLA+ users in `docs/tlc-differences.md`, in the
    same PR as the code. Test both modes.
- **Evidence policy.** Never commit run outputs, logs, stdout or stderr dumps,
  `.gz` files, source snapshots or `before-*` directories. Committed evidence is
  compact JSON or Markdown, at most 100 KB per item. Bulky evidence is regenerated
  from pinned inputs. Test fixtures hold only what a test reads.
  `scripts/check-repo-budget.py` enforces this from the pre-commit hook.
- **Decisions** go in numbered ADRs in `docs/decisions/` (copy
  `0000-template.md`), one ADR per real decision. Changing the crate dependency
  graph is a decision.
- **Coverage** is tracked in `docs/scoreboard.md` (corpus numbers and roadmap
  checklist); `conformance/parity-surfaces.json` is the reference inventory.

## Commit messages and authorship

Use plain conventional commits (`feat:`, `fix:`, `docs:`, `chore:`, `test:`,
`refactor:`, `perf:`, `ci:`) under the configured human author. Commit messages
and PR bodies must not contain Claude, Codex, Copilot or other AI attribution,
AI co-authorship trailers, "Generated with" lines, or links to AI sessions.
This is enforced by `.githooks/commit-msg` (installed with
`cargo xtask install-hooks`, once per clone), by the CI job
`No AI authorship markers` (`scripts/check-ai-markers.sh`), and by
`.claude/settings.json`. Never bypass hooks with `--no-verify`.

## Workflow mode

**PR mode** (since 2026-10-08). `main` is protected (`cargo xtask protect-main`).
Every change lands through a squash-merged PR that passes the required checks
(`No AI authorship markers`, `Workspace gate` and `Conformance gate`); admins are
not exempt. CI's conformance gate runs `conformance/gates/smoke.txt` at 1 and 4
workers; the full cm1 gate runs locally with `cargo xtask gate --full`. Keep `main` green: gate locally before pushing, never merge
on red, and fix a red `main` first (label the issue `failing-on-main`).

- Small, single-scope PRs from `main`. No long-lived integration or compose
  branches.
- Each PR states the conformance entries it affects and the harness results.
- Parallel agents use separate worktrees, branches and build directories; the
  lead merges. Set `CARGO_TARGET_DIR=target` from the worktree root, or use a
  worktree-exclusive absolute directory. Never share a writable Cargo target between
  worktrees. If a target has already been shared, rebuild in a fresh directory
  before using its results as correctness or performance evidence.
- Feature notes go in one guide per area (for example `docs/user/*.md` or
  `docs/pluscal-translation.md`), not one doc per increment. User-visible changes
  update the user guide and `CHANGELOG.md` in the same PR.

## Repository layout

```
crates/dive-*   one crate per charter component (see docs/CHARTER.md)
xtask/          cargo xtask: install-hooks, check-layering, protect-main
conformance/    oracle and corpus pins, generated goldens, reports
docs/           charter, roadmap, ADRs, user and developer guides
scripts/        small helpers used by hooks and CI
.githooks/      tracked git hooks
```

**Layering.** `xtask/src/layering.rs` is the single table of permitted internal
dependency edges; `cargo xtask check-layering` enforces it. `dive-trace`, the
independent trace validator, must not depend on `dive-check`, `dive-liveness`,
`dive-sim` or `dive-store`. It shares the core evaluator, including `ENABLED`
(ADR 0103). `dive-model` (ADR 0013 §8) is shared by every backend.

## Rust conventions

- **Toolchain.** Pinned in `rust-toolchain.toml` (1.99.0); MSRV is
  `rust-version` in `Cargo.toml`. Bump both together, on purpose. Edition 2024,
  stable only.
- **Unsafe code.** `unsafe_code = "forbid"` workspace-wide and
  `#![forbid(unsafe_code)]` in every crate root. Exceptions need an owner-approved
  ADR, an isolated module, a safety argument and Miri tests.
- **Lints.** `[workspace.lints]`: `clippy::all` and `clippy::pedantic` deny, plus
  `unwrap_used`, `expect_used`, `panic`, `todo`, `unimplemented` and
  `missing_docs`. Document any local `#[allow]` with a reason.
- **Errors.** No panics on user input. User-facing errors carry a source span and
  a stable error code. Internal invariant violations are reported as bugs.
- **Dependencies.** Add crates to `[workspace.dependencies]` and reference them
  with `workspace = true`; `deny.toml` holds the licence and source policy. Every
  new dependency must earn its place.

## Gates

CI's required checks are deliberately small. The `Workspace gate` job runs, in
one compile, what `cargo xtask gate --fast` runs locally:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo nextest run --workspace --locked --profile ci   # excludes slow tests
cargo test --workspace --doc --locked
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
cargo xtask check-layering
cargo deny --locked check                       # only when Cargo.lock or deny.toml changed
```

The `ci` nextest profile (`.config/nextest.toml`) leaves out slow,
evidence-oriented and multi-worker matrix tests; everything it excludes still
runs in the full gate. The `Conformance gate` job checks the eight models of
`conformance/gates/smoke.txt` at 1 and 4 workers.

**Full gate.** `cargo xtask gate --full` runs the fast gate with the whole
nextest suite, then `scripts/conformance-gate.sh` over `conformance/gates/cm1.txt`
and the validation-smoke corpus at 1, 4 and 8 workers, then the small corpus
tier in both profiles through the harness, failing on any regression against
`conformance/baselines/` (`--skip-corpus` leaves that out; see
`docs/developer/ci.md`). It writes
`target/gate-report.md`. Run it before opening any PR that touches
`dive-eval`, `dive-check`, `dive-value`, `dive-store`, `dive-liveness`,
`dive-model` or `dive-trace`, and paste the report into the PR body. On a
shared machine, set `CARGO_BUILD_JOBS` and `NEXTEST_TEST_THREADS`.

**Testing tiers** (charter §Engineering practice):
- *Fast:* CI's `Workspace gate` and `Conformance gate`, on every PR and push to
  `main` (`cargo xtask gate --fast` locally).
- *Standard:* `cargo xtask gate --full`, run locally before checker PRs as
  above. Every gate model must fully agree. `.github/workflows/conformance.yml`
  runs the same small tier, plus the TLC self-checks, on manual dispatch.
- *Deep:* `conformance.yml` with `tier=deep`, on manual dispatch and before
  releases. Its separate frontend job runs deterministic syntax/layout/semantic
  mutations and both SANY-case directories through the pinned oracle. See
  `docs/frontend-campaign.md`; preserve the complete selection and raw reports.
- *Not in CI:* benchmarks, profiling and long fuzzing. Run them by hand with
  `dive-bench` and `cargo build --profile profiling`, and record machine details
  with the results.

## Conformance harness

`cargo run -p dive-conformance --release -- <command>` (see
`crates/dive-conformance/README.md` and ADRs 0002–0005):
- `oracle fetch` / `corpus fetch` download the pinned JRE, `tla2tools.jar`
  oracles, module libraries and corpora into `$DIVE_CACHE_DIR` (default
  `~/.cache/dive`), verified by SHA-256 or commit on every use;
- `golden generate` produces goldens from TLC and SANY (overwrite needs
  `--reason`);
- `run --impl tlc|tlc:<id>|dive|adapter:<toml>` compares an implementation with
  the goldens and emits `conformance/reports/<run-id>.json` and
  `docs/conformance.md`.

No system Java is needed. Concurrent runs on one cache are safe (advisory locks).
Keep run outputs outside the repository. The private oracle asset needs
`GH_TOKEN`, `GITHUB_TOKEN` or `gh auth login`.

## Tracking and docs

- GitHub milestones on `Chelis-Lang/dive` track phases; `docs/ROADMAP.md` is the
  narrative. One issue per deliverable or defect; every conformance disagreement
  becomes an issue with a minimal reproducing spec. Close issues by hand after
  verifying the fix.
- Owner questions go in `docs/open-questions.md`. Add to it rather than guessing.
- Docs describe the current state, use repo-relative paths only (no absolute
  local paths or personal emails), and follow
  `docs/developer/documentation-policy.md`. Public APIs carry rustdoc with
  examples.
