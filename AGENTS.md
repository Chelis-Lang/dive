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
  counterexample that `dive-trace` independently validates. Precisely registered
  owner-approved default differences may change only their declared observable
  fields; independent trace validation is never waived. Do not emulate
  arbitrary or nondeterministic TLC internals, such as timing, thread or queue
  order, tableau numbering, SCC or lasso choice, or Java method projections.
  Liveness names are the complete independently validated false-property set of
  the actual reported word; different valid words may name different sets (ADR 0126).
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
- **Better by default, nominal TLC compatibility** (ADR 0153, superseding
  ADR 0010's exact-compatibility requirement).
  - `--compat=tlc` (also `DIVE_COMPAT=tlc` and the config key) selects the
    existing inexpensive TLC-compatible switches on Core. Exact fidelity is not
    a release goal; unsupported cases refuse by name or retain canonical
    behaviour with a documented gap in `docs/tlc-differences.md`. Spec I/O still
    needs explicit grants in either profile (D-017).
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

## Standing direction (owner, 2026-11-05)

Read [ADR 0153](docs/decisions/0153-core-is-the-product.md) and
[docs/RELEASE.md](docs/RELEASE.md) before choosing work. The
[progress review](docs/reviews/2026-11-05-progress-review.md) explains why these
rules exist.

**Choose work by the release gates.**
- Every PR links an issue in the `v0.1: first value` milestone. The exceptions
  are a fix for a wrong user-visible result, or work the owner has signed off.
- Rank Core work by the refusal-reason histogram, largest bucket first. Do not
  rank it model by model.
- Frozen until v0.1:
  - distributed workers;
  - DAP;
  - checkpoint, spill and recovery breadth;
  - IOUtils/IOEnv;
  - TLC coverage and logical costs;
  - speculative performance caches;
  - LSP;
  - any Reference-engine feature.

**Do the better thing.** In the default profile, use canonical TLA+ semantics
where TLC's behaviour is an implementation artifact. Register the divergence.
Do not reproduce TLC internals: normalization history, demand order, receiver
identity or error-site certificates. Prefer correct, faster results over
suboptimal TLC bookkeeping; register the precise difference without asking the
owner to decide each artifact variant again. This standing authorization does
not relax soundness or excuse unrelated observable differences.

**Compat is nominal.** `--compat=tlc` runs on Core using the cheap switches that
already exist. Anything it cannot reproduce cheaply is a documented gap, not a
project.

**Measure the product.**
- Every week, run Core alone on the full tier and post the R1–R3 numbers to the
  pinned status issue.
- A PR body is at most 15 lines. It states which corpus IDs change and which
  focused checks ran.

**Keep the loop fast.**
- `cargo xtask gate --full`, the census and corpus/worker campaigns run only for
  a release candidate or an engine-cutover candidate. Otherwise there are no
  "phase" qualifications, checkpoint JSON, cloud archive ledgers or cleanup
  byte counts.
- At most three live worktrees. Prune each one when its PR merges.
- Cite only commits that are on `main`.

**Write less, decide more.**
- An ADR records a public contract, a default or an architecture edge, about one
  a week at most. A rejected experiment gets a short issue comment, not an ADR
  or a report.
- Status documents state the current numbers in plain sentences. Do not bind
  narrative text to commit or tree hashes, and keep history out of them; git has
  it.
- Do not open an issue only as a receipt for a PR.

**Escalate, don't emulate.**
- TLC bookkeeping variants follow the standing correct/faster default ruling;
  register their precise differences instead of proposing emulation machinery.
- Escalate genuine uncertainty about mathematical semantics or soundness in
  `docs/open-questions.md`; retain a named refusal while correctness is unproved.
- When data meets a condition the owner attached to a ruling, say so in the same
  way.

**No time or effort estimates** in plans, issues, milestones or status.

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
workers, then shadows `conformance/gates/shadow-smoke.txt` (the new core against
the reference engine); the full cm1 gate runs locally with `cargo xtask gate --full`. Keep `main` green: gate locally before pushing, never merge
on red, and fix a red `main` first (label the issue `failing-on-main`).

- Small, single-scope PRs from `main`. No long-lived integration or compose
  branches.
- Each PR states the conformance entries it affects and the harness results.
- Parallel agents use separate worktrees, branches and build directories; the
  lead merges. Set `CARGO_TARGET_DIR=target` from the worktree root, or use a
  worktree-exclusive absolute directory. Never share a writable Cargo target between
  worktrees. If a target has already been shared, rebuild in a fresh directory
  before using its results as correctness or performance evidence.
- Worktrees live only in `.claude/worktrees/<name>` (git-ignored), not in
  sibling directories. After the branch merges or is abandoned, run
  `cargo clean`, then `git worktree remove <path>` and `git worktree prune`.
  Review and phase archive directories keep sources and reports, never a
  `target/`: run `cargo clean` before archiving.
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
(ADR 0103). Both gates build `dive-trace` alone in a dedicated target so
workspace feature unification cannot hide a violation. `dive-model`
(ADR 0013 §8) is shared by every backend.

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
cargo build -p dive-trace --locked --target-dir target/standalone-trace
scripts/test-rust-consumer.sh                 # independent public API tests, empty PATH
cargo deny --locked check                       # only when Cargo.lock or deny.toml changed
```

The `ci` nextest profile (`.config/nextest.toml`) leaves out slow,
evidence-oriented and multi-worker matrix tests; everything it excludes still
runs in the full gate. The `Conformance gate` job checks the eight models of
`conformance/gates/smoke.txt` at 1 and 4 workers, then the Core golden gate: Core alone (`--engine core`, default
profile) must agree with the TLC goldens, or match a registered expected
divergence, on `conformance/gates/core-smoke.txt` at 1 and 4 workers. It then runs
`scripts/shadow-gate.sh` over `conformance/gates/shadow-smoke.txt`: the new
execution core (`--engine core`) must agree with the reference engine and its
counterexamples must validate (`docs/developer/core/08-correctness.md`).

**Full gate.** `cargo xtask gate --full` runs the fast gate with the whole
nextest suite, then `scripts/conformance-gate.sh` over `conformance/gates/cm1.txt`
and the validation-smoke corpus at 1, 4 and 8 workers, then the small corpus
tier in both profiles through the harness, failing on any regression against
`conformance/baselines/` (`--skip-corpus` leaves that out; see
`docs/developer/ci.md`). It writes
`target/gate-report.md`. Run it on the integrated milestone candidate and before release qualification;
record the exact source and paste the report into the milestone or release PR.
Do not launch it for every edit or incremental checker PR. On a
shared machine, set `CARGO_BUILD_JOBS` and `NEXTEST_TEST_THREADS`.

**Development loop** (owner correction, 2026-10-29): use focused tests and checks
with feedback in seconds to a few minutes, then integrate small working changes
through required CI. Broad qualification and major performance campaigns belong
at integrated milestone and release-candidate boundaries. Reuse passing evidence
within its actual source/scope; do not relabel it as a newer source or replace
implementation progress with additional prerequisite splits.

**Testing tiers** (charter §Engineering practice):
- *Fast:* CI's `Workspace gate` and `Conformance gate`, on every PR and push to
  `main` (`cargo xtask gate --fast` locally).
- *Standard:* `cargo xtask gate --full`, on the integrated milestone candidate
  and before releases. Every gate model must fully agree. `.github/workflows/conformance.yml`
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
