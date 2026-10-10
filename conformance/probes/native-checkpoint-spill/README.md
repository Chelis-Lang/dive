# Native checkpoint recovery with transient storage

The `before/` six CLI captures and 26 exact source files retain the frozen
parallel implementation's frontier/log/both admission refusals in both profiles.
Those are dive limitations, not TLC defects. Original parallel receipts and
oracle goldens remain unchanged.

The continuation admits the same pure finite safety models with either existing
transient budget, both or neither. Recovery inserts configured reduced keys and
original parent/ordinal records in order, streaming only the saved frontier
suffix into Queue. It creates no second complete decoded frontier or RAM disk
offset index. Snapshot discovery metadata, serialization, visited/candidate and
worker memory remain outside both budgets. The archive's separate 64 MiB and
65,536-state/name admission bounds remain. No aggregate-memory, Java archive
interoperability, authenticity or independent reachability claim is made.

Capture the complete selection into a fresh directory:

```sh
python3 -I conformance/probes/native-checkpoint-spill/capture_recovery.py DIVE FRESH_OUTPUT
```

The original 144 groups are retained unchanged in the memory family. The same
selection adds frontier-only, predecessor-only and both-zero families: all three
pure VIEW/symmetry cases, both profiles/stores, 1/4/8 widths, chaos off/19 and
barriers 0/1. In total 576 groups account for 4,032 CLI runs and 1,728 independent
positive/altered concrete witness controls. Every spill full/Retain result equals
its memory counterpart; default TLC reset preserves the original counter rule.
Zero-budget retained bytes and successful cleanup are checked independently.
Current execution credit is recorded only after the complete fresh run finishes.

Rust controls also change budgets/width on actual recovery, use tiny capacities,
reject corrupted archives and force blocked storage directories. The close-time
control commits real native bytes, removes actual queue/log backing files before
close, and proves the receipt retains those bytes while typed finalized cleanup
is false. The existing engine finalizer controls separately prove storage failure
replaces a deciding verdict with resource-limit and retains the deciding event;
no diagnostic substring selects behavior. Both successful and failed cleanup
facts are explicit. A committed/imported archive is never a model certificate.

The first cleanup test did not compile because it used incorrect helper and
value/option constructor spellings. `development/first-cleanup-test-build/`
retains that source and build log; correcting the spellings changed no production
behavior or assertion. Runtime effects/names, checker reads, deferred evaluator
values, temporal graph state and broader family certification remain pending.
F-032 stays partial. See the runnable [CLI/Rust guide](../../../docs/native-checkpoint.md#recovery-with-transient-disk-storage).

The first complete local gate ran every test: 856 of 857 passed. Its CLI
checkpoint-option test still expected frontier/log byte budgets to be refused.
The implementation now admits those options and correctly reaches the missing
input diagnostic. `development/first-complete-workspace-gate/` retains the exact
old test, full log and raw missing-input captures; `before-cli-contract-authorities/`
retains the pre-repair manifests and exact source bytes. The repaired CLI control
requires exit 2, the input error, empty stdout and no archive for frontier, log
and both budgets, while retaining temporal/orphan-option refusals. No production
code or oracle expectation changed for that repair.

## Completed frozen controls

`delivery.json` pins the runtime producer (`d7a7349`) and release binary, the
separate verification tree (`deeab34`), 257 archived implementation inputs,
raw/source/trace/archive hashes and completed receipts. The complete selection
passed: 576 groups, 4,032 CLI calls, 1,728 independent witness controls and 96
storage/corruption/admission failure controls. The runnable guide's three CLI
commands and Rust example also executed successfully; their source, outputs and
archive hashes are retained in `guide/`.

The fixed full local gate passed all 858 tests, Python/source checks, Clippy,
doc tests, warnings-denied documentation, layering, standalone trace and
dependency checks. CM1's 21 and validation-smoke's 16 models fully agreed at
1/4/8 workers. `receipts/timings.json` retains per-test times and observed load:
nextest took 1,579.653 seconds; the entire gate took about 1,720 seconds with two
local test threads and two compile jobs. Every 180-second per-test bound is
unchanged. New recovery wrappers took 32.324–83.463 seconds under concurrent
local load. These are functional controls, not benchmarks or a prediction of
Linux CI runtime; CI's workspace job remains 15 minutes and uses runner-default
nextest width. Reusing immutable parse/analyze/IR preparation may reduce repeated
setup, but startup binding/effects and evaluator-specific caches must stay
per-run unless a separate proof permits reuse. No control was dropped.

The later CLI test-only restart initially failed Clippy's `assert_is_empty`
style check; `development/cli-contract-clippy/` retains that source, log and
manifests. Exact empty-byte equality repaired diagnostics without changing the
assertion or production behavior. Historical focused logs are retained and not
credited as final-source executions. Original memory receipts, primary oracle
results, failed gates and pre-repair source authorities remain separate.

Capture the failure selection from the repository root into a fresh directory:

```sh
python3 -I conformance/probes/native-checkpoint-spill/capture_failures.py DIVE FRESH_OUTPUT
```

F-032 remains partial/audit-required. The native snapshot and its corruption
checksum are trusted-local recovery inputs, never an independent reachability
certificate; neither transient budget bounds total search or archive memory.

## Current combined-source verification (2026-10-11)

[`current-shipping/delivery.json`](current-shipping/delivery.json) pins the new
`7872bf6` runtime and release executable separately from the original frozen
producer above. The existing parallel engine, graph capture and production CLI
are unchanged from its integrated baseline. All 576 recovery groups, 4,032 CLI
calls, 1,728 independent witness controls and 96 failure controls passed again;
10,609 raw/trace/archive/source hashes verified. Three CLI guide commands and
the maintained Rust guide ran against this combined implementation. Its 254
implementation inputs and exact historical authority bytes are archived.

The full gate passed 916 tests, all 15 Python/source scripts, documentation,
layering, standalone trace and dependency checks. CM1 21/21 and validation-smoke
16/16 fully agreed at 1/4/8 workers. Nextest took 1,314.189 seconds at two local
threads; total gate time was 1,915.979 seconds, including cold compilation/docs.
The 18 spill matrix wrappers took 20.344–85.079 seconds; their aggregate test
work plus the two failure/cross-budget tests was 867.460 seconds. All 180-second
per-test bounds and selected cases remained unchanged. Release compilation,
recovery campaigns and conformance ran concurrently; recorded load and individual
times are functional evidence, not Linux runtime or performance claims.

The lead reported a following main Workspace job exceeding its unchanged
15-minute budget without a failed test; publication is held for scheduling repair
that preserves the complete selection. No test, oracle, assertion or timeout was
weakened here. Immutable preparation reuse is a possible future optimization;
startup/binding, ordered names and evaluator-specific caches remain per-run.
This study leaves F-032 partial/audit-required and preserves every earlier
producer/source/raw receipt. Capture this integrated selection with
`capture_shipping_recovery.py` and `capture_shipping_failures.py` into fresh
output directories; original capture scripts and receipts remain historical.
