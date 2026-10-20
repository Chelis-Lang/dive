# CI

`.github/workflows/ci.yml` runs three jobs on every PR and every push to
`main`. Their names are the required checks applied by `cargo xtask
protect-main`:

- **No AI authorship markers**: `scripts/check-ai-markers.sh` over the pushed
  or PR commits.
- **Workspace gate**: one job and one compile. It runs rustfmt, clippy, the
  nextest `ci` profile, doctests, rustdoc with `-D warnings`, the layering
  check and a standalone `dive-trace` build. `cargo deny` runs only when the
  PR changes `Cargo.lock` or `deny.toml` (a step condition, so the required
  check always reports), and on every push to `main`.
- **Conformance gate**: release builds of the harness and `dive`, then
  `scripts/conformance-gate.sh conformance/gates/smoke.txt --workers 1,4`.

The push run on `main` checks the merged result (branch protection is not
strict) and saves the rust-cache entries that PR runs restore.

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
  --timeout 1800 --report-dir "$OUT" --no-summary
dive-conformance report baseline "$OUT"/<run-id>.json \
  --out conformance/baselines/small-tlc-w1.json
```

The default profile adds `--no-sany` and writes `small-dive-w1.json`.

`conformance/baselines/full-tlc-w1.json` is the same digest for the full tier
(`--tier all --no-sany --timeout 3600`, TLC profile, one worker). No gate uses
it; it records the dated full-tier row of `docs/scoreboard.md`, and a full-tier
run can be checked against it with `--baseline`.
`report summarise` regenerates `docs/conformance.md` from the same reports.
