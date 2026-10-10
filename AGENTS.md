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
- **Better by default where approved, TLC-compatible always built and proven**
  (ADR 0010).
  - `--compat=tlc` is the single global switch (also `DIVE_COMPAT=tlc` and the
    config key) that makes *every* behaviour precisely TLC-equivalent, including
    defaults like the worker count, seeding, exit codes and output conventions.
    Host access is the owner-approved exception: spec I/O requires explicit category
    grants even in compat mode, and relevant restrictions must be reported (D-017).
    Full conformance is enforced on supported behavior. Every new option must declare its
    TLC-equivalent setting.
  - Wherever TLC imposes a handcuff, build a named dive option that does better:
    single-worker exactness, unseeded or unreplayable randomness, 32-bit
    integers, silent soundness hazards, or human-only output. "Better" means
    faster, more deterministic, or more useful to agents and Rust callers.
  - Never let an oracle limitation become a dive limitation. Goldens use
    `-workers 1`, but dive itself must scale wide and stay deterministic.
  - **Result-preserving enhancements** (same answer, better path) are on by
    default, once the harness shows they equal the TLC golden at every setting.
  - **Result-changing enhancements** are opt-in unless the owner approved them as
    the default (see the registry). Their TLC-matching counterpart is always
    built, and is proven against the goldens under `--compat=tlc`.
  - Propose new result-changing defaults in `docs/open-questions.md`. Never adopt
    one silently.
  - **Nondeterminism must be replayable.** Always report the seed, and make
    `--seed` reproduce the run exactly.
  - **Register every handcuff you find** in `docs/divergences.md`, with
    minimal-spec evidence, before or alongside the implementation.
  - **Keep `docs/tlc-differences.md` current.** This is the user-facing log of
    every place where TLC does something wrong or suboptimal and dive does
    something different. In the same PR as the registry change or the code:
    - Each registry row in `docs/divergences.md` gets a stable `D-NNN` entry.
    - The entry contains a minimal example, what TLC does (measured on the pinned
      oracle, with numbers), why it is wrong or suboptimal, dive's default, the
      switch that restores TLC behaviour, and the status and evidence.
    - Update the status as the code moves from planned to implemented to default.
    - Never reuse an ID. Mark withdrawn entries as *withdrawn*.
    - Record dive's own limitations relative to TLC there too, as honest
      refusals.
    - Write for TLA+ users, not for dive developers.
- **Divergences** from TLC are opt-in, named, documented in `docs/divergences.md`,
  and tested in both modes.
- **Decisions** go in numbered ADRs in `docs/decisions/` (copy `0000-template.md`).
  Changing the crate dependency graph is a decision.
- **Keep parity accounting current.** `docs/tlc-parity.md` and
  `conformance/parity-features.json` record stable semantic feature families,
  acceptance obligations and evidence. Update affected families and source
  surfaces in the same PR as an implementation. Registry entries, parsed
  flags, mixed-profile matches and explicit refusals are not certification.
  Preserve all selected cases and do not invent an overall completion percent.
  The approved delivery sequence is `docs/design/tlc-parity-plan.md`.

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

`main` is protected. Every change lands through a squash-merged PR, and the PR must pass three required checks: `No AI authorship markers`, `Workspace gate` and `Conformance gate`. The conformance gate runs `dive check --compat=tlc` against the TLC goldens for the models in `conformance/gates/cm1.txt`, at 1, 4 and 8 workers. Admins are not exempt.

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
`dive-store`. It must also build without `dive-eval`'s `enumerate` feature, the
successor enumerator. `check-layering` resolves the features a standalone
`dive-trace` build enables, and the gate builds `dive-trace` on its own so that
workspace feature unification cannot hide a violation (ADR 0016 §2).
`dive-model`, the binding stage (ADR 0013 §8), is shared by every backend.

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

Run the gate locally before pushing. CI's required `Workspace gate` aggregates
`Workspace checks` and all four `Workspace tests` partitions. Every prerequisite
must succeed; failed, cancelled or skipped jobs fail the required check. The
combined jobs run the same full gate below, with tests partitioned using
`--profile ci --partition hash:N/4` for N = 1, 2, 3, 4.
`scripts/check-test-partitions.py` verifies their disjoint union equals the full
workspace suite. Test and job time limits remain unchanged:


