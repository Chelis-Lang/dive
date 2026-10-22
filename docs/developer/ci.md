# CI

`.github/workflows/ci.yml` runs three jobs on every PR and every push to
`main`. Their names are the required checks applied by `cargo xtask
protect-main`:

- **No AI authorship markers**: `scripts/check-ai-markers.sh` over the pushed
  or PR commits.
- **Workspace gate**: one job and one compile. It runs rustfmt, clippy, the
  nextest `ci` profile, doctests, rustdoc with `-D warnings` and the layering
  check. `cargo deny` runs only when the
  PR changes `Cargo.lock` or `deny.toml` (a step condition, so the required
  check always reports), and on every push to `main`.
- **Conformance gate**: release builds of the harness and `dive`, then
  `scripts/conformance-gate.sh conformance/gates/smoke.txt --workers 1,4`.

The push run on `main` checks the merged result (branch protection is not
strict) and saves the rust-cache entries that PR runs restore.

## Self-hosted runners

Every job's `runs-on` reads the repository variable `DIVE_CI_RUNNER`. Unset,
jobs run on `ubuntu-latest`. Set to `dive-local`, they run on self-hosted
runners with that label, so required checks keep reporting when hosted
minutes are unavailable:

```sh
gh variable set DIVE_CI_RUNNER --body dive-local   # self-hosted
gh variable delete DIVE_CI_RUNNER                  # back to hosted
```

A `dive-local` runner is a stock `actions/runner` registered to this
repository with the label `dive-local`, running as an ordinary user with the
pinned toolchain on `PATH`. Each runner needs its own `CARGO_HOME` (set in the
runner's `.env`, with its `bin` first in the runner's `.path` and the user's
rustup proxies after it), and `CARGO_TARGET_DIR=target` if the user's
environment sets a shared target directory. `Swatinem/rust-cache` treats
`CARGO_HOME/bin` as disposable: its post step deletes binaries that existed
before the job, which on a shared `CARGO_HOME` removes the user's `rustup` and
installed tools. The workflows also pass `cache-bin: false` on self-hosted
runners as a second guard. Register one
runner per concurrent job (three cover one PR). The runners execute PR code
with that user's privileges, which is acceptable only because the repository
is private. Steps that need a hosted image, such as freeing preinstalled SDKs,
are skipped with `runner.environment == 'github-hosted'`.

## The `ci` nextest profile

`.config/nextest.toml` gives the `ci` profile a `default-filter` that leaves out
the slow tests: the checkpoint, spill, view-normalization, storage, PlusCal
checksum and disk-role evidence binaries, the periodic-snapshot worker matrix,
individual multi-worker or whole-model sweeps, and the evidence source guards
that run `scripts/check-*.py` (those pin hashes of repository files, including
this workflow). The selection was taken from per-test durations of main and PR
CI runs. It keeps about 1230 of 1768 tests and about 400 of 8385
test-seconds, about three minutes on the 2-vCPU hosted runner, with tests of
every crate.

The job sets `CARGO_PROFILE_DEV_DEBUG=0` and `CARGO_PROFILE_TEST_DEBUG=0` and
deletes unused preinstalled SDKs in the background: the whole test inventory
does not otherwise fit on the runner disk (the linker dies with SIGBUS).

Run the excluded tests alone with
`cargo nextest run --workspace --profile ci --ignore-default-filter`, or the
whole suite with the default profile.

## Local gates

The conformance harness owns each native command's process group on Linux and
macOS (ADR 0112). Its leader remains unreaped until bounded system-process
inspection finds no live group members. Cancellation permanently rejects new
work and drains the retained owners; completion and cancellation cannot send a
later signal through a reaped numeric group ID. A failed inspection or cleanup
is a harness error, not a verdict or an empty group. Keep the original raw output
and require external cleanup proof before accepting a campaign with such a
failure. Native model budgets do not bound independent validation inside the
harness process, so retain the outer campaign deadline.

