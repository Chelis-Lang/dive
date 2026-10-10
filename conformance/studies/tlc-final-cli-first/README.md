# First full small-selection final/TLC census

This is the unchanged 140-model small Examples selection run on frozen source
`72f8ae9023603f00f9ac7b5e181b1960212ccd71`. It adds **no general corpus
agreement credit**: 140 explicit unsupported outcomes, zero agreements,
disagreements or timeouts. The generated report retains every model; 138 raw
CLI invocations are retained, while two action-composition models were refused
by the harness before starting a subprocess.

The largest blockers were pre-bind opaque operator capture (32), an empty
OOS/temporal obligation list (31 safety-only models), native/runtime-name effects
(18), concrete-capture scope/effect admission (18), state-kind conditional
admission (16), and symmetry/VIEW/ALIAS witness recovery (9). Other rows retain
their precise named planning/resource/randomness/hook limitations. These are
refusals, not evidence that the models have no temporal violations.

`before.json` records the exact source, binary and archival launcher hashes.
`after.json` verifies unchanged source/binary bytes and every selected artifact.
The launcher transparently preserves raw stdout/stderr; the harness correctly
identifies its hash separately from the actual release executable. The shell's
postprocessing attempted to assign its read-only `status` variable after the
harness returned; the actual harness exit was therefore not retained and is
explicitly `null`, never inferred from the report. The complete native outcomes
and raw streams survived that bookkeeping failure.

The same frozen release passed eight supported positive models at 1, 3, 4 and 8
workers (32 CLI controls), all 20 then-current CM1 models and all 16 validation
smoke models at 1, 4 and 8 workers. The preceding full workspace gate passed
691 tests, 12 Python controls, formatting, clippy, documentation, layering,
standalone independent validation and dependency checks. The partial initial
release-control tooling observation is preserved separately: it encountered
an absent counterexample on a successful model before the tooling was fixed.

This history predates the shared current-source admission/worker-wave shipping
integration. It must not be relabelled as output from a later executable. The
user-facing mode is described in `docs/tlc-final-api.md`; a fresh run uses:

```sh
cargo build --release --locked -p dive-cli -p dive-conformance
cargo run --release --locked -p dive-conformance -- run \
  --impl dive-final-tlc --impl-profile tlc --dive-bin target/release/dive \
  --corpus examples --tier small --workers 1 --no-sany --no-summary \
  --jobs 1 --timeout 600 --report-dir conformance/reports
```

Safety-only models need a future real ordinary-safety route with its actual
counts, worker and validation evidence. No product graph or temporal proof may
be fabricated to promote these refusals. Wider scopes, runtime effects and
conditional planning need their own pinned controls before admission changes.
