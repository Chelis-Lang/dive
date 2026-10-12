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
the slow tests: the checkpoint, spill, view-normalization and storage evidence
binaries, the periodic-snapshot worker matrix, and individual multi-worker or
whole-model sweeps. The selection was taken from per-test durations of a main
CI run on 4-vCPU runners. It keeps about 1300 of 1768 tests and about 535 of
8385 test-seconds, and at least one test binary of every crate.

Run the excluded tests alone with
`cargo nextest run --workspace --profile ci --ignore-default-filter`, or the
whole suite with the default profile.

## Local gates

```sh
cargo xtask gate --fast                 # what CI's Workspace gate runs
cargo xtask gate --full                 # whole suite, cm1 + validation-smoke at 1/4/8, small corpus tier
cargo xtask gate --full --skip-corpus   # without the small corpus tier
```

`--full` keeps going after a failure and writes `target/gate-report.md` (under
`CARGO_TARGET_DIR` if set): each step's result and duration, and the corpus
tier's outcome counts. Paste it into the PR body. It is required before PRs
that touch `dive-eval`, `dive-check`, `dive-value`, `dive-store`,
`dive-liveness`, `dive-model` or `dive-trace`. It needs `cargo-nextest`, and
`cargo-deny` when the dependency policy inputs change. On a shared machine,
set `CARGO_BUILD_JOBS` and `NEXTEST_TEST_THREADS`.

`.github/workflows/conformance.yml` runs the small or deep tier, including the
TLC self-checks, on manual dispatch only.