```sh
cargo fmt --all --check
python3 scripts/test-validation-import.py
python3 -I scripts/test-frontend-campaign.py
python3 -I scripts/check-parity-ledger.py
python3 -I scripts/test-parity-ledger.py
python3 -I scripts/check-tlc-planning.py
python3 -I scripts/check-tlc-tableau.py
python3 -I scripts/check-temporal-action-order.py
python3 -I scripts/check-phase-l.py
python3 -I scripts/check-phase-l-graph.py
python3 -I scripts/check-phase-execution.py
python3 -I scripts/check-tlc-final.py

python3 -I scripts/check-state-if-planning.py
python3 -I scripts/check-predecessor-spill.py
python3 -I scripts/check-native-checkpoint.py
python3 -I scripts/check-native-parity-integration.py
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo nextest run --workspace --locked          # or: cargo test --workspace
cargo test --workspace --doc --locked
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
cargo xtask check-layering
cargo build -p dive-trace --locked              # standalone: no feature unification
cargo deny --locked check
```

**Testing tiers** (charter §Engineering practice):
- *Fast:* the gate above. It runs on every push and PR.
- *Standard:* `.github/workflows/conformance.yml` runs nightly and on manual
  dispatch, over the small-model corpus. Its PR-gating subset is
  `conformance/gates/cm1.txt`, run by the required `Conformance gate` job
  (`scripts/conformance-gate.sh conformance/gates/cm1.txt --workers 1,4,8`).
  The same job checks all 16 generated ValidationTestSuite smoke models at
  those widths (`DIVE_GATE_CORPUS=validation-smoke` and
  `conformance/gates/validation-smoke.txt`). Every model must fully agree.
- *Deep:* the same workflow with `tier=deep`, run on manual dispatch and before
  releases. Its separate frontend job runs deterministic syntax/layout/semantic
  mutations and both SANY-case directories through the pinned oracle. See
  `docs/frontend-campaign.md`; preserve the complete selection and raw reports.
- *Not in CI:* benchmarks, profiling and long fuzzing. Run them by hand with
  `dive-bench` and `cargo build --profile profiling`, and record machine details
  with the results.

## Conformance harness

See `crates/dive-conformance/README.md` and `docs/decisions/0002`–`0005`. In
short, the harness (`cargo run -p dive-conformance --release -- <command>`):
- fetches the pinned Temurin JRE, the `tla2tools.jar` oracles, the
  CommunityModules jars, the TLAPS and Apalache module libraries and the corpora
  into `$DIVE_CACHE_DIR` (default `~/.cache/dive`). Jars and archives are
  verified by SHA-256 on every use; checkouts by commit (`oracle fetch`,
  `corpus fetch`);
- generates goldens from TLC and SANY (`golden generate`). Overwriting a golden
  needs `--reason`;
- runs an implementation against the goldens (`run --impl tlc|tlc:<id>|dive|adapter:<toml>`).
  The implementation is TLC itself, `dive`, or an external tool through an adapter;
- emits `conformance/reports/<run-id>.json` and `docs/conformance.md`.

No system Java is needed; `--java <path>` opts into one and is recorded in
provenance.

**Parallel agents use their own cache.** Concurrent harness runs on one cache
are safe (cache entries are created under advisory file locks), but a shared
checkout is still shared state. Each worktree sets its own `DIVE_CACHE_DIR`,
seeded by copying the shared cache, e.g.
`rsync -a --exclude tmp ~/.cache/dive/ "$DIVE_CACHE_DIR/"`; the harness
verifies the copy before using it. Never write into a cache checkout by hand.
See the harness README, "Cache layout". The private oracle asset needs `GH_TOKEN`, `GITHUB_TOKEN` or
`gh auth login`.

## Tracking

- **Milestones.** GitHub milestones M0–M5 on `Chelis-Lang/dive` track phases.
  `docs/ROADMAP.md` is the narrative.
- **Issues.** One issue per deliverable or defect. Every conformance disagreement
  becomes an issue containing the minimal reproducing spec.
- **Closing issues.** Close issues by hand after verifying the fix. Don't rely on
  `Closes #N`.
- **Owner questions.** Open questions for the owner live in
  `docs/open-questions.md`. Add to it rather than guessing.

## Documentation delivery

Documentation is part of implementation, not a separate phase that pauses other
work. Every user-visible feature/change updates its user-facing Markdown guide
in the same PR: exact CLI/API invocation, runnable example, outputs/exits,
defaults/TLC counterparts, supported/refused combinations and resource/proof
qualifications. Keep `docs/user/README.md` current and link new feature pages.
Maintain developer design, provenance, known TLC issues, parity obligations and
historical evidence alongside the user contract. ADRs and experiment READMEs
alone do not replace a user guide. Follow `docs/developer/documentation-policy.md`.

## Documentation style

Docs describe the current state. Use repo-relative paths only: no absolute local
paths and no personal emails. Public APIs carry rustdoc with examples. Record
user-visible changes in `CHANGELOG.md`.
