# Host input controls

CLI tests copy these exact modules/configurations into a temporary working
directory. Captured/Demand/EnvironmentOrder write `input.txt` as UTF-8 `hello\n`.
Their child processes have exactly
`DIVE_COUNT=2` and `DIVE_LABEL=outside`; tests never capture the test runner's
ambient environment.

`oracle.json` retains compact primary expectations and digests from pinned
`tlc-2026.10.06.014338`, the configured CommunityModules/dependency jars and pinned
JRE. Those three primaries run with workers 1, seed 7, fp 0, SerialGC, heap 128 MiB and
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

The `native_history_controls` primaries use workers 1, seed 0, fp 0 and `-tool`
with the same pinned inputs. TwoFilesSafeNames uses exactly DIVE_LABEL=alpha,
first text `α-line\nβ-line\r\n`, and second text `second`. Its actual second
stdout is integer zero; TLC reports 2102/2103, exit 75 and one generated/distinct
state at depth one. Shared native result names explain this effect (#761).
The strict test refuses the second request in both profiles with only one
recorded input; callback tests also prove the second arguments are not reached.

TextBeforeEnvOrder and EnvBeforeTextOrder use exactly DIVE_LABEL=zUnknown and
file text `aUnknown`. Both primaries select zUnknown and violate the invariant.
TLC constructs static ENV before any file result, regardless of source order.
The former is a strict TLC refusal; the latter is an admitted/replayed violation.
The canonical default selects aUnknown under its existing CHOOSE order.

AtoiConcatBeforeEnvOrder uses exactly DIVE_LABEL=zUnknown, an earlier startup
atoi("0"), and pure `"a" \o "Unknown"`. The primary selects zUnknown and violates
the invariant. The strict TLC test refuses the first IOEnv after new startup
name construction before recording any environment input. This is a conservative
boundary: pure concat without earlier atoi, and ToString of a source numeral,
are successful separate primary controls retained in external evidence.

Other tests retain callback-free default denial, input bounds, proper record
options, runtime/exec/recovery refusals and forged or incomplete replay contexts.
See `docs/user/modules.md` and ADR 0148 for the public supported scope.
