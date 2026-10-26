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
observers. Each drain attempt has a five-second budget per registered native group, plus
bounded preflight-observer cleanup. Error/unwind guards can make a further
bounded attempt; cancellation may serialize with one already in progress. Allow
that cleanup headroom for the harness to handle
SIGINT/SIGTERM before an outer supervisor kills it. Killing only the harness's
own group does not cover its separate native groups. The harness's documented
host scope is Linux/macOS; child execution elsewhere fails before launch rather
than silently running without group cleanup. Session escape,
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

## Metamorphic tests

`crates/dive-api/tests/metamorphic.rs` checks the charter's metamorphic rule in
the default profile. Each seed is a fixture module with a same-named
configuration. The test rewrites it through the lossless syntax tree and
checks both versions. It renames module-level symbols (to names of the same
length, so junction-list layout is kept), reorders definitions in a random
dependency order, adds comments, adds unused definitions with strings and a
string `CHOOSE`, and splits the module into an extended base and the rest.
These must reproduce the verdict, exit class, property, statistics,
diagnostic codes, printed output and every counterexample state. Reordering
`Next` disjuncts or configured invariants may change which same-depth
violation is reported, so those compare the verdict, the depth and complete
statistics. Reordering an invariant's conjuncts is exact unless exactly one
side fails with an evaluation error, as a reordered guard does in TLC.
Source locations are normalized away. Runs stop at 20,000 states or ten
seconds, and a stopped pair is inconclusive.

The `ci` profile runs a fixed list of 43 seeds covering every verdict and
relation. To check every eligible fixture (about 1,350 seeds, three minutes in
a release build):

```sh
DIVE_METAMORPHIC=all DIVE_METAMORPHIC_REPORT="$TMPDIR/metamorphic.json" \
  cargo test -p dive-api --release --test metamorphic -- every_fixture --nocapture
```

The report lists each disagreement with the rewritten files. Two known
text dependences are reported separately and do not fail the sweep. The
start-up random stream draws constant definitions in definition and module
order (`RndStartup`), so reordering or splitting changes random values. A
failing constant definition's `Print` runs again for each module in the
`EXTENDS` chain (`SuPrintRetry`), so a split changes the printed output.

## Counterexample validation

`crates/dive-api/tests/counterexample_validation.rs` checks every
counterexample `dive check` reports with the independent validator, in both
profiles. The harness does this for corpus models with goldens; this test
covers every fixture spec with a same-named configuration, goldens or not.
Each run uses seed 0, fp 0, one worker, the default store (the exact store
refuses liveness) and stops at 20,000 states or ten seconds. A trace must
end as the report's verdict says and validate. A trace the validator reports
partly unsupported, or a run without a trace document, is counted but does
not fail.

The `ci` profile runs a fixed list of seeds covering each ending. To check
every fixture (about 1,400 seeds, both profiles, a few minutes in a release
build):

```sh
DIVE_CX_VALIDATION=all DIVE_CX_VALIDATION_REPORT="$TMPDIR/cx-validation.json" \
  cargo test -p dive-api --release --test counterexample_validation -- every_fixture --nocapture
```

Three known refutations of correct counterexamples, each matching TLC's own
trace, are reported separately and do not fail the sweep:

