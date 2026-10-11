# Workspace CI scheduling

The protected `Workspace gate` requires successful authorship checks, all
`Workspace checks`, and each of eight `Workspace tests` jobs. Every matrix job
executes one disjoint hash partition of the full workspace suite. A failure,
timeout, cancellation, skipped prerequisite or missing result fails the gate.

Run the complete local gate in [AGENTS.md](../../AGENTS.md) before pushing.
To reproduce partition coverage and one CI partition:

```sh
python3 -I scripts/check-test-partitions.py
python3 -I scripts/test-workspace-gate.py
cargo nextest run --workspace --locked --profile ci --partition hash:1/8
```

Repeat the last command with partitions 2/8 through 8/8 to run all tests locally.
The coverage verifier queries nextest for the full inventory and each actual
partition, rejects empty inventories, duplicate tests, missing tests and extra
tests, and reports each partition's selected count. It also verifies ignored
tests stay unselected and no partition introduces another filter reason.
No count is hard-coded: newly added workspace tests must be covered too.

Each test job and Workspace checks retain their 15-minute budget. Nextest CI
retains its three 60-second slow-test periods and `fail-fast = false`. The
required aggregation job has five minutes and performs no test selection.
The dependency check controls exercise all 125 combinations of success,
failure, cancellation, skipped and missing prerequisite results.

The initial cancellation at main 8b0b7db is recorded in issue #173 and the
[workspace ADR](../decisions/0001-workspace-toolchain-and-ci.md). It is not a
passing suite receipt. Partitioning changes scheduling, not TLC conformance
coverage, generated goldens or the user-facing checker contract.

The runtime-name checkpoint matrix has separate store/worker tests so each
retains the existing 180-second bound on Linux. All original profile, chaos,
pause, recovery-counter and witness assertions remain. PR #179’s initial timing
failure and a mechanical equality check of the original assertion body are
retained in `conformance/probes/native-parity-integration/current-controls/full-runtime-ci-before-scheduling-repair/`.

Splitting a matrix must preserve comparison scope as well as assertions. Each
runtime-name wrapper now obtains actual fingerprint and same-store reference
runs at one worker, and retains the original store/width index for its spill
family. The first split lost those shared comparisons; its interrupted source
and gate remain in `current-controls/full-runtime-ci-split-before-reference-repair/`.
The corrected matrix equality includes all 360 spill-family/boundary groups.
