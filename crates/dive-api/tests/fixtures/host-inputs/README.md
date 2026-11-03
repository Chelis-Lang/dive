# Host input controls

CLI tests copy these exact modules/configurations into a temporary working
directory and write `input.txt` as UTF-8 `hello\n`. Child processes have exactly
`DIVE_COUNT=2` and `DIVE_LABEL=outside`; tests never capture the test runner's
ambient environment.

`oracle.json` retains compact primary expectations and digests from pinned
`tlc-2026.10.06.014338`, the configured CommunityModules/dependency jars and pinned
JRE. Each primary runs with workers 1, seed 7, fp 0, SerialGC, heap 128 MiB and
stack 64 MiB, `-tool -dumpTrace json ... -config MODEL.cfg MODEL`. Primary
stdout/stderr, trace data, command receipts and states remain external working
evidence and are archived to S3 at milestone closeout; none are committed here.

Captured matches exit 12, generated/distinct/depth 3 and the integer word 2,3,4
in both profiles. Demand matches exit 12, generated/distinct/depth 2 and prints
`"options"` before `"path"`, confirming the evaluating override's argument order.
Both words replay after file deletion without ambient specification I/O.

EnvironmentOrder is the strict TLC construction-order refusal control (#758).
The primary reports an initial invariant violation at DIVE_LABEL and emits no
statistics. A prototype that sorted new strings incorrectly reported success
at DIVE_COUNT in TLC mode. The canonical default retains its existing CHOOSE
order; TLC snapshots with multiple unknown strings refuse.

Other tests retain callback-free default denial, input bounds, proper record
options, runtime/exec/recovery refusals and forged or incomplete replay contexts.
See `docs/user/modules.md` and ADR 0148 for the public supported scope.