Group inspection uses the fixed system `/bin/ps` with isolated, bounded leaf
observers. Cleanup can add up to five seconds per registered native group, plus
bounded preflight-observer cleanup. Allow that time for the harness to handle
SIGINT/SIGTERM before an outer supervisor kills it. Killing only the harness's
own group does not cover its separate native groups. Other targets retain
direct-child deadlines without a descendant-containment claim. Session escape,
external reapers and uninterruptible OS work still require honest HOLD/cleanup
handling; these controls are not a process sandbox.

```sh
cargo xtask gate --fast                 # what CI's Workspace gate runs
cargo xtask gate --full                 # whole suite, cm1 + validation-smoke at 1/4/8, small corpus tier vs baselines
cargo xtask gate --full --skip-corpus   # without the small corpus tier
```

`--full` keeps going after a failure and writes `target/gate-report.md` (under
`CARGO_TARGET_DIR` if set): each step's result and duration, and the corpus
tier's outcome counts per profile. The corpus tier runs the 156-model small
selection (examples, probes, validation-smoke, configured-temporal-operators)
at one worker in the `tlc` profile (models and modules) and the default `dive`
profile (models), each with `run --baseline` against its committed baseline
(see [Corpus baselines](#corpus-baselines)). A regression fails the step. Paste it into the PR body. It is required before PRs
that touch `dive-eval`, `dive-check`, `dive-value`, `dive-store`,
`dive-liveness`, `dive-model` or `dive-trace`. It needs `cargo-nextest`, and
`cargo-deny` when the dependency policy inputs change. On a shared machine,
set `CARGO_BUILD_JOBS` and `NEXTEST_TEST_THREADS`.

Repository tasks resolve the Cargo workspace from the invocation directory at
runtime. Invoking a task outside a Cargo workspace fails instead of falling back
to the driver's build directory. This selects the source workspace; it does not
establish that Cargo's cached dependencies came from that workspace.

Keep a separate build directory for each worktree. Set `CARGO_TARGET_DIR=target`
from the worktree root, or use a worktree-exclusive absolute directory. Unsetting
the variable alone may still select a shared `build.target-dir` in Cargo's user
configuration. Never run different worktrees against the same writable target.
We observed a model test linked against an evaluator library from another
worktree while the current worktree's evaluator unit tests passed. A fresh
isolated build passed the model tests. If a build directory has been shared,
rebuild in a fresh directory before relying on test or benchmark results; a
clean Git status and a successful Cargo command alone do not establish the
library's source identity.

`.github/workflows/conformance.yml` runs the small or deep tier, including the
TLC self-checks, on manual dispatch. Its `baseline` job runs nightly (and on a
small-tier dispatch): the same two corpus runs as the full gate, against the
committed baselines. It is not a required check.

## Corpus baselines

`conformance/baselines/small-<profile>-w<workers>.json` are compact digests
(`dive-conformance report baseline`) of small-tier reports on `main`: each
model's outcome and observables (verdict, error class and code, physical exit,
violated property, distinct and generated counts, depth) per worker count, and
each module's outcome. `dive-conformance run --baseline <file>` fails (exit 1)
when a model or module that agreed in the baseline no longer does, or any
model result's observables changed while the model does not agree. Improvements
(to `agree`, whose observables then match the golden) are listed and pass.

Refresh a baseline in the PR that improves it (or that deliberately changes an
observable, with the reason in the PR body), from a run of that PR:

```sh
dive-conformance run --impl dive --impl-profile tlc \
  --corpus examples --corpus probes --corpus validation-smoke \
  --corpus configured-temporal-operators --tier small --workers 1 \
  --tlc-budgets --budget-cap 900 --report-dir "$OUT" --no-summary
dive-conformance report baseline "$OUT"/<run-id>.json \
  --out conformance/baselines/small-tlc-w1.json
```

The default profile adds `--no-sany` and writes `small-dive-w1.json`.
`report summarise` regenerates `docs/conformance.md` from the same reports.

The model gates and nightly runs pass `--tlc-budgets --budget-cap 900`
(ADR 0107): each model gets `max(60 s, 10 × golden TLC elapsed)`, hard-capped at
900 s. Both dive profiles receive explicit
time and memory budgets, and the harness allows a further 30 s only for
uninterruptible work before killing the process. A budget stop is a resource
finding, never agreement or an unsupported feature. These runs do not depend
on the release-time default limit.