- a claimed evaluation error is re-raised while the validator checks the
  last state's invariants, constraints, action constraints or implied action
  properties, and the trace is refuted (#490);
- a last state outside `CONSTRAINT` is accepted only for an invariant
  violation, so action-property violations and error endings on such a state
  are refuted (`ErInvConstrained`, `DfidActionConstrained`; #499);
- checking `Init` raises on a disjunct whose empty product or record set
  TLC's generator skips (`EmptyProductSkipsMixedInit`; #443).

`known_refutations_still_reproduce` fails once one is fixed, so its entry can
be removed.

## Output agreement

`crates/dive-api/tests/output_agreement.rs` checks that the human and JSON
reports of one `dive check` run agree: the header's profile, store, seed and
workers, the verdict line and violated properties, the statistics, the
counterexample (state count, each state, the loop-back state), the printed
output in order and the warnings. Both renderings come from the same report,
so a difference is a rendering defect. It does not compare dive with TLC:
#489, where the property names differ from TLC's but agree between the two
reports, is a parity issue.

The `ci` profile runs a fixed list of seeds covering each verdict family. To
check every fixture in both profiles:

```sh
DIVE_OUTPUT_AGREEMENT=all DIVE_OUTPUT_AGREEMENT_REPORT="$TMPDIR/output-agreement.json" \
  cargo test -p dive-api --release --test output_agreement -- every_fixture --nocapture
```

One known difference is reported separately and does not fail the sweep: a
temporal evaluation error's JSON report carries the counterexample to the
failing state, while the human report says only that capture completed
(`ConcatFirstTypeError`; #500). `known_differences_still_reproduce` fails once it is
fixed.

## Directional metamorphic tests

`crates/dive-api/tests/metamorphic_directional.rs` changes a model in ways
whose effect is known in direction, in the default profile:

- `CONSTRAINT TLCGet("level") < k`: the distinct count never decreases as `k`
  grows and reaches the unbounded count; a violation, once found, keeps the
  unbounded depth;
- `INVARIANT TLCGet("level") < k` on a passing model: the counterexample grows
  by one state per larger `k` until `k` passes the model's depth;
- an invariant replaced by `TRUE`: a passing model keeps its counts, another
  invariant's violation keeps its depth, and dropping the violated invariant
  finds nothing shallower.

Each change is made in a wrapper module that extends the seed's root module,
`TLC` and `Naturals`. Seeds whose unchanged wrapper reports differently are
skipped (mostly roots whose helper modules follow them in the same file and
so are not found from the wrapper), as are temporal properties, models
without a behavior, and seeds whose own run takes over a second: the memory
budget measures the whole test process. A run dive refuses as `unsupported`
(D-002 random replay) is inconclusive.

The `ci` profile runs a fixed seed list. To check every fixture:

```sh
DIVE_DIRECTIONAL=all DIVE_DIRECTIONAL_REPORT="$TMPDIR/directional.json" \
  cargo test -p dive-api --release --test metamorphic_directional -- every_fixture --nocapture
```

## Fault injection

`crates/dive-api/tests/fault_injection.rs` checks that a fault ends in the
correct answer or an explicit safe stop, never a different verdict or counts
presented as complete. Each fixture is checked once unfaulted, then:

- transparent faults must reproduce the reference's verdict, property, TLC
  code and exit status, statistics, counterexample and output: chaos
  scheduling at 1, 4 and 8 workers, frontier, predecessor and seen-set spill
  with zero memory budgets, and a checkpoint pause at a completed level
  followed by recovery;
- disruptive faults may also stop (`stopped` or `resource-limit`, no
  counterexample, statistics no larger than the reference's) or refuse with
  an error: state, depth and time budgets at several points, missing and
  read-only spill or checkpoint directories, and checkpoint archives that are
  emptied, truncated, extended or have one byte flipped before recovery. A
  state budget may also keep the deciding error with smaller counts, since it
  bounds diagnostic replay too (see `docs/user/checking.md`).

An `internal-error` always fails; a mode dive refuses as `unsupported` is
inconclusive. Runs use the exact store, seed 0 and fp 0; seeds whose own run
takes over a second are skipped. The `ci` profile runs a fixed seed list. To
check every fixture:

```sh
DIVE_FAULTS=all DIVE_FAULTS_REPORT="$TMPDIR/faults.json" \
  cargo test -p dive-api --release --test fault_injection -- every_fixture --nocapture
```

One known finding is reported separately: a state budget that cuts
diagnostic replay short changes the reported TLC error class and exit status
without marking the run stopped (`ErInvReplayError` under `max_states` 4;
reported on #379). `known_findings_still_reproduce` fails once it is fixed.

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
