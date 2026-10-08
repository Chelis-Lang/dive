# AGENTS.md

Operating guide for humans and coding agents working on **dive** (D.I.V.E., the
Deterministic Invariant Verification Engine), a ground-up Rust implementation of
the TLA+ toolchain. `CLAUDE.md` is a symlink to this file.

`docs/CHARTER.md` is the project charter, and this file must stay consistent with
it. When the charter and a later instruction conflict, ask the owner before acting.
The charter's appendix records factual corrections made since it was written.

## Ground rules

- **Soundness first.** Conformance with the pinned TLC/SANY is the definition of
  correctness. Anything `dive` does not implement exactly must yield an explicit
  `unsupported` result naming the feature. It must never approximate and never
  report success. Never weaken a test, golden file or conformance check to make a
  change pass. If a check looks wrong, raise it with evidence.
- **Goldens are generated, never edited.** Files under `conformance/golden/` come
  only from the harness, run against a pinned oracle, with a recorded `--reason`.
- **Surprising TLC behaviour:** write the smallest spec that shows it, add it to
  the corpus with the oracle's result, and document it before you implement it.
- **Ports follow TLC's semantics, not its structure.** Record provenance for every
  ported algorithm in `docs/provenance.md`, naming the Java class or method matched.
- **Divergences** from TLC are opt-in, named, documented in `docs/divergences.md`,
  and tested in both modes.
- **Decisions** go in numbered ADRs in `docs/decisions/` (copy `0000-template.md`).
  Changing the crate dependency graph is a decision.

## Commit messages and authorship

Use plain conventional commits (`feat:`, `fix:`, `docs:`, `chore:`, `test:`,
`refactor:`, `perf:`, `ci:`) under the configured human author. Commit messages
and PR bodies must not contain Claude, Codex, Copilot or other AI attribution,
AI co-authorship trailers, "Generated with" lines, or links to AI sessions.

Three layers enforce this:
1. **Local hook.** `.githooks/commit-msg`, installed by `cargo xtask install-hooks`.
   Run this once per clone. It covers every worktree.
2. **CI.** The `No AI authorship markers` job runs `scripts/check-ai-markers.sh`
   over every pushed or PR commit, checking messages, author and committer.
3. **Agent config.** `.claude/settings.json` disables Claude Code's attribution.
   This overrides any default instruction to add `Co-Authored-By` lines.

Never bypass the hook with `--no-verify`.

## Workflow mode

**Current mode: PR mode.** This mode has applied since 2026-10-08, once parallel agent work began.

`main` is protected. Every change lands through a squash-merged PR, and the PR must pass two required checks: `No AI authorship markers` and `Workspace gate`. Admins are not exempt.

**Keep `main` green.**
- Run the full local gate (see Gates) before you push.
- Never merge on red.
- If `main` goes red, fixing it takes priority over new work. Label the tracking issue `failing-on-main`.

Each PR states:
- which conformance entries it affects;
- the harness results.

Agents working in parallel use separate worktrees and branches. The lead merges their work.

Protection is applied by `cargo xtask protect-main`, which is idempotent.

## Repository layout

```
crates/dive-*        one crate per charter component (see docs/CHARTER.md)
xtask/               cargo xtask: install-hooks, check-layering, protect-main
conformance/         oracle and corpus pins, generated goldens, reports
docs/                charter, roadmap, ADRs, survey, divergences, provenance,
                     open questions, conformance summary (generated)
scripts/             small shell helpers used by hooks and CI
.githooks/           tracked git hooks
```

**Layering.** `xtask/src/layering.rs` is the single table of permitted internal
dependency edges, and `cargo xtask check-layering` enforces it in CI. One
structural rule matters for soundness: `dive-trace` (the independent trace
validator) must not depend on `dive-check`, `dive-liveness`, `dive-sim` or
`dive-store`.

## Rust conventions

- **Toolchain.** Pinned exactly in `rust-toolchain.toml` (1.99.0). MSRV is
  `rust-version` in `Cargo.toml`. Bump both together, on purpose.
- **Edition and features.** Edition 2024, stable only, no nightly features.
- **Unsafe code.** `unsafe_code = "forbid"` workspace-wide, and every crate root
  also has `#![forbid(unsafe_code)]`. Any exception needs an ADR with owner
  approval, an isolated module, a written safety argument, and Miri tests.
- **Lints.** Configured in `[workspace.lints]`: `clippy::all` and
  `clippy::pedantic` deny, plus `unwrap_used`, `expect_used`, `panic`, `todo`,
  `unimplemented` and `missing_docs`. Document any local `#[allow]` with a reason.
  `clippy.toml` lists domain words for `doc_markdown`.
- **Errors.** No panics on user input. User-facing errors carry a source span and
  a stable error code. Internal invariant violations are reported as bugs, with
  enough context to reproduce them.
- **Dependencies.** Add third-party crates to `[workspace.dependencies]` first and
  reference them with `workspace = true`. `deny.toml` holds the licence allowlist
  and source policy.
- **Dependency hygiene.** Keep it tight; every new dependency should earn its place.

## Gates

Run the gate locally before pushing. This is exactly what CI's `Workspace gate` runs:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo nextest run --workspace --locked          # or: cargo test --workspace
cargo test --workspace --doc --locked
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
cargo xtask check-layering
cargo deny --locked check
```

**Testing tiers** (charter §Engineering practice):
- *Fast:* the gate above. It runs on every push and PR.
- *Standard:* `.github/workflows/conformance.yml` runs nightly and on manual
  dispatch, over the small-model corpus. It becomes a PR gate on a small subset
  once `dive` produces verdicts.
- *Deep:* the same workflow with `tier=deep`, run on manual dispatch and before
  releases.
- *Not in CI:* benchmarks, profiling and long fuzzing. Run them by hand with
  `dive-bench` and `cargo build --profile profiling`, and record machine details
  with the results.

## Conformance harness

See `crates/dive-conformance/README.md` and `docs/decisions/0002`–`0005`. In
short, the harness:
- fetches the pinned JDK, `tla2tools.jar` and corpora, verifying each by SHA-256,
  into `$DIVE_CACHE_DIR` (default `~/.cache/dive`);
- generates goldens from TLC and SANY;
- runs an implementation (TLC itself, `dive`, or an external tool through an
  adapter) against the goldens;
- emits a JSON report and `docs/conformance.md`.

No system Java is needed.

## Tracking

- **Milestones.** GitHub milestones M0–M5 on `Chelis-Lang/dive` track phases.
  `docs/ROADMAP.md` is the narrative.
- **Issues.** One issue per deliverable or defect. Every conformance disagreement
  becomes an issue containing the minimal reproducing spec.
- **Closing issues.** Close issues by hand after verifying the fix. Don't rely on
  `Closes #N`.
- **Owner questions.** Open questions for the owner live in
  `docs/open-questions.md`. Add to it rather than guessing.

## Documentation style

Docs describe the current state. Use repo-relative paths only: no absolute local
paths and no personal emails. Public APIs carry rustdoc with examples. Record
user-visible changes in `CHANGELOG.md`.
